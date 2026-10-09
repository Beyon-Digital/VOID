//! Validated WGSL preset registry (T84): generated shader bodies pass
//! a real compile pipeline — size gate → forbidden-builtin scan → naga
//! WGSL parse → naga IR validation (empty capabilities) → static cost
//! budget — before they may render. Compile results are cached by
//! content hash. Watchdog/cancel is an honest policy model (monotonic
//! ns deadlines; the watchdog worker runs under JobBudget rlimits and
//! coordinator cancel). Fallback chain resolves to a known-good
//! builtin; every level returns the report that failed it (T84).

use crate::analysis::{module_cost, ShaderCost};
use crate::error::{Result, VisFxError};
use crate::spec::hex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Budget descriptor
// ---------------------------------------------------------------------------

/// Resource-budget descriptor attached to a preset. Wall-clock bounds
/// live in WatchdogPolicy (monotonic ns); these bound the shader's
/// per-pixel work + resource footprint, which the coordinator folds
/// into the watchdog worker's JobSpec reservations.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceBudget {
    /// Max texture dimension the shader may be invoked on (tex size).
    pub max_tex_width: u32,
    pub max_tex_height: u32,
    /// Per-entry-point bound on sampled-texture ops × loop trips —
    /// the "iterations" budget that rejects expensive shaders.
    pub max_sample_iterations: u64,
    /// Flat instruction bound per entry point.
    pub max_instructions: u64,
    /// Source size cap (cheap early reject).
    pub max_source_bytes: u32,
    /// `loop{}` with no break_if is rejected unless explicitly allowed.
    pub allow_unbounded_loops: bool,
}

impl Default for ResourceBudget {
    fn default() -> Self {
        Self {
            max_tex_width: 2048,
            max_tex_height: 2048,
            max_sample_iterations: 64,
            max_instructions: 4096,
            max_source_bytes: 64 * 1024,
            allow_unbounded_loops: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Compile pipeline
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStage {
    /// Layout/source-size gate.
    Shape,
    /// Forbidden-builtin scan (side-effecting storage/atomics/barriers).
    ForbiddenCheck,
    /// naga WGSL front-end parse.
    Parse,
    /// naga IR validation.
    Validate,
    /// Static cost model vs ResourceBudget.
    Budget,
    /// Runtime: killed by watchdog/deadline (recorded, not predicted).
    Watchdog,
    /// Runtime: renderer-reported failure (recorded, not predicted).
    Kernel,
}

impl ValidationStage {
    pub const ORDER: &'static [ValidationStage] = &[
        Self::Shape,
        Self::ForbiddenCheck,
        Self::Parse,
        Self::Validate,
        Self::Budget,
        Self::Watchdog,
        Self::Kernel,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Shape => "shape",
            Self::ForbiddenCheck => "forbidden_check",
            Self::Parse => "parse",
            Self::Validate => "validate",
            Self::Budget => "budget",
            Self::Watchdog => "watchdog",
            Self::Kernel => "kernel",
        }
    }
    /// Compile-time stages this crate can actually execute on Linux.
    pub fn static_stages() -> &'static [ValidationStage] {
        &ValidationStage::ORDER[..5]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RejectReason {
    InvalidLayout,
    Forbidden,
    InvalidSyntax,
    InvalidSemantics,
    OverBudget,
    WatchdogTimeout,
    WatchdogCancelled,
    KernelFailed,
    Disabled,
}

impl RejectReason {
    pub fn label(self) -> &'static str {
        match self {
            Self::InvalidLayout => "invalid_layout",
            Self::Forbidden => "forbidden",
            Self::InvalidSyntax => "invalid_syntax",
            Self::InvalidSemantics => "invalid_semantics",
            Self::OverBudget => "over_budget",
            Self::WatchdogTimeout => "watchdog_timeout",
            Self::WatchdogCancelled => "watchdog_cancelled",
            Self::KernelFailed => "kernel_failed",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CompileReport {
    pub shader_id: String,
    /// sha256 of the composed module actually compiled (cache key).
    pub composed_sha256: String,
    /// Stage the pipeline reached (first failure wins).
    pub stage: ValidationStage,
    /// All compile-time stages before `stage` passed.
    pub passed_all_previous: bool,
    /// Static cost measurements (populated once Validate passed).
    #[serde(default)]
    pub cost: Option<CostFacts>,
    /// Wall-time the compile pipeline took (ns).
    #[serde(default)]
    pub wall_ns: u64,
    /// Runtime measurements when a watchdog/kernel outcome merged in.
    #[serde(default)]
    pub runtime_ns: Option<u64>,
    pub rejection: Option<RejectReason>,
    #[serde(default)]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CostFacts {
    pub texture_samples_per_pixel: u64,
    pub instructions: u64,
    pub has_unbounded_loop: bool,
}

impl From<&ShaderCost> for CostFacts {
    fn from(c: &ShaderCost) -> Self {
        Self {
            texture_samples_per_pixel: c.texture_samples_per_pixel,
            instructions: c.instructions,
            has_unbounded_loop: c.has_unbounded_loop,
        }
    }
}

impl CompileReport {
    fn new(shader_id: &str, composed_sha256: &str) -> Self {
        Self {
            shader_id: shader_id.into(),
            composed_sha256: composed_sha256.into(),
            stage: ValidationStage::Shape,
            passed_all_previous: false,
            cost: None,
            wall_ns: 0,
            runtime_ns: None,
            rejection: None,
            detail: None,
        }
    }
    pub fn ok(&self) -> bool {
        self.rejection.is_none()
    }
    fn fail(&mut self, stage: ValidationStage, r: RejectReason, detail: String) {
        self.stage = stage;
        self.passed_all_previous = ValidationStage::static_stages()
            .iter()
            .position(|s| *s == stage)
            .map(|_| true)
            .unwrap_or(true);
        self.rejection = Some(r);
        self.detail = Some(detail);
    }
}

/// Runtime outcome fed back into the registry (watchdog kill, renderer
/// error) — a failed report is minted and the shader quarantined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelOutcome {
    Ok { wall_ns: u64 },
    Timeout { wall_ns: u64 },
    Cancelled,
    Failed { detail_code: &'static str },
}

// ---------------------------------------------------------------------------
// Forbidden builtins — generated shaders are pure fragment code; the
// host never binds storage buffers/textures to them, so storage and
// synchronization builtins only exist to smuggle side effects.
// ---------------------------------------------------------------------------

const FORBIDDEN_BUILTINS: &[&str] = &[
    "textureStore",
    "atomicStore",
    "atomicLoad",
    "atomicAdd",
    "atomicSub",
    "atomicMax",
    "atomicMin",
    "atomicAnd",
    "atomicOr",
    "atomicXor",
    "atomicExchange",
    "atomicCompareExchangeWeak",
    "workgroupBarrier",
    "workgroupUniformLoad",
    "storageBarrier",
    "textureBarrier",
    // enables clause keywords the preset template never allows
    "enable ",
    "requires ",
];

// ---------------------------------------------------------------------------
// Presets + registry persistence
// ---------------------------------------------------------------------------

/// A shader preset reference — builtin (compiled into void-visual's
/// generator registry) or a validated generated body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum PresetRef {
    /// Builtin registry name (e.g. "plasma").
    Builtin { name: String },
    /// Generated body registered under `id`.
    Generated { id: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresetStatus {
    Active,
    /// Compile/kernel failure — resolves only through fallback.
    Quarantined,
    Disabled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetRecord {
    pub id: String,
    /// builtin | generated
    pub kind: String,
    /// WGSL generator body (generated only; builtins resolve by name).
    #[serde(default)]
    pub body: Option<String>,
    pub status: PresetStatus,
    /// Most recent compile report (also the cache entry).
    #[serde(default)]
    pub last_report: Option<CompileReport>,
    #[serde(default)]
    pub description: Option<String>,
    /// Decimal u64 string.
    #[serde(default)]
    pub seed: Option<String>,
    pub added_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegistryFile {
    schema_version: u32,
    presets: Vec<PresetRecord>,
}

// ---------------------------------------------------------------------------
// Watchdog / cancel policy model (T84)
// ---------------------------------------------------------------------------

/// Honest policy model for the isolated shader renderer: the watchdog
/// worker runs under void-jobs `JobBudget` (monotonic deadline +
/// rlimits) and coordinator cancel. This crate does not spawn the
/// renderer — it models and validates the policy the runtime enforces.
/// NOTE: rlimits cannot cap GPU memory; `min_reserved_ram_bytes` is a
/// floor for the *worker process* staging+compile buffers only.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WatchdogPolicy {
    /// Monotonic ns deadline: renderer exceeding it is killed
    /// (watchdog_timeout) and the shader quarantined.
    pub kill_after_ns: u64,
    /// Grace after coordinator cancel before kill (ns).
    pub cancel_grace_ns: u64,
    /// Worker-process RAM floor for staging/compile buffers (honest:
    /// CPU-side only — GPU budget is the ResourceBudget).
    pub min_reserved_ram_bytes: u64,
    /// If the renderer emits no progress/heartbeat in this window,
    /// treat as hung → timeout.
    pub heartbeat_timeout_ns: u64,
    /// Timeout counts as validation failure (quarantine) vs retry.
    pub timeout_quarantines: bool,
}

impl Default for WatchdogPolicy {
    fn default() -> Self {
        Self {
            kill_after_ns: 250_000_000,   // 250ms per frame
            cancel_grace_ns: 50_000_000,  // 50ms
            min_reserved_ram_bytes: 64 * 1024 * 1024,
            heartbeat_timeout_ns: 100_000_000,
            timeout_quarantines: true,
        }
    }
}

impl WatchdogPolicy {
    pub fn validate(&self) -> Result<()> {
        if self.kill_after_ns == 0 {
            return Err(VisFxError::Shader("watchdog kill_after_ns=0".into()));
        }
        if self.heartbeat_timeout_ns >= self.kill_after_ns {
            return Err(VisFxError::Shader(
                "heartbeat_timeout must be < kill_after_ns".into(),
            ));
        }
        if self.min_reserved_ram_bytes < 8 * 1024 * 1024 {
            return Err(VisFxError::Shader("min_reserved_ram <8MiB".into()));
        }
        Ok(())
    }
    /// JobSpec reservations for a watchdog-run render job (string
    /// int64 fields per wire contract).
    pub fn reservations(&self) -> void_jobs::Reservations {
        void_jobs::Reservations {
            ram_bytes: self.min_reserved_ram_bytes.to_string(),
            vram_bytes: "0".into(), // honest: rlimit can't cap GPU mem (NEEDS)
            cpu_threads: 1,
        }
    }
}

// ---------------------------------------------------------------------------
// Fallback policy + chain
// ---------------------------------------------------------------------------

/// Per-stage fallback targets (builtin names). Missing entry →
/// `DEFAULT_FALLBACK`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FallbackPolicy {
    #[serde(default)]
    pub on_invalid: Option<String>,
    #[serde(default)]
    pub on_over_budget: Option<String>,
    #[serde(default)]
    pub on_watchdog: Option<String>,
    #[serde(default)]
    pub on_kernel: Option<String>,
}

pub const DEFAULT_FALLBACK: &str = "black";

/// Builtin generator preset ids — mirrors `generator_body`'s match
/// arms in void-visual (the registry contract, kept adjacent so a new
/// builtin upstream is picked up by simply extending the match there
/// and this list here).
pub const BUILTIN_PRESETS: &[&str] = &[
    "black",
    "color-bars",
    "checker",
    "gradient",
    "plasma",
    "pulse",
];

impl FallbackPolicy {
    fn target(&self, r: RejectReason) -> String {
        match r {
            RejectReason::InvalidSyntax | RejectReason::InvalidSemantics | RejectReason::Forbidden | RejectReason::InvalidLayout | RejectReason::Disabled => {
                self.on_invalid.clone()
            }
            RejectReason::OverBudget => self.on_over_budget.clone(),
            RejectReason::WatchdogTimeout | RejectReason::WatchdogCancelled => {
                self.on_watchdog.clone()
            }
            RejectReason::KernelFailed => self.on_kernel.clone(),
        }
        .unwrap_or_else(|| DEFAULT_FALLBACK.into())
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedShader {
    /// Preset id that produced `wgsl`.
    pub id: String,
    /// True when the requested preset failed and a fallback supplied it.
    pub fell_back: bool,
    /// Full composed WGSL module for the renderer.
    pub wgsl: String,
    /// The report for THIS resolution (success, or last failure in chain).
    pub report: CompileReport,
    /// Failure chain that led here — empty for direct success.
    pub chain: Vec<CompileReport>,
}

/// Monadic composition per the work item: each level returns the
/// shader or the report that failed it; `.or(next)` adds a fallback.
pub struct ShaderChain<'a> {
    registry: &'a mut ShaderRegistry,
    outcome: std::result::Result<ResolvedShader, CompileReport>,
}

impl<'a> ShaderChain<'a> {
    /// Try another preset when the current level failed; returns the
    /// shader or the accumulated failure chain.
    pub fn or(self, next: PresetRef) -> Self {
        let Self {
            registry, outcome, ..
        } = self;
        match outcome {
            ok @ Ok(_) => Self {
                registry,
                outcome: ok,
            },
            Err(first) => match registry.resolve(&next) {
                Ok(mut r) => {
                    r.fell_back = true;
                    r.chain.push(first);
                    Self {
                        registry,
                        outcome: Ok(r),
                    }
                }
                Err(second) => Self {
                    registry,
                    outcome: Err(CompileReport::chained(first, second)),
                },
            },
        }
    }
    pub fn resolve(self) -> std::result::Result<ResolvedShader, CompileReport> {
        self.outcome
    }
}

impl CompileReport {
    /// Squash a chain: the deepest report wins with prior stages
    /// recorded in detail.
    fn chained(first: CompileReport, second: CompileReport) -> CompileReport {
        let mut s = second;
        s.detail = Some(format!(
            "fallback after {}:{} ({})",
            first.shader_id,
            first.stage.label(),
            first.detail.unwrap_or_else(|| "-".into())
        ));
        s
    }
}

// ---------------------------------------------------------------------------
// Compile cache
// ---------------------------------------------------------------------------

const CACHE_MAX: usize = 256;

#[derive(Debug, Default)]
struct CompileCache {
    /// composed_sha256 → report
    entries: BTreeMap<String, CompileReport>,
}

impl CompileCache {
    fn get(&self, key: &str) -> Option<&CompileReport> {
        self.entries.get(key)
    }
    fn put(&mut self, key: String, r: CompileReport) {
        if self.entries.len() >= CACHE_MAX {
            if let Some(k) = self.entries.keys().next().cloned() {
                self.entries.remove(&k);
            }
        }
        self.entries.insert(key, r);
    }
    fn remove(&mut self, key: &str) {
        self.entries.remove(key);
    }
}

// ---------------------------------------------------------------------------
// Registry
// ---------------------------------------------------------------------------

pub struct ShaderRegistry {
    path: PathBuf,
    presets: BTreeMap<String, PresetRecord>,
    cache: CompileCache,
    pub budget: ResourceBudget,
    pub watchdog: WatchdogPolicy,
    pub fallback: FallbackPolicy,
}

impl ShaderRegistry {
    /// container/.void layout: `<container>/visfx/shaders.json`.
    pub fn open(container: &Path) -> Result<Self> {
        let dir = container.join("visfx");
        fs::create_dir_all(&dir)?;
        let path = dir.join("shaders.json");
        let mut presets = BTreeMap::new();
        if path.exists() {
            let f: RegistryFile = serde_json::from_slice(&fs::read(&path)?)?;
            for p in f.presets {
                presets.insert(p.id.clone(), p);
            }
        }
        let mut reg = Self {
            path,
            presets,
            cache: CompileCache::default(),
            budget: ResourceBudget::default(),
            watchdog: WatchdogPolicy::default(),
            fallback: FallbackPolicy {
                on_invalid: None,
                on_over_budget: None,
                on_watchdog: None,
                on_kernel: None,
            },
        };
        reg.ensure_builtins()?;
        Ok(reg)
    }

    /// Seed builtin presets from void-visual's generator registry —
    /// `BUILTIN_PRESETS` mirrors `generator_body`'s match arms (the
    /// void-visual contract for builtin generator names).
    fn ensure_builtins(&mut self) -> Result<()> {
        for name in BUILTIN_PRESETS {
            if !self.presets.contains_key(*name) {
                self.presets.insert(
                    (*name).into(),
                    PresetRecord {
                        id: (*name).into(),
                        kind: "builtin".into(),
                        body: None,
                        status: PresetStatus::Active,
                        last_report: None,
                        description: Some("void-visual builtin".into()),
                        seed: None,
                        added_at: crate::store::utc_now(),
                    },
                );
            }
        }
        self.persist()
    }

    fn persist(&self) -> Result<()> {
        let f = RegistryFile {
            schema_version: 1,
            presets: self.presets.values().cloned().collect(),
        };
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(&f)?)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<&PresetRecord> {
        self.presets.get(id)
    }

    /// Register a generated shader body — must survive the compile
    /// pipeline to be stored as Active (T84: validated presets only).
    pub fn register_generated(
        &mut self,
        id: &str,
        body: &str,
        description: Option<String>,
        seed: Option<u64>,
    ) -> Result<PresetRecord> {
        if !void_protocol::ids::is_valid_id(id) {
            return Err(VisFxError::Shader(format!("preset id {id:?} not a uuid")));
        }
        let report = self.compile_body(id, body);
        if !report.ok() {
            return Err(VisFxError::Shader(format!(
                "rejected at {}: {}",
                report.stage.label(),
                report.detail.unwrap_or_default()
            )));
        }
        let rec = PresetRecord {
            id: id.into(),
            kind: "generated".into(),
            body: Some(body.into()),
            status: PresetStatus::Active,
            last_report: Some(report),
            description,
            seed: seed.map(|s| s.to_string()),
            added_at: crate::store::utc_now(),
        };
        self.presets.insert(id.into(), rec.clone());
        self.persist()?;
        Ok(rec)
    }

    /// Composed module for a body via void-visual's template contract.
    pub fn compose(body: &str) -> String {
        void_visual::shaders::GENERATOR_TEMPLATE.replace("%BODY%", body)
    }

    fn compose_for(&self, rec: &PresetRecord) -> Result<String> {
        match rec.kind.as_str() {
            "builtin" => {
                let body = void_visual::shaders::generator_body(&rec.id).ok_or_else(|| {
                    VisFxError::Shader(format!("unknown builtin {:?}", rec.id))
                })?;
                Ok(Self::compose(&body))
            }
            "generated" => Ok(Self::compose(rec.body.as_deref().unwrap_or(""))),
            other => Err(VisFxError::Shader(format!("preset kind {other:?}"))),
        }
    }

    /// Full compile pipeline on an arbitrary body (not registered).
    /// Returns the report — caller decides registration/fallback.
    pub fn compile_body(&mut self, shader_id: &str, body: &str) -> CompileReport {
        let wgsl = Self::compose(body);
        self.compile_wgsl(shader_id, &wgsl)
    }

    /// Compile a composed WGSL module through all static stages.
    pub fn compile_wgsl(&mut self, shader_id: &str, wgsl: &str) -> CompileReport {
        let key = hex(&Sha256::digest(wgsl.as_bytes()));
        if let Some(r) = self.cache.get(&key) {
            let mut r = r.clone();
            r.shader_id = shader_id.into();
            return r;
        }
        let t0 = std::time::Instant::now();
        let mut rep = CompileReport::new(shader_id, &key);

        // Stage 1: shape — bounded source, nonempty.
        if wgsl.is_empty() || wgsl.len() > self.budget.max_source_bytes as usize {
            rep.fail(
                ValidationStage::Shape,
                RejectReason::InvalidLayout,
                format!("source {} bytes > {}", wgsl.len(), self.budget.max_source_bytes),
            );
        }
        // Stage 2: forbidden builtins (isolated-renderer contract).
        if rep.ok() {
            for b in FORBIDDEN_BUILTINS {
                if wgsl.contains(b) {
                    rep.fail(
                        ValidationStage::ForbiddenCheck,
                        RejectReason::Forbidden,
                        format!("builtin {b:?} is outside the reactive-vis contract"),
                    );
                    break;
                }
            }
        }
        // Stage 3+4: naga parse + IR validation.
        let mut module_opt = None;
        if rep.ok() {
            match naga::front::wgsl::parse_str(wgsl) {
                Ok(m) => module_opt = Some(m),
                Err(e) => rep.fail(
                    ValidationStage::Parse,
                    RejectReason::InvalidSyntax,
                    format!("{e:?}"),
                ),
            }
        }
        if rep.ok() {
            let m = module_opt.as_ref().unwrap();
            let mut v = naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                // reactive fragment code needs no extra capabilities —
                // empty set rejects push-constant/subgroup/etc. surfaces
                naga::valid::Capabilities::empty(),
            );
            if let Err(e) = v.validate(m) {
                rep.fail(
                    ValidationStage::Validate,
                    RejectReason::InvalidSemantics,
                    format!("{e:?}"),
                );
            }
        }
        // Stage 5: static cost budget.
        if rep.ok() {
            let m = module_opt.as_ref().unwrap();
            let cost = module_cost(m);
            rep.cost = Some(CostFacts::from(&cost));
            if cost.has_unbounded_loop && !self.budget.allow_unbounded_loops {
                rep.fail(
                    ValidationStage::Budget,
                    RejectReason::OverBudget,
                    "unbounded loop{} (no break_if)".into(),
                );
            } else if cost.texture_samples_per_pixel > self.budget.max_sample_iterations {
                rep.fail(
                    ValidationStage::Budget,
                    RejectReason::OverBudget,
                    format!(
                        "{} tex samples/px > budget {}",
                        cost.texture_samples_per_pixel, self.budget.max_sample_iterations
                    ),
                );
            } else if cost.instructions > self.budget.max_instructions {
                rep.fail(
                    ValidationStage::Budget,
                    RejectReason::OverBudget,
                    format!(
                        "{} instructions > budget {}",
                        cost.instructions, self.budget.max_instructions
                    ),
                );
            }
        }
        rep.wall_ns = t0.elapsed().as_nanos() as u64;
        if rep.ok() {
            rep.stage = ValidationStage::Budget;
            rep.passed_all_previous = true;
        }
        self.cache.put(key, rep.clone());
        rep
    }

    /// Resolve a preset to a compiled module — the cache makes repeat
    /// resolves free; failures return the report (never a fake).
    pub fn resolve(&mut self, req: &PresetRef) -> std::result::Result<ResolvedShader, CompileReport> {
        let id = match req {
            PresetRef::Builtin { name } => name.clone(),
            PresetRef::Generated { id } => id.clone(),
        };
        let Some(rec) = self.presets.get(&id).cloned() else {
            let mut r = CompileReport::new(&id, "");
            r.fail(
                ValidationStage::Shape,
                RejectReason::InvalidLayout,
                format!("unknown preset {id:?}"),
            );
            return Err(r);
        };
        if rec.status != PresetStatus::Active {
            let mut r = CompileReport::new(&id, "");
            r.fail(
                ValidationStage::Shape,
                RejectReason::Disabled,
                format!("preset status {:?}", rec.status),
            );
            return Err(r);
        }
        let wgsl = match self.compose_for(&rec) {
            Ok(w) => w,
            Err(e) => {
                let mut r = CompileReport::new(&id, "");
                r.fail(ValidationStage::Shape, RejectReason::InvalidLayout, e.to_string());
                return Err(r);
            }
        };
        let rep = self.compile_wgsl(&id, &wgsl);
        if !rep.ok() {
            return Err(rep);
        }
        Ok(ResolvedShader {
            id,
            fell_back: false,
            wgsl,
            report: rep,
            chain: Vec::new(),
        })
    }

    /// Monadic fallback composition (T84): resolve preset A; on failure
    /// the *report* (not a silent fake) drives policy-based fallback to
    /// a known-good builtin — defaults to `black`.
    pub fn resolve_or_fallback(&mut self, req: &PresetRef) -> ResolvedShader {
        match self.resolve(req) {
            Ok(r) => r,
            Err(rep) => {
                let reason = rep.rejection.unwrap_or(RejectReason::KernelFailed);
                let target = self.fallback.target(reason);
                let next = PresetRef::Builtin { name: target };
                match self.resolve(&next) {
                    Ok(mut r) => {
                        r.fell_back = true;
                        r.chain.push(rep);
                        r
                    }
                    Err(rep2) => {
                        // last resort: the `black` builtin is compiled
                        // in — if IT fails the whole registry is broken;
                        // compose black's WGSL manually (never panic to
                        // audio path: callers get the module anyway).
                        let mut r = self
                            .resolve(&PresetRef::Builtin {
                                name: DEFAULT_FALLBACK.into(),
                            })
                            .unwrap_or_else(|rep3| ResolvedShader {
                                id: DEFAULT_FALLBACK.into(),
                                fell_back: true,
                                wgsl: Self::compose(
                                    &void_visual::shaders::generator_body("black")
                                        .unwrap_or_else(|| {
                                            "return vec4<f32>(0.0,0.0,0.0,1.0);".into()
                                        }),
                                ),
                                report: rep3,
                                chain: vec![],
                            });
                        r.fell_back = true;
                        r.chain.push(rep);
                        r.chain.push(rep2);
                        r
                    }
                }
            }
        }
    }

    /// Feed a runtime outcome back: timeouts quarantine; a failed
    /// report is minted (stage watchdog/kernel) and cached-invalidated.
    pub fn record_kernel_outcome(
        &mut self,
        id: &str,
        outcome: KernelOutcome,
    ) -> Result<CompileReport> {
        let Some(rec) = self.presets.get_mut(id) else {
            return Err(VisFxError::NotFound(id.into()));
        };
        let mut rep = rec
            .last_report
            .clone()
            .unwrap_or_else(|| CompileReport::new(id, ""));
        match outcome {
            KernelOutcome::Ok { wall_ns } => {
                rep.runtime_ns = Some(wall_ns);
            }
            KernelOutcome::Timeout { wall_ns } => {
                rep.runtime_ns = Some(wall_ns);
                rep.fail(
                    ValidationStage::Watchdog,
                    RejectReason::WatchdogTimeout,
                    format!("killed at {}ns", wall_ns),
                );
                rec.status = PresetStatus::Quarantined;
                if let Some(key) = rec.last_report.as_ref().map(|r| r.composed_sha256.clone()) {
                    self.cache.remove(&key);
                }
            }
            KernelOutcome::Cancelled => {
                rep.fail(
                    ValidationStage::Watchdog,
                    RejectReason::WatchdogCancelled,
                    "cancelled by coordinator".into(),
                );
            }
            KernelOutcome::Failed { detail_code } => {
                rep.fail(
                    ValidationStage::Kernel,
                    RejectReason::KernelFailed,
                    detail_code.into(),
                );
                rec.status = PresetStatus::Quarantined;
            }
        }
        rec.last_report = Some(rep.clone());
        self.persist()?;
        Ok(rep)
    }

    /// Invalidate a cached/verdict entry (e.g. driver bump) — status
    /// stays Active; next resolve recompiles fresh.
    pub fn invalidate(&mut self, id: &str) -> Result<()> {
        let Some(rec) = self.presets.get_mut(id) else {
            return Err(VisFxError::NotFound(id.into()));
        };
        if let Some(r) = rec.last_report.take() {
            self.cache.remove(&r.composed_sha256);
        }
        self.persist()
    }

    pub fn quarantine(&mut self, id: &str) -> Result<()> {
        let Some(rec) = self.presets.get_mut(id) else {
            return Err(VisFxError::NotFound(id.into()));
        };
        rec.status = PresetStatus::Quarantined;
        self.persist()
    }

    /// Begin a monadic chain for external composition.
    pub fn chain(&mut self, req: PresetRef) -> ShaderChain<'_> {
        let outcome = self.resolve(&req);
        ShaderChain {
            registry: self,
            outcome,
        }
    }

    pub fn preset_ids(&self) -> Vec<String> {
        self.presets.keys().cloned().collect()
    }
}
