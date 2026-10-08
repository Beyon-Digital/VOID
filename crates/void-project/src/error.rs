//! Project-container error vocabulary. User-safe: no raw paths or secrets
//! beyond the project file names defined by CONTRACTS.md §4.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("asset store error: {0}")]
    Asset(#[from] void_assets::AssetError),

    #[error("container is not a .void project: {0}")]
    NotAProject(String),

    #[error("project format {found} needs migration (current {current})")]
    NeedsMigration { found: u32, current: u32 },

    #[error(
        "project format {found} is newer than supported {supported_max}; opened read-only / write refused"
    )]
    UnsupportedNewer { found: u32, supported_max: u32 },

    #[error("manifest invalid: {0}")]
    ManifestInvalid(String),

    #[error("checkpoint verification failed at {path}: {reason}")]
    VerifyFailed { path: String, reason: String },

    #[error("CURRENT pointer corrupt: {0}")]
    PointerCorrupt(String),

    #[error("unsafe container-relative path rejected: {0}")]
    UnsafePath(String),

    #[error("save aborted; no durable checkpoint was published")]
    SaveAborted,

    #[error("save failed during {step}: no durable checkpoint was published")]
    SaveFailed { step: &'static str },

    #[error("simulated crash at save boundary: {0}")]
    Failpoint(&'static str),

    #[error("operation requires a writable open; container is read-only")]
    ReadOnly,
}

pub type Result<T> = std::result::Result<T, ProjectError>;
