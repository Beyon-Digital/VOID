//! void-jobs error vocabulary.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum JobError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("job {0} not found")]
    NotFound(String),

    #[error("invalid transition {from} -> {to}")]
    InvalidTransition {
        from: &'static str,
        to: &'static str,
    },

    #[error("invalid job spec: {0}")]
    InvalidSpec(String),

    #[error("result discarded: {0}")]
    LateResult(String),

    // ---- W12 runner surface (additive) --------------------------------
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("asset store error: {0}")]
    Asset(#[from] void_assets::AssetError),

    #[error("worker spawn failed: {0}")]
    SpawnFailed(String),

    #[error("worker declared artifact outside staging: {0}")]
    EscapesStaging(String),

    #[error("worker declared artifact missing: {0}")]
    ArtifactNotFound(String),

    #[error("job was cancelled")]
    Cancelled,

    #[error("admission refused: {0}")]
    Busy(String),

    #[error("runtime not usable: {0}")]
    RuntimeUnusable(String),
}

pub type Result<T> = std::result::Result<T, JobError>;
