//! Error vocabulary for the exchange crate (W20).
//!
//! Distinct from the wire `ErrorCode` set in CONTRACTS.md §3 — these are
//! interchange-layer failures that never cross the audio path. Safe
//! messages only: no raw filesystem paths beyond the offending
//! container-relative name, no secrets, no native stack data.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ExchangeError {
    #[error("i/o: {0}")]
    Io(#[from] std::io::Error),

    #[error("zip container: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("xml: {0}")]
    Xml(String),

    #[error("malformed {0}")]
    Malformed(String),

    #[error("unsupported: {0}")]
    Unsupported(String),

    /// A referenced payload exceeded a declared bound (CONTRACTS.md §2
    /// bounded-resource discipline — archives are untrusted input).
    #[error("resource bound exceeded: {0}")]
    TooLarge(String),

    /// Container member path was absolute/traversing/otherwise unsafe.
    #[error("unsafe path: {0}")]
    UnsafePath(String),

    /// A manifest/payload hash did not match its declared sha256.
    #[error("sha256 mismatch for {0}")]
    DigestMismatch(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, ExchangeError>;
