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
}

pub type Result<T> = std::result::Result<T, JobError>;
