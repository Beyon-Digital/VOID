//! void-producer error vocabulary — mirrors the other crates' thiserror
//! style; every variant carries a safe message (no raw paths/secrets).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProducerError {
    #[error("invalid spec: {0}")]
    InvalidSpec(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
    #[error("locked/protected region invariant violated: {0}")]
    LockedViolation(String),
    #[error("malformed document: {0}")]
    MalformedDocument(String),
    #[error("invalid transition: {from} -> {to}")]
    InvalidTransition {
        from: &'static str,
        to: &'static str,
    },
    #[error("record not found: {0}")]
    NotFound(String),
    #[error("revalidation failed: {0}")]
    Revalidation(String),
    #[error("stale proposal: {0}")]
    Stale(String),
    #[error("pcm decode failed: {0}")]
    Pcm(String),
    #[error("consolidation failed: {0}")]
    Consolidation(String),
    #[error("import plan failed: {0}")]
    Import(String),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("proposal layer: {0}")]
    Proposals(#[from] void_proposals::ProposalError),
    #[error("asset layer: {0}")]
    Assets(#[from] void_assets::AssetError),
    #[error("export layer: {0}")]
    Export(#[from] void_export::ExportError),
    #[error("job layer: {0}")]
    Jobs(#[from] void_jobs::JobError),
}

pub type Result<T> = std::result::Result<T, ProducerError>;
