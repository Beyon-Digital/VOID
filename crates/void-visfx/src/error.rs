//! Error type for void-visfx — mirrors the codebase's thiserror style.

use serde::{Deserialize, Serialize};

#[derive(Debug, thiserror::Error)]
pub enum VisFxError {
    #[error("invalid scene-gen spec: {0}")]
    InvalidSpec(String),
    #[error("invalid scene document: {0}")]
    InvalidDocument(String),
    #[error("forbidden scene action/field: {0}")]
    Forbidden(String),
    #[error("record not found: {0}")]
    NotFound(String),
    #[error("invalid lifecycle transition {from} -> {to}")]
    InvalidTransition { from: &'static str, to: &'static str },
    #[error("stale: {0}")]
    Stale(String),
    #[error("revalidation failed: {0}")]
    Revalidation(String),
    #[error("shader rejected: {0}")]
    Shader(String),
    #[error("camera policy violation: {0}")]
    PolicyViolation(String),
    #[error("{0}")]
    Busy(String),
    #[error(transparent)]
    Job(#[from] void_jobs::JobError),
    #[error(transparent)]
    Asset(#[from] void_assets::AssetError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

pub type Result<T> = std::result::Result<T, VisFxError>;

/// Camera-policy violation kinds — surfaced so the coordinator can
/// refuse non-default privacy postures (T86).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyViolation {
    /// Raw camera frames/audio retained anywhere.
    RawRetention,
    /// Any upload path enabled — camera data never leaves the box.
    NetworkUpload,
    /// Unbounded landmark history retention.
    UnboundedLandmarkHistory,
    /// Missing/weak confidence threshold (would emit phantom controls).
    WeakConfidence,
    /// No tracking-loss timeout — could hold notes forever.
    NoLossTimeout,
}
