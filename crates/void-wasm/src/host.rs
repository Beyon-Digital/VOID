//! The restricted module host: a wasmtime engine configured for
//! denial — fuel metering, epoch deadlines, a resource limiter that
//! returns OUR typed errors, and a linker containing exactly one host
//! import (`void_host.log`). Everything else a module can import fails
//! validation in `spec::scan_wasm` and would fail linking anyway.
//!
//! Enforced invariants (T97):
//!   * fuel exhaustion  → `WasmError::FuelExhausted`   (deterministic)
//!   * epoch deadline   → `WasmError::DeadlineExceeded` (≈1 ms ticks)
//!   * cancel token     → `WasmError::Cancelled`        (cross-thread)
//!   * memory over limit→ `WasmError::MemoryLimitExceeded` (instantiate
//!     AND grow; our limiter returns the typed error, not a silent -1)
//!   * denied imports   → `WasmError::ImportDenied`     (pre-scan)
//!     plus a generic linker failure if a hand-built module ever
//!     reached instantiate anyway — nothing ambient is ever provided.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;
use wasmtime::{
    Caller, Config, Engine, Instance, Linker, Memory, Module, ResourceLimiter, Store,
    StoreContextMut, TypedFunc, UpdateDeadline,
};

use crate::abi;
use crate::capability::{Capability, Placement};
use crate::error::{Result, WasmError};
use crate::manifest::ModuleManifest;
use crate::policy::{HostPolicy, EPOCH_TICK_MS};
use crate::spec::ModuleSpec;

/// Absolute ceiling on a module package's wasm bytes — load-DoS guard.
pub const MAX_MODULE_BYTES: usize = 32 * 1024 * 1024;
/// One guest log call may copy at most this many bytes.
const MAX_LOG_LINE: usize = 256;

#[derive(Debug, Clone)]
pub struct LogRecord {
    pub level: i32,
    pub text: String,
}

/// Custom resource limiter — returns typed WasmErrors so denial is
/// distinguishable from a guest trap (a plain `false` would surface
/// only as memory.grow returning -1, hiding the denial).
struct ModuleLimits {
    memory_bytes: usize,
    table_elements: usize,
}

impl ResourceLimiter for ModuleLimits {
    fn memory_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        if desired > self.memory_bytes {
            return Err(WasmError::MemoryLimitExceeded {
                requested: desired,
                limit: self.memory_bytes,
            }
            .into());
        }
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        if desired > self.table_elements {
            return Err(WasmError::InvalidModulePackage(format!(
                "table grew to {desired} elements over cap {}",
                self.table_elements
            ))
            .into());
        }
        Ok(true)
    }

    fn instances(&self) -> usize {
        1
    }

    fn tables(&self) -> usize {
        8
    }

    fn memories(&self) -> usize {
        1
    }
}

pub(crate) struct HostState {
    limits: ModuleLimits,
    /// Fuel budget per guest call (from the manifest).
    fuel_budget: u64,
    /// Epoch ticks left in the current call (deadline accounting).
    remaining_ticks: i64,
    /// Deadline in ms for error reporting.
    deadline_ms: u64,
    /// Cross-thread cancel flag shared with the module's CancelToken.
    cancel: Arc<AtomicBool>,
    logs: Vec<LogRecord>,
    log_bytes: usize,
    log_cap: usize,
}

/// Cancel handle: dropping does nothing; `cancel()` asks the next
/// epoch tick (≤ ~1 ms) to terminate the in-flight guest call.
#[derive(Debug, Clone)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
}

impl CancelToken {
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::Relaxed);
    }
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::Relaxed)
    }
}

/// The host: owns the engine, the policy and the epoch ticker that
/// drives every store's deadline/cancel checks.
pub struct Host {
    engine: Engine,
    policy: HostPolicy,
    stop_epoch: Arc<AtomicBool>,
    ticker: Option<JoinHandle<()>>,
}

impl Host {
    /// Build a host and start its epoch ticker (one thread per Host;
    /// `ModuleRuntime` owns exactly one).
    pub fn new(policy: HostPolicy) -> Result<Host> {
        let mut config = Config::new();
        config.consume_fuel(true);
        config.epoch_interruption(true);
        let engine =
            Engine::new(&config).map_err(|e| WasmError::Engine(format!("engine init: {e}")))?;
        let stop_epoch = Arc::new(AtomicBool::new(false));
        let ticker = {
            let engine = engine.clone();
            let stop = stop_epoch.clone();
            std::thread::Builder::new()
                .name("void-wasm-epoch".into())
                .spawn(move || {
                    while !stop.load(Ordering::Relaxed) {
                        engine.increment_epoch();
                        std::thread::sleep(Duration::from_millis(EPOCH_TICK_MS));
                    }
                })
                .map_err(|e| WasmError::Engine(format!("epoch ticker spawn: {e}")))?
        };
        Ok(Host {
            engine,
            policy,
            stop_epoch,
            ticker: Some(ticker),
        })
    }

    pub fn policy(&self) -> &HostPolicy {
        &self.policy
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    fn new_store(&self, manifest: &ModuleManifest) -> Store<HostState> {
        let mut store = Store::new(
            &self.engine,
            HostState {
                limits: ModuleLimits {
                    memory_bytes: manifest.limits.max_memory_bytes as usize,
                    table_elements: 256,
                },
                fuel_budget: manifest.limits.max_fuel,
                remaining_ticks: 0,
                deadline_ms: manifest.limits.deadline_ms,
                cancel: Arc::new(AtomicBool::new(false)),
                logs: Vec::new(),
                log_bytes: 0,
                log_cap: self.policy.max_log_bytes,
            },
        );
        store.limiter(|s| &mut s.limits);
        // Fire the deadline callback every epoch tick; the callback
        // decrements the per-call remaining budget and honors cancel.
        store.set_epoch_deadline(1);
        store.epoch_deadline_callback(
            |mut ctx: StoreContextMut<'_, HostState>| -> wasmtime::Result<UpdateDeadline> {
                let data = ctx.data_mut();
                if data.cancel.load(Ordering::Relaxed) {
                    return Err(WasmError::Cancelled.into());
                }
                if data.remaining_ticks <= 0 {
                    let ms = data.deadline_ms;
                    return Err(WasmError::DeadlineExceeded(ms).into());
                }
                data.remaining_ticks -= 1;
                Ok(UpdateDeadline::Continue(1))
            },
        );
        store
    }

    fn linker(&self) -> Result<Linker<HostState>> {
        let mut linker = Linker::new(&self.engine);
        linker
            .func_wrap(
                crate::capability::HOST_MODULE,
                crate::capability::HOST_FN_LOG,
                |mut caller: Caller<'_, HostState>, level: i32, ptr: i32, len: i32| {
                    // Bounded: clamp the line, never fail the guest call,
                    // never touch fs/net — records stay in HostState.
                    let len = len.clamp(0, MAX_LOG_LINE as i32) as usize;
                    let ptr = if ptr < 0 { 0 } else { ptr as usize };
                    if let Some(mem) = caller
                        .get_export(crate::abi::EXPORT_MEMORY)
                        .and_then(|e| e.into_memory())
                    {
                        let mut buf = vec![0u8; len];
                        if mem.read(&caller, ptr, &mut buf).is_ok() {
                            let text = String::from_utf8_lossy(&buf).into_owned();
                            let data = caller.data_mut();
                            if data.log_bytes + text.len() <= data.log_cap {
                                data.log_bytes += text.len();
                                data.logs.push(LogRecord { level, text });
                            }
                        }
                    }
                    Ok(())
                },
            )
            .map_err(|e| WasmError::Engine(format!("linker: {e}")))?;
        Ok(linker)
    }

    /// Instantiate a verified spec: link (denied-import surface),
    /// ABI-shape check, capability→export check, placement check,
    /// then `void_abi_version` + `void_init` under budget.
    pub fn instantiate(&self, spec: &ModuleSpec) -> Result<ModuleInstance> {
        if spec.wasm_bytes.len() > MAX_MODULE_BYTES {
            return Err(WasmError::InvalidModulePackage(format!(
                "module exceeds {} byte package ceiling",
                MAX_MODULE_BYTES
            )));
        }
        check_placement(&spec.manifest)?;
        let mut store = self.new_store(&spec.manifest);
        let module = Module::new(&self.engine, &spec.wasm_bytes)
            .map_err(|e| WasmError::InvalidModulePackage(format!("compile: {e}")))?;
        let linker = self.linker()?;
        let cancel = CancelToken {
            flag: store.data().cancel.clone(),
        };
        // The start section (if any) runs at instantiate — arm a
        // budget so a malicious start can't spin forever.
        arm_call(&mut store)?;
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| map_err(&e))?;
        let memory = instance
            .get_memory(&mut store, abi::EXPORT_MEMORY)
            .ok_or_else(|| {
                WasmError::AbiViolation("`memory` export missing — ABI requires it".into())
            })?;
        let mut inst = ModuleInstance {
            store,
            instance,
            memory,
            manifest: spec.manifest.clone(),
            cancel,
        };
        inst.check_exports()?;
        inst.call_init()?;
        Ok(inst)
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.stop_epoch.store(true, Ordering::Relaxed);
        if let Some(t) = self.ticker.take() {
            let _ = t.join();
        }
    }
}

fn check_placement(m: &ModuleManifest) -> Result<()> {
    // Placement is where the runtime may schedule the module's entry
    // points; capabilities must agree with it (capability checks are
    // the contract — placement declares the placement policy).
    if m.has_capability(Capability::RenderOff) && m.placement != Placement::OfflineRender {
        return Err(WasmError::InvalidManifest(format!(
            "capability render-off requires placement offline_render (got {:?})",
            m.placement
        )));
    }
    if m.has_capability(Capability::MidiTransform)
        && !matches!(
            m.placement,
            Placement::MidiTransform | Placement::OfflineRender
        )
    {
        return Err(WasmError::InvalidManifest(format!(
            "capability midi-transform requires placement midi_transform/offline_render (got {:?})",
            m.placement
        )));
    }
    Ok(())
}

/// Arm a guest call: refill fuel to the module's declared budget and
/// reset the deadline tick counter. Every entry into guest code goes
/// through this — there is no unbudgeted path.
fn arm_call(store: &mut Store<HostState>) -> Result<()> {
    let (fuel, deadline_ms) = {
        let d = store.data();
        (d.fuel_budget, d.deadline_ms)
    };
    store
        .set_fuel(fuel)
        .map_err(|e| WasmError::Engine(format!("set_fuel: {e}")))?;
    let ticks = deadline_ms.div_ceil(EPOCH_TICK_MS).max(1) as i64;
    store.data_mut().remaining_ticks = ticks;
    // Re-arm the epoch deadline: callback fires every tick.
    store.set_epoch_deadline(1);
    Ok(())
}

/// Translate a wasmtime error into our typed vocabulary: our own
/// WasmError propagated through the boundary wins, then well-known
/// traps, then generic engine errors (never leaking internals).
pub fn map_err(err: &anyhow::Error) -> WasmError {
    if let Some(w) = err.downcast_ref::<WasmError>() {
        return rewrap(w);
    }
    if let Some(t) = err.downcast_ref::<wasmtime::Trap>() {
        return match t {
            wasmtime::Trap::OutOfFuel => WasmError::FuelExhausted { budget: 0 },
            wasmtime::Trap::Interrupt => WasmError::DeadlineExceeded(0),
            other => WasmError::Engine(format!("trap: {other:?}")),
        };
    }
    // ResourceLimiter errors surface as their anyhow message when the
    // downcast misses (e.g. wrapped) — still recognizable by content.
    WasmError::Engine(err.to_string())
}

fn rewrap(w: &WasmError) -> WasmError {
    // WasmError isn't Clone (io/json variants); rebuild the variants
    // that can legitimately cross the call boundary.
    match w {
        WasmError::Cancelled => WasmError::Cancelled,
        WasmError::DeadlineExceeded(ms) => WasmError::DeadlineExceeded(*ms),
        WasmError::MemoryLimitExceeded { requested, limit } => WasmError::MemoryLimitExceeded {
            requested: *requested,
            limit: *limit,
        },
        WasmError::ImportDenied { module, name } => WasmError::ImportDenied {
            module: module.clone(),
            name: name.clone(),
        },
        WasmError::FuelExhausted { budget } => WasmError::FuelExhausted { budget: *budget },
        other => WasmError::Engine(other.to_string()),
    }
}

/// A live, budgeted module instance. NOT Send/Sync through the public
/// API — calls run on the caller's thread under armed budgets; cancel
/// and the epoch ticker interrupt from elsewhere.
pub struct ModuleInstance {
    store: Store<HostState>,
    instance: Instance,
    memory: Memory,
    manifest: ModuleManifest,
    cancel: CancelToken,
}

impl ModuleInstance {
    pub fn manifest(&self) -> &ModuleManifest {
        &self.manifest
    }

    pub fn cancel_token(&self) -> CancelToken {
        self.cancel.clone()
    }

    /// Bounded guest logs captured so far.
    pub fn logs(&self) -> &[LogRecord] {
        &self.store.data().logs
    }

    fn arm(&mut self) -> Result<()> {
        arm_call(&mut self.store)
    }

    /// Guest-call error mapping — like `map_err` but fills the real
    /// fuel budget into FuelExhausted for evidence-grade reporting.
    fn call_err(&self, e: &anyhow::Error) -> WasmError {
        match map_err(e) {
            WasmError::FuelExhausted { .. } => WasmError::FuelExhausted {
                budget: self.store.data().fuel_budget,
            },
            other => other,
        }
    }

    fn typed<Params, Results>(&mut self, name: &str) -> Result<TypedFunc<Params, Results>>
    where
        Params: wasmtime::WasmParams,
        Results: wasmtime::WasmResults,
    {
        self.instance
            .get_typed_func::<Params, Results>(&mut self.store, name)
            .map_err(|_| WasmError::AbiViolation(format!("`{name}` missing or mis-typed")))
    }

    /// Check the ABI: required exports exist and every declared
    /// capability's required export set is present.
    fn check_exports(&mut self) -> Result<()> {
        for required in [abi::FN_ALLOC, abi::FN_ABI_VERSION, abi::FN_INIT] {
            if self
                .instance
                .get_export(&mut self.store, required)
                .is_none()
            {
                return Err(WasmError::AbiViolation(format!(
                    "required export `{required}` missing"
                )));
            }
        }
        for cap in &self.manifest.capabilities {
            for export in cap.required_exports() {
                if self.instance.get_export(&mut self.store, export).is_none() {
                    return Err(WasmError::MissingCapabilityExport {
                        capability: cap.as_str().to_string(),
                        export: export.to_string(),
                    });
                }
            }
        }
        Ok(())
    }

    fn call_init(&mut self) -> Result<()> {
        let ver: i32 = {
            self.arm()?;
            self.typed::<(), i32>(abi::FN_ABI_VERSION)?
                .call(&mut self.store, ())
                .map_err(|e| self.call_err(&e))?
        };
        if ver != abi::ABI_VERSION {
            return Err(WasmError::AbiViolation(format!(
                "void_abi_version {ver} ≠ host {}",
                abi::ABI_VERSION
            )));
        }
        self.arm()?;
        let status = self
            .typed::<(), i32>(abi::FN_INIT)?
            .call(&mut self.store, ())
            .map_err(|e| self.call_err(&e))?;
        if status != 0 {
            return Err(WasmError::InitRejected(status));
        }
        // Params: count must stay inside the declared bound.
        if self.manifest.has_capability(Capability::Params) {
            let n = self.param_count()?;
            if n < 0 || n as u64 > self.manifest.limits.max_params {
                return Err(WasmError::AbiViolation(format!(
                    "void_param_count {n} exceeds declared bound {}",
                    self.manifest.limits.max_params
                )));
            }
        }
        // State: blob bound must stay inside the declared limit.
        if self.manifest.has_capability(Capability::State) {
            let n = self.state_len()?;
            if n < 0 || n as u64 > self.manifest.limits.max_state_bytes {
                return Err(WasmError::StateTooLarge {
                    actual: n.max(0) as usize,
                    bound: self.manifest.limits.max_state_bytes as usize,
                });
            }
        }
        Ok(())
    }

    fn require_cap(&self, cap: Capability) -> Result<()> {
        if !self.manifest.has_capability(cap) {
            return Err(WasmError::CapabilityNotDeclared(cap.as_str().into()));
        }
        Ok(())
    }

    /// Guest-side allocation: `void_alloc(len)` → pointer. Used by the
    /// host to reserve output/state buffers; bounds-checked on use.
    fn guest_alloc(&mut self, len: usize) -> Result<i32> {
        if len > i32::MAX as usize {
            return Err(WasmError::InvalidModulePackage(format!(
                "alloc request {len} exceeds i32 range"
            )));
        }
        self.arm()?;
        let ptr = self
            .typed::<i32, i32>(abi::FN_ALLOC)?
            .call(&mut self.store, len as i32)
            .map_err(|e| self.call_err(&e))?;
        if ptr < 0 {
            return Err(WasmError::AbiViolation(format!(
                "void_alloc returned invalid pointer {ptr}"
            )));
        }
        Ok(ptr)
    }

    fn mem_write(&mut self, ptr: i32, bytes: &[u8]) -> Result<()> {
        self.memory
            .write(&mut self.store, ptr as usize, bytes)
            .map_err(|_| {
                WasmError::AbiViolation(format!(
                    "write {ptr}+{len} outside memory",
                    len = bytes.len()
                ))
            })
    }

    fn mem_read(&mut self, ptr: i32, len: usize) -> Result<Vec<u8>> {
        let mut buf = vec![0u8; len];
        self.memory
            .read(&self.store, ptr as usize, &mut buf)
            .map_err(|_| WasmError::AbiViolation(format!("read {ptr}+{len} outside memory")))?;
        Ok(buf)
    }

    // ---- params --------------------------------------------------

    pub fn param_count(&mut self) -> Result<i32> {
        self.require_cap(Capability::Params)?;
        self.arm()?;
        self.typed::<(), i32>(abi::FN_PARAM_COUNT)?
            .call(&mut self.store, ())
            .map_err(|e| self.call_err(&e))
    }

    pub fn param_get(&mut self, index: i32) -> Result<f64> {
        self.require_cap(Capability::Params)?;
        if index < 0 {
            return Err(WasmError::AbiViolation(format!("param index {index} < 0")));
        }
        self.arm()?;
        self.typed::<i32, f64>(abi::FN_PARAM_GET)?
            .call(&mut self.store, index)
            .map_err(|e| self.call_err(&e))
    }

    pub fn param_set(&mut self, index: i32, value: f64) -> Result<()> {
        self.require_cap(Capability::Params)?;
        if index < 0 {
            return Err(WasmError::AbiViolation(format!("param index {index} < 0")));
        }
        if !value.is_finite() {
            return Err(WasmError::AbiViolation(
                "param value must be finite (NaN/Inf rejected)".into(),
            ));
        }
        self.arm()?;
        let status = self
            .typed::<(i32, f64), i32>(abi::FN_PARAM_SET)?
            .call(&mut self.store, (index, value))
            .map_err(|e| self.call_err(&e))?;
        if status != 0 {
            return Err(WasmError::GuestRejected {
                name: abi::FN_PARAM_SET,
                status,
            });
        }
        Ok(())
    }

    // ---- state ----------------------------------------------------

    pub fn state_len(&mut self) -> Result<i32> {
        self.require_cap(Capability::State)?;
        self.arm()?;
        self.typed::<(), i32>(abi::FN_STATE_LEN)?
            .call(&mut self.store, ())
            .map_err(|e| self.call_err(&e))
    }

    /// Snapshot the module's opaque state blob (hot-reload/save path).
    pub fn state_save(&mut self) -> Result<Vec<u8>> {
        self.require_cap(Capability::State)?;
        let len = self.state_len()?;
        if len < 0 || len as u64 > self.manifest.limits.max_state_bytes {
            return Err(WasmError::StateTooLarge {
                actual: len.max(0) as usize,
                bound: self.manifest.limits.max_state_bytes as usize,
            });
        }
        let ptr = self.guest_alloc(len as usize)?;
        self.arm()?;
        let written = self
            .typed::<i32, i32>(abi::FN_STATE_SAVE)?
            .call(&mut self.store, ptr)
            .map_err(|e| self.call_err(&e))?;
        if written < 0 || written > len {
            return Err(WasmError::OutputOverflow {
                claimed: written.max(0) as usize,
                granted: len as usize,
            });
        }
        self.mem_read(ptr, written as usize)
    }

    /// Restore a previously saved state blob (hot-reload target).
    pub fn state_restore(&mut self, state: &[u8]) -> Result<()> {
        self.require_cap(Capability::State)?;
        if state.len() as u64 > self.manifest.limits.max_state_bytes {
            return Err(WasmError::StateTooLarge {
                actual: state.len(),
                bound: self.manifest.limits.max_state_bytes as usize,
            });
        }
        let ptr = self.guest_alloc(state.len())?;
        self.mem_write(ptr, state)?;
        self.arm()?;
        let status = self
            .typed::<(i32, i32), i32>(abi::FN_STATE_RESTORE)?
            .call(&mut self.store, (ptr, state.len() as i32))
            .map_err(|e| self.call_err(&e))?;
        if status != 0 {
            return Err(WasmError::GuestRejected {
                name: abi::FN_STATE_RESTORE,
                status,
            });
        }
        Ok(())
    }

    // ---- offline render --------------------------------------------

    /// Run `void_render_off` — OFFLINE render only. The caller is a
    /// job thread; this entry point is never wired into an audio
    /// callback (placement is declared, not requested).
    pub fn render_off(&mut self, frames: u32) -> Result<Vec<f32>> {
        self.require_cap(Capability::RenderOff)?;
        let bytes = (frames as usize)
            .checked_mul(4)
            .ok_or_else(|| WasmError::InvalidModulePackage("frame count overflow".into()))?;
        if bytes as u64 > self.manifest.limits.max_out_bytes {
            return Err(WasmError::OutputOverflow {
                claimed: bytes,
                granted: self.manifest.limits.max_out_bytes as usize,
            });
        }
        let ptr = self.guest_alloc(bytes)?;
        self.arm()?;
        let written_frames = self
            .typed::<(i32, i32), i32>(abi::FN_RENDER_OFF)?
            .call(&mut self.store, (ptr, frames as i32))
            .map_err(|e| self.call_err(&e))?;
        if written_frames < 0 || written_frames > frames as i32 {
            return Err(WasmError::OutputOverflow {
                claimed: (written_frames.max(0) as usize) * 4,
                granted: bytes,
            });
        }
        let raw = self.mem_read(ptr, written_frames as usize * 4)?;
        let mut out = Vec::with_capacity(written_frames as usize);
        for chunk in raw.as_chunks::<4>().0.iter() {
            out.push(f32::from_le_bytes(*chunk));
        }
        Ok(out)
    }

    // ---- midi transform ----------------------------------------------

    /// Run `void_midi_xform` — bounded NoteEvent transform. Input and
    /// output are packed 24-byte event records; output is capped by
    /// `max_out_bytes` and every emitted event is range-validated
    /// (FX-07: a bad script cannot emit unbounded or malformed events).
    pub fn midi_xform(&mut self, input_events: &[abi::NoteEvent]) -> Result<Vec<abi::NoteEvent>> {
        self.require_cap(Capability::MidiTransform)?;
        for (i, e) in input_events.iter().enumerate() {
            if !e.is_valid() {
                return Err(WasmError::InvalidModulePackage(format!(
                    "input event {i} invalid (pitch/velocity/channel/length range)"
                )));
            }
        }
        let input = abi::encode_events(input_events);
        let in_ptr = self.guest_alloc(input.len().max(1))?;
        self.mem_write(in_ptr, &input)?;
        let out_cap = (input_events.len().saturating_mul(4).max(64) * abi::NOTE_EVENT_SIZE)
            .min(self.manifest.limits.max_out_bytes as usize);
        let out_ptr = self.guest_alloc(out_cap)?;
        self.arm()?;
        let written = self
            .typed::<(i32, i32, i32, i32), i32>(abi::FN_MIDI_XFORM)?
            .call(
                &mut self.store,
                (in_ptr, input.len() as i32, out_ptr, out_cap as i32),
            )
            .map_err(|e| self.call_err(&e))?;
        if written < 0 || written as usize > out_cap {
            return Err(WasmError::OutputOverflow {
                claimed: written.max(0) as usize,
                granted: out_cap,
            });
        }
        if !(written as usize).is_multiple_of(abi::NOTE_EVENT_SIZE) {
            return Err(WasmError::AbiViolation(format!(
                "midi_xform returned {written} bytes — not a multiple of {}",
                abi::NOTE_EVENT_SIZE
            )));
        }
        let raw = self.mem_read(out_ptr, written as usize)?;
        let events = abi::decode_events(&raw);
        for (i, e) in events.iter().enumerate() {
            if !e.is_valid() {
                return Err(WasmError::AbiViolation(format!(
                    "midi_xform emitted malformed event at index {i}"
                )));
            }
        }
        Ok(events)
    }
}
