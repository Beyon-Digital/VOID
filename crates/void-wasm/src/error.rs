//! void-wasm error vocabulary — mirrors sibling crates' thiserror
//! style; every variant carries a safe message (no raw paths/secrets).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum WasmError {
    /// Manifest JSON could not be parsed or failed schema validation.
    #[error("invalid manifest: {0}")]
    InvalidManifest(String),
    /// The package bytes are not a WebAssembly module (e.g. native code
    /// — modules are wasm+manifest only, never arbitrary native code).
    #[error("invalid module package: {0}")]
    InvalidModulePackage(String),
    /// A module/member asked for a capability that is never grantable.
    #[error("ambient capability is never grantable: {0}")]
    AmbientDenied(String),
    /// Module imports something outside the narrow `void_host` surface.
    #[error("import denied: {module}.{name}")]
    ImportDenied { module: String, name: String },
    /// Declared capability lacks the exports the ABI requires for it.
    #[error("capability `{capability}` declared but required export `{export}` missing")]
    MissingCapabilityExport { capability: String, export: String },
    /// Capability-gated call attempted on a module that didn't declare it.
    #[error("capability `{0}` not declared by module")]
    CapabilityNotDeclared(String),
    /// Declared linear-memory minimum exceeds the granted limit.
    #[error("memory limit exceeded: requested {requested} bytes > limit {limit} bytes")]
    MemoryLimitExceeded { requested: usize, limit: usize },
    /// Guest call consumed its fuel budget — deterministic kill.
    #[error("fuel exhausted (budget {budget})")]
    FuelExhausted { budget: u64 },
    /// Guest call exceeded its wall-clock epoch deadline.
    #[error("deadline exceeded ({0} ms)")]
    DeadlineExceeded(u64),
    /// Guest call was cancelled through the module's cancel token.
    #[error("call cancelled")]
    Cancelled,
    /// ABI shape violation (missing/ill-typed export, bad abi version).
    #[error("abi violation: {0}")]
    AbiViolation(String),
    /// State blob exceeded the declared bound.
    #[error("state too large: {actual} bytes > bound {bound}")]
    StateTooLarge { actual: usize, bound: usize },
    /// Module reported an output length beyond the granted buffer.
    #[error("output overflow: module claimed {claimed} bytes > granted {granted}")]
    OutputOverflow { claimed: usize, granted: usize },
    /// Guest init rejected the module (nonzero void_init).
    #[error("module init rejected: void_init returned {0}")]
    InitRejected(i32),
    /// Guest rejected a param write or state restore (nonzero status).
    #[error("module rejected call `{name}` with status {status}")]
    GuestRejected { name: &'static str, status: i32 },
    /// Registry bytes disagree with the recorded hashes — tampering.
    #[error("registry tamper detected on {what}: expected sha256 {expected}, got {actual}")]
    Tampered {
        what: String,
        expected: String,
        actual: String,
    },
    /// Install of an already-present module@version.
    #[error("duplicate module version: {0}")]
    DuplicateVersion(String),
    /// Module is revoked — nothing revoked may instantiate.
    #[error("module revoked: {0}")]
    Revoked(String),
    /// Module/version not found in the registry.
    #[error("not found: {0}")]
    NotFound(String),
    /// A native/SDK integration was requested while its rights gate is
    /// closed (ARA, plugin export — see docs/wasm ADRs).
    #[error("native integration gated: {0}")]
    IntegrationGated(&'static str),
    /// Hot reload failed; the last-known-good instance was kept.
    #[error("hot reload failed, last-known-good kept: {0}")]
    HotReloadFailed(String),
    /// wasmtime engine/compile/instantiate/trap error surface.
    #[error("engine: {0}")]
    Engine(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, WasmError>;
