use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProposalError {
    #[error("invalid proposal request: {0}")]
    InvalidRequest(String),
    #[error("malformed proposal document: {0}")]
    MalformedDocument(String),
    #[error("proposal {0} not found")]
    NotFound(String),
    #[error("illegal transition {from} → {to}")]
    InvalidTransition {
        from: &'static str,
        to: &'static str,
    },
    #[error("proposal is stale: {0}")]
    Stale(String),
    #[error("accept revalidation failed: {0}")]
    Revalidation(String),
    #[error("accept selection overlaps locked range at note indices {0:?}")]
    LockedCollision(Vec<usize>),
    #[error("job error: {0}")]
    Jobs(#[from] void_jobs::JobError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, ProposalError>;
