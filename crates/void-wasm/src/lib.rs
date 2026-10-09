//! void-wasm — W28 "Advanced synthesis and restricted extensions",
//! non-native half: the restricted WASM module host.
//!
//! A VOID extension module is **declarative**: `manifest.json`
//! (capabilities, limits, placement, sha256 bind) + `module.wasm`.
//! There is no native-code path — a module can never be an .so/.dll,
//! and no ambient power (`fs`, `net`, `env`, `spawn`, `clock`,
//! `threads`, `random`, `wasi`) is grantable: the manifest rejects
//! the request and the linker contains no such imports.
//!
//! Runtime guarantees (T97):
//!   * fuel metering kills deterministic runaway compute;
//!   * an epoch deadline (~1 ms ticks) kills time-runaway calls;
//!   * a cross-thread cancel token stops in-flight guest calls;
//!   * a resource limiter enforces the declared memory ceiling at
//!     instantiate AND grow, with typed `MemoryLimitExceeded`;
//!   * the registry is sha256-verified + versioned; revoke → unload;
//!   * hot reload carries state across versions and keeps the
//!     last-known-good instance on any failure.
//!
//! Modules run OUTSIDE the critical audio path — `placement` is a
//! declared constant and `rt_audio`/`audio_path` are unrepresentable
//! in the schema. The wire ops/read-views a coordinator would use to
//! drive this (`MODULE_LIST`, load/unload/reload/revoke) do not exist
//! in protocol major.1 — recorded in docs/wasm/NEEDS.md.
//!
//! SDK-gated native work (ARA, native plugin export — T98) is modelled
//! in `gates` and stays closed without evidence; see docs/wasm/adr/.
//!
//! Modules:
//!   * `capability` — grantable capabilities + never-grantable ambient
//!     set + the single host import surface;
//!   * `manifest` + `policy` + `strnum` — schema v1, policy ceilings,
//!     string-int64 boundary fields;
//!   * `spec` — package loader + static wasm scan (magic, imports,
//!     declared memory minimum);
//!   * `host` — wasmtime engine/store/linker, budgeted call wrappers,
//!     param/state/render_off/midi_xform ABI;
//!   * `registry` — sha256-verified versioned store with revoke;
//!   * `runtime` — live instances, hot reload, cancel, catalog;
//!   * `gates` — ARA / plugin-export rights gates (T98).

pub mod abi;
pub mod capability;
pub mod error;
pub mod gates;
pub mod host;
pub mod manifest;
pub mod policy;
pub mod registry;
pub mod runtime;
pub mod spec;
pub mod strnum;

pub use abi::{decode_events, encode_events, NoteEvent, ABI_NAME, ABI_VERSION, NOTE_EVENT_SIZE};
pub use capability::{granted_import, reject_ambient, AmbientRequest, Capability, Placement};
pub use error::{Result, WasmError};
pub use gates::{
    gate_register, require_open, try_qualify, GateRecord, GateStatus, GatedIntegration,
};
pub use host::{CancelToken, Host, LogRecord, ModuleInstance, MAX_MODULE_BYTES};
pub use manifest::{is_valid_version, sha256_hex, version_cmp, ModuleManifest, MANIFEST_VERSION};
pub use policy::{DeclaredLimits, HostPolicy, EPOCH_TICK_MS};
pub use registry::{EntryStatus, ModuleRegistry, RegistryEntry, RegistryRow};
pub use runtime::{LiveModule, ModuleRuntime, ReloadReport};
pub use spec::{scan_wasm, ModuleSpec};
