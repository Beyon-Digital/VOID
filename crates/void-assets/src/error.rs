//! Asset error vocabulary. Errors are safe to surface to users: they never
//! contain raw filesystem paths beyond the offending archive entry name.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AssetError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("corrupt or unsupported media: {0}")]
    CorruptMedia(String),

    #[error("unsafe archive entry rejected: {0}")]
    UnsafeEntry(String),

    #[error("archive limit exceeded: {0}")]
    LimitExceeded(String),

    #[error("asset exceeds the configured size limit ({0} bytes)")]
    Oversized(u64),

    #[error("asset {0} is missing; relink placeholder required (no silent substitution)")]
    Missing(String),

    #[error("hash mismatch: expected {expected}, computed {actual}")]
    HashMismatch { expected: String, actual: String },

    #[error("archive decode error: {0}")]
    Archive(String),

    #[error("invalid asset reference: {0}")]
    InvalidRef(String),
}

pub type Result<T> = std::result::Result<T, AssetError>;
