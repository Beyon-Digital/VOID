//! void-models error vocabulary. Safe messages only — no raw paths.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ModelError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("asset error: {0}")]
    Asset(#[from] void_assets::AssetError),

    #[error("model {0} not found")]
    NotFound(String),

    #[error("invalid manifest: {0}")]
    InvalidManifest(String),

    #[error("manifest integrity check failed (declared {declared}, computed {actual})")]
    ManifestTampered { declared: String, actual: String },

    #[error("manifest requests disallowed capability: {0}")]
    DisallowedCapability(String),

    #[error("required model artifact {0} missing or corrupt")]
    MissingArtifact(String),
}

pub type Result<T> = std::result::Result<T, ModelError>;
