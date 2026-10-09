//! Error vocabulary for void-content (W19).
//!
//! Mirrors the CONTRACTS.md §3 spirit: bounded, machine-readable reasons —
//! never raw paths/secrets in messages that cross a boundary.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ContentError {
    #[error("manifest invalid: {0}")]
    InvalidManifest(String),

    #[error("manifest tampered: declared {declared}, actual {actual}")]
    ManifestTampered { declared: String, actual: String },

    #[error("file tampered: {path} (declared {declared}, actual {actual})")]
    FileTampered {
        path: String,
        declared: String,
        actual: String,
    },

    #[error("file missing: {0}")]
    FileMissing(String),

    #[error("pack not found: {0}")]
    PackNotFound(String),

    #[error("pack already installed: {pack_id}@{version}")]
    PackExists { pack_id: String, version: String },

    #[error("pack version regression: installed {installed}, candidate {candidate}")]
    VersionRegression {
        installed: String,
        candidate: String,
    },

    #[error("hash mismatch: expected {expected}, actual {actual}")]
    HashMismatch { expected: String, actual: String },

    #[error("content is not missing — relink refused")]
    NotMissing,

    #[error("voice budget exceeded: {active} active of {budget}")]
    VoiceBudgetExceeded { active: u32, budget: u32 },

    #[error("cache budget exceeded: {required} bytes required, {budget} budget")]
    CacheBudgetExceeded { required: u64, budget: u64 },

    #[error("zone invalid: {0}")]
    InvalidZone(String),

    #[error("unsafe path: {0}")]
    UnsafePath(String),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Asset(#[from] void_assets::AssetError),
}

pub type Result<T> = std::result::Result<T, ContentError>;
