//! void-av error vocabulary. User-safe: no raw paths or secrets beyond
//! container file names and tool base names (CONTRACTS.md §3/§7).
//!
//! `CodecUnavailable` is a *typed* error — an absent ffmpeg or an absent
//! encoder is a structured outcome the caller can surface, never a fake
//! success and never an opaque renderer failure (T88).

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AvError {
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

    #[error("invalid av export spec: {0}")]
    InvalidSpec(String),

    /// Declarative codec/pipeline gap: the ffmpeg binary itself is
    /// missing, or the requested codec id is unknown/unavailable in the
    /// detected build, or its recorded rights block selection. Typed so
    /// the UI can show "codec X unavailable" instead of a generic fail.
    #[error("codec unavailable: {0}")]
    CodecUnavailable(String),

    /// The encoder subprocess ran and failed (non-zero exit, missing
    /// artifact, protocol breakage). Distinct from CodecUnavailable —
    /// the tool existed and was invoked.
    #[error("encoder failed: {0}")]
    EncoderFailed(String),

    #[error("av artifact verification failed: {0}")]
    VerifyFailed(String),

    /// Probe-back of the produced artifact failed (ffprobe could not run
    /// or its output did not parse). Verification cannot be skipped —
    /// an unprobed artifact never publishes.
    #[error("probe failed: {0}")]
    ProbeFailed(String),

    #[error("checkpoint input is not an immutable verified checkpoint: {0}")]
    CheckpointInvalid(String),

    /// Encoder exceeded its declared wall-clock bound and was killed.
    #[error("encoder timed out")]
    Timeout,

    /// Artifact grew past the declared output-size bound.
    #[error("output exceeds declared byte bound")]
    OutputLimitExceeded,

    #[error("av export cancelled")]
    Cancelled,

    #[error("simulated crash at av boundary: {0}")]
    Failpoint(&'static str),
}

pub type Result<T> = std::result::Result<T, AvError>;
