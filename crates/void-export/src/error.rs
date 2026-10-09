//! void-export error vocabulary. User-safe: no raw paths or secrets beyond
//! the container file names defined by CONTRACTS.md §4/§7.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("asset store error: {0}")]
    Asset(#[from] void_assets::AssetError),

    #[error("project error: {0}")]
    Project(#[from] void_project::ProjectError),

    #[error("job error: {0}")]
    Job(#[from] void_jobs::JobError),

    #[error("invalid export spec: {0}")]
    InvalidSpec(String),

    #[error("renderer failed: {0}")]
    RendererFailed(String),

    #[error("export verification failed: {0}")]
    VerifyFailed(String),

    #[error("checkpoint input is not an immutable verified checkpoint: {0}")]
    CheckpointInvalid(String),

    #[error("export cancelled")]
    Cancelled,

    #[error("simulated crash at export boundary: {0}")]
    Failpoint(&'static str),
}

pub type Result<T> = std::result::Result<T, ExportError>;
