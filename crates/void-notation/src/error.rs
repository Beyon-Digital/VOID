//! Error vocabulary for void-notation.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum NotationError {
    /// Input that cannot be parsed (bad XML, malformed document).
    #[error("malformed: {0}")]
    Malformed(String),

    /// Well-formed input using a surface VOID does not support. Never
    /// silently degraded — the caller gets a typed refusal and the
    /// reason is recorded for the loss report where applicable.
    #[error("unsupported: {0}")]
    Unsupported(String),

    /// Structural validation problems. The whole list is returned, not
    /// the first failure — mirrors `ExchangeDocument::validate`.
    #[error("invalid score: {}", .0.join("; "))]
    Invalid(Vec<String>),

    /// An op referenced an element/part/measure that does not exist or
    /// cannot accept the mutation.
    #[error("op rejected: {0}")]
    OpRejected(String),

    /// quick-xml reader failure, stringified (the crate error type is
    /// not public-api stable across versions).
    #[error("xml: {0}")]
    Xml(String),
}

pub type Result<T> = std::result::Result<T, NotationError>;
