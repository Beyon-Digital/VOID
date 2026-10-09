//! W28 test fixtures: REAL `.wasm` modules produced at test time from
//! WAT via the `wat` crate (no prebuilt blobs, no fake modules) plus
//! manifest/package writers bound to the actual wasm bytes' sha256.
//!
//! The malicious fixtures (loop-forever, memory-bomb, denied imports)
//! are honest adversaries: each one exercises a real kill/reject path
//! in `void-wasm`.

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use void_wasm::{ModuleManifest, ModuleRegistry, ModuleSpec, RegistryEntry, Result, WasmError};

// ---------- WAT sources ----------

/// Shared prelude: memory export, bump allocator, ABI version.
const PRELUDE: &str = r#"
    (memory (export "memory") 1)
    (global $__alloc (mut i32) (i32.const 1024))
    (func (export "void_abi_version") (result i32) (i32.const 1))
    (func (export "void_alloc") (param i32) (result i32)
        (local $p i32)
        (local.set $p (global.get $__alloc))
        (global.set $__alloc
            (i32.and
                (i32.add (local.get $p) (i32.add (local.get 0) (i32.const 7)))
                (i32.const -8)))
        (local.get $p))
    (func (export "void_init") (result i32) (i32.const 0))
"#;

/// Shared param/state surface: 3 f64 params (freq, amp, detune) saved
/// in a 24-byte state blob so state migrates across versions.
const PARAM_STATE: &str = r#"
    (global $freq (mut f64) (f64.const 440.0))
    (global $amp (mut f64) (f64.const 0.5))
    (global $phase (mut f64) (f64.const 0.0))
    (func (export "void_param_count") (result i32) (i32.const 2))
    (func (export "void_param_get") (param i32) (result f64)
        (if (i32.eqz (local.get 0)) (then (return (global.get $freq))))
        (global.get $amp))
    (func (export "void_param_set") (param i32 f64) (result i32)
        (if (i32.eqz (local.get 0))
            (then (global.set $freq (local.get 1)) (return (i32.const 0))))
        (if (i32.eq (local.get 0) (i32.const 1))
            (then (global.set $amp (local.get 1)) (return (i32.const 0))))
        (i32.const 1))
    (func (export "void_state_len") (result i32) (i32.const 24))
    (func (export "void_state_save") (param i32) (result i32)
        (f64.store (local.get 0) (global.get $freq))
        (f64.store (i32.add (local.get 0) (i32.const 8)) (global.get $amp))
        (f64.store (i32.add (local.get 0) (i32.const 16)) (global.get $phase))
        (i32.const 24))
    (func (export "void_state_restore") (param i32 i32) (result i32)
        (if (i32.lt_u (local.get 1) (i32.const 24))
            (then (return (i32.const 1))))
        (global.set $freq (f64.load (local.get 0)))
        (global.set $amp (f64.load (i32.add (local.get 0) (i32.const 8))))
        (global.set $phase (f64.load (i32.add (local.get 0) (i32.const 16))))
        (i32.const 0))
"#;

/// `saw-gen` v1.0.0 — renders a real sawtooth into the host buffer.
pub fn saw_v1_wat() -> String {
    format!(
        r#"
(module
{PRELUDE}{PARAM_STATE}
    (func (export "void_render_off") (param i32 i32) (result i32)
        (local $i i32) (local $samp f64)
        (block $done
            (loop $lp
                (br_if $done (i32.ge_u (local.get $i) (local.get 1)))
                (local.set $samp
                    (f64.mul (global.get $amp)
                        (f64.sub (f64.mul (f64.const 2.0) (global.get $phase))
                                 (f64.const 1.0))))
                (f32.store
                    (i32.add (local.get 0) (i32.mul (local.get $i) (i32.const 4)))
                    (f32.demote_f64 (local.get $samp)))
                (global.set $phase
                    (f64.add (global.get $phase)
                             (f64.div (global.get $freq) (f64.const 48000.0))))
                (if (f64.ge (global.get $phase) (f64.const 1.0))
                    (then (global.set $phase
                        (f64.sub (global.get $phase) (f64.const 1.0)))))
                (local.set $i (i32.add (local.get $i) (i32.const 1)))
                (br $lp)))
        (local.get 1))
)
"#
    )
}

/// `saw-gen` v2.0.0 — same id, same 24B state layout, SQUARE render so
/// a hot reload proves state migration AND code swap (output differs).
pub fn saw_v2_wat() -> String {
    format!(
        r#"
(module
{PRELUDE}{PARAM_STATE}
    (func (export "void_render_off") (param i32 i32) (result i32)
        (local $i i32) (local $samp f64)
        (block $done
            (loop $lp
                (br_if $done (i32.ge_u (local.get $i) (local.get 1)))
                (local.set $samp
                    (select
                        (global.get $amp)
                        (f64.neg (global.get $amp))
                        (f64.lt (global.get $phase) (f64.const 0.5))))
                (f32.store
                    (i32.add (local.get 0) (i32.mul (local.get $i) (i32.const 4)))
                    (f32.demote_f64 (local.get $samp)))
                (global.set $phase
                    (f64.add (global.get $phase)
                             (f64.div (global.get $freq) (f64.const 48000.0))))
                (if (f64.ge (global.get $phase) (f64.const 1.0))
                    (then (global.set $phase
                        (f64.sub (global.get $phase) (f64.const 1.0)))))
                (local.set $i (i32.add (local.get $i) (i32.const 1)))
                (br $lp)))
        (local.get 1))
)
"#
    )
}

pub fn saw_v3_wat() -> String {
    r#"
(module
    (memory (export "memory") 1)
    (global $__alloc (mut i32) (i32.const 1024))
    (func (export "void_abi_version") (result i32) (i32.const 1))
    (func (export "void_alloc") (param i32) (result i32)
        (local $p i32)
        (local.set $p (global.get $__alloc))
        (global.set $__alloc
            (i32.and (i32.add (local.get $p) (i32.add (local.get 0) (i32.const 7)))
                     (i32.const -8)))
        (local.get $p))
    (func (export "void_init") (result i32) (i32.const 0))
    (func (export "void_param_count") (result i32) (i32.const 2))
    (func (export "void_param_get") (param i32) (result f64)
        (global.get $freq))
    (func (export "void_param_set") (param i32 f64) (result i32)
        (global.set $freq (local.get 1))
        (i32.const 0))
    (global $freq (mut f64) (f64.const 440.0))
    (global $amp (mut f64) (f64.const 0.5))
    (global $phase (mut f64) (f64.const 0.0))
    (func (export "void_state_len") (result i32) (i32.const 24))
    (func (export "void_state_save") (param i32) (result i32)
        (f64.store (local.get 0) (global.get $freq))
        (f64.store (i32.add (local.get 0) (i32.const 8)) (global.get $amp))
        (f64.store (i32.add (local.get 0) (i32.const 16)) (global.get $phase))
        (i32.const 24))
    ;; Hostile restore: advertises state support but never accepts a blob.
    (func (export "void_state_restore") (param i32 i32) (result i32)
        (i32.const 1))
    (func (export "void_render_off") (param i32 i32) (result i32)
        (local.get 1))
)
"#
    .to_string()
}

/// `spinner` — `void_render_off` loops forever. Fuel + deadline + cancel
/// all have to be able to kill it (T97).
pub fn spinner_wat() -> String {
    format!(
        r#"
(module
{PRELUDE}
    (func (export "void_render_off") (param i32 i32) (result i32)
        (loop $forever (br $forever))
        (i32.const 0))
)
"#
    )
}

/// `hog` — grows its linear memory inside `void_render_off` until the
/// host's ResourceLimiter refuses (typed MemoryLimitExceeded trap).
pub fn hog_wat() -> String {
    format!(
        r#"
(module
{PRELUDE}
    (func (export "void_render_off") (param i32 i32) (result i32)
        (local $n i32)
        (loop $lp
            (if (i32.eq (memory.grow (i32.const 1)) (i32.const -1))
                (then (return (local.get $n))))
            (local.set $n (i32.add (local.get $n) (i32.const 1)))
            (br $lp))
        (local.get $n))
)
"#
    )
}

/// `fatty` — declares a 512-page (32 MiB) minimum memory at parse time.
/// Any manifest that declares a lower memory limit must reject it
/// BEFORE instantiate (declared-min > declared-limit).
pub fn fatty_wat() -> String {
    r#"
(module
    (memory (export "memory") 512)
    (func (export "void_abi_version") (result i32) (i32.const 1))
    (func (export "void_init") (result i32) (i32.const 0))
    (func (export "void_render_off") (param i32 i32) (result i32)
        (local.get 1))
)
"#
    .to_string()
}

/// `sneak-net` — tries to import `env.http_get` (ambient network).
pub fn sneak_net_wat() -> String {
    r#"
(module
    (import "env" "http_get" (func $get (param i32) (result i32)))
    (memory (export "memory") 1)
    (func (export "void_abi_version") (result i32) (i32.const 1))
    (func (export "void_init") (result i32) (i32.const 0))
)
"#
    .to_string()
}

/// `sneak-fs` — tries to import a WASI fd_read (ambient filesystem).
pub fn sneak_fs_wat() -> String {
    r#"
(module
    (import "wasi_snapshot_preview1" "fd_read"
        (func $fd_read (param i32 i32 i32 i32) (result i32)))
    (memory (export "memory") 1)
    (func (export "void_abi_version") (result i32) (i32.const 1))
    (func (export "void_init") (result i32) (i32.const 0))
)
"#
    .to_string()
}

/// `talker` — uses the ONE granted import `void_host.log` during init.
pub fn talker_wat() -> String {
    r#"
(module
    (import "void_host" "log" (func $log (param i32 i32 i32)))
    (memory (export "memory") 1)
    (data (i32.const 16) "hello from wasm")
    (global $__alloc (mut i32) (i32.const 1024))
    (func (export "void_abi_version") (result i32) (i32.const 1))
    (func (export "void_alloc") (param i32) (result i32)
        (local $p i32)
        (local.set $p (global.get $__alloc))
        (global.set $__alloc
            (i32.and (i32.add (local.get $p) (i32.add (local.get 0) (i32.const 7)))
                     (i32.const -8)))
        (local.get $p))
    (func (export "void_init") (result i32)
        (call $log (i32.const 1) (i32.const 16) (i32.const 15))
        (i32.const 0))
)
"#
    .to_string()
}

/// `arp` — real midi-transform: duplicates every NoteEvent into the
/// original plus a +12 semitone clone 96000 ticks later.
pub fn arp_wat() -> String {
    format!(
        r#"
(module
{PRELUDE}
    (func $memcpy (param $dst i32) (param $src i32) (param $n i32)
        (local $i i32)
        (block $done
            (loop $lp
                (br_if $done (i32.ge_u (local.get $i) (local.get $n)))
                (i32.store8 (i32.add (local.get $dst) (local.get $i))
                            (i32.load8_u (i32.add (local.get $src) (local.get $i))))
                (local.set $i (i32.add (local.get $i) (i32.const 1)))
                (br $lp))))
    (func (export "void_midi_xform") (param i32 i32 i32 i32) (result i32)
        (local $i i32) (local $w i32)
        (block $done
            (loop $lp
                (br_if $done (i32.ge_u (local.get $i) (local.get 1)))
                (br_if $done
                    (i32.gt_u (i32.add (local.get $w) (i32.const 48))
                              (local.get 3)))
                ;; copy event verbatim at out+w
                (call $memcpy
                    (i32.add (local.get 2) (local.get $w))
                    (i32.add (local.get 0) (local.get $i))
                    (i32.const 24))
                ;; clone at out+w+24: onset += 96000, pitch += 12
                (call $memcpy
                    (i32.add (local.get 2) (i32.add (local.get $w) (i32.const 24)))
                    (i32.add (local.get 0) (local.get $i))
                    (i32.const 24))
                (i64.store
                    (i32.add (local.get 2) (i32.add (local.get $w) (i32.const 24)))
                    (i64.add (i64.load (i32.add (local.get 0) (local.get $i)))
                             (i64.const 96000)))
                (i32.store8
                    (i32.add (local.get 2) (i32.add (local.get $w) (i32.const 40)))
                    (i32.add
                        (i32.load8_u
                            (i32.add (local.get 0)
                                     (i32.add (local.get $i) (i32.const 16))))
                        (i32.const 12)))
                (local.set $w (i32.add (local.get $w) (i32.const 48)))
                (local.set $i (i32.add (local.get $i) (i32.const 24)))
                (br $lp)))
        (local.get $w))
)
"#
    )
}

/// `evil-xform` — writes a structurally invalid NoteEvent (pitch 200).
/// The host must reject malformed output, not pass it downstream.
pub fn evil_xform_wat() -> String {
    format!(
        r#"
(module
{PRELUDE}
    (func (export "void_midi_xform") (param i32 i32 i32 i32) (result i32)
        ;; one 24B event: onset 0, len 96000, pitch 200, vel 100
        (i64.store (local.get 2) (i64.const 0))
        (i64.store (i32.add (local.get 2) (i32.const 8)) (i64.const 96000))
        (i32.store8 (i32.add (local.get 2) (i32.const 16)) (i32.const 200))
        (i32.store8 (i32.add (local.get 2) (i32.const 17)) (i32.const 100))
        (i32.store8 (i32.add (local.get 2) (i32.const 18)) (i32.const 0))
        (i32.store8 (i32.add (local.get 2) (i32.const 19)) (i32.const 1))
        (i32.const 24))
)
"#
    )
}

/// `hoarder` — claims a 2 MiB state blob; a manifest that declares a
/// small state bound must reject it at init (StateTooLarge).
pub fn hoarder_wat() -> String {
    format!(
        r#"
(module
{PRELUDE}
    (func (export "void_state_len") (result i32) (i32.const 2097152))
    (func (export "void_state_save") (param i32) (result i32)
        (i32.const 2097152))
    (func (export "void_state_restore") (param i32 i32) (result i32)
        (i32.const 0))
)
"#
    )
}

/// `spoiled` — exports void_init but NOT void_alloc → AbiViolation.
pub fn spoiled_wat() -> String {
    r#"
(module
    (memory (export "memory") 1)
    (func (export "void_abi_version") (result i32) (i32.const 1))
    (func (export "void_init") (result i32) (i32.const 0))
)
"#
    .to_string()
}

// ---------- package helpers ----------

pub fn wasm_of(wat: impl AsRef<str>) -> Vec<u8> {
    wat::parse_str(wat.as_ref()).expect("fixture wat must compile")
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A full manifest JSON bound to real wasm bytes. Limits serialize as
/// STRING-int64 per the manifest schema / protocol DTO rule.
pub fn manifest_json(
    module_id: &str,
    name: &str,
    version: &str,
    wasm: &[u8],
    capabilities: &[&str],
    placement: &str,
    limits: &[(&str, &str)],
    ambient_requests: &[&str],
) -> String {
    let sha = hex(&Sha256::digest(wasm));
    let caps: Vec<String> = capabilities.iter().map(|c| format!("\"{c}\"")).collect();
    let amb: Vec<String> = ambient_requests
        .iter()
        .map(|c| format!("\"{c}\""))
        .collect();
    let lims: Vec<String> = limits
        .iter()
        .map(|(k, v)| format!("\"{k}\":\"{v}\""))
        .collect();
    format!(
        r#"{{"manifest_version":1,"module_id":"{module_id}","name":"{name}","version":"{version}","wasm_sha256":"{sha}","capabilities":[{}],"ambient_requests":[{}],"limits":{{{}}},"placement":"{placement}","abi":"void-wasm/1"}}"#,
        caps.join(","),
        amb.join(","),
        lims.join(",")
    )
}

/// Declared limits matching a typical sane module.
pub const STANDARD_LIMITS: &[(&str, &str)] = &[
    ("max_memory_bytes", "16777216"), // 16 MiB
    ("max_fuel", "10000000"),
    ("deadline_ms", "2000"),
    ("max_state_bytes", "65536"),
    ("max_out_bytes", "4194304"),
    ("max_params", "8"),
];

/// Write a spec package (manifest.json + module.wasm) into `dir`.
pub fn write_package(dir: &Path, manifest: &str, wasm: &[u8]) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("manifest.json"), manifest).unwrap();
    fs::write(dir.join("module.wasm"), wasm).unwrap();
}

/// Build + write a spec package, return the parsed spec.
pub fn make_spec(
    dir: &Path,
    module_id: &str,
    name: &str,
    version: &str,
    wat: impl AsRef<str>,
    capabilities: &[&str],
    placement: &str,
    limits: &[(&str, &str)],
) -> ModuleSpec {
    let wasm = wasm_of(wat.as_ref());
    let mj = manifest_json(
        module_id,
        name,
        version,
        &wasm,
        capabilities,
        placement,
        limits,
        &[],
    );
    write_package(dir, &mj, &wasm);
    ModuleSpec::from_dir(dir, &void_wasm::HostPolicy::default()).expect("spec must be valid")
}

/// Install a spec into a registry and return the entry.
pub fn install(reg: &mut ModuleRegistry, spec: &ModuleSpec) -> RegistryEntry {
    reg.install(spec).expect("install must succeed")
}

/// Parse a manifest straight from JSON (validation-path tests).
pub fn parse_manifest(json: &str) -> Result<ModuleManifest> {
    ModuleManifest::parse(json, &void_wasm::HostPolicy::default())
}

/// Assert an error is a specific variant by name string — keeps test
/// readouts honest without exporting match helpers.
pub fn err_name(e: &WasmError) -> &'static str {
    match e {
        WasmError::InvalidManifest(_) => "InvalidManifest",
        WasmError::InvalidModulePackage(_) => "InvalidModulePackage",
        WasmError::AmbientDenied(_) => "AmbientDenied",
        WasmError::ImportDenied { .. } => "ImportDenied",
        WasmError::MissingCapabilityExport { .. } => "MissingCapabilityExport",
        WasmError::CapabilityNotDeclared(_) => "CapabilityNotDeclared",
        WasmError::MemoryLimitExceeded { .. } => "MemoryLimitExceeded",
        WasmError::FuelExhausted { .. } => "FuelExhausted",
        WasmError::DeadlineExceeded(_) => "DeadlineExceeded",
        WasmError::Cancelled => "Cancelled",
        WasmError::AbiViolation(_) => "AbiViolation",
        WasmError::StateTooLarge { .. } => "StateTooLarge",
        WasmError::OutputOverflow { .. } => "OutputOverflow",
        WasmError::InitRejected(_) => "InitRejected",
        WasmError::GuestRejected { .. } => "GuestRejected",
        WasmError::Tampered { .. } => "Tampered",
        WasmError::DuplicateVersion(_) => "DuplicateVersion",
        WasmError::Revoked(_) => "Revoked",
        WasmError::NotFound(_) => "NotFound",
        WasmError::IntegrationGated(_) => "IntegrationGated",
        WasmError::HotReloadFailed { .. } => "HotReloadFailed",
        WasmError::Engine(_) => "Engine",
        WasmError::Io(_) => "Io",
        WasmError::Json(_) => "Json",
    }
}

/// Scratch dir for one spec package under a tempdir.
pub fn pkg_dir(base: &Path, tag: &str) -> PathBuf {
    let d = base.join(tag);
    fs::create_dir_all(&d).unwrap();
    d
}
