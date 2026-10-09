//! Error vocabulary for the visual lane — mirrors the ErrorCode numbering
//! of `void_control.fbs`/`voidvis` schema values 1:1 for shared codes and
//! adds visual-specific codes in the 100+ range.

use thiserror::Error;

/// Wire-level error codes (must match `VisualErrorCode` in
/// protocol/visual/void_visual.fbs). Serialized as its u16 discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[repr(u16)]
#[serde(try_from = "u16", into = "u16")]
pub enum VisualErrorCode {
    None = 0,
    BadRequest = 1,
    UnsupportedVersion = 2,
    UnsupportedCapability = 3,
    NotAuthorized = 4,
    NotFound = 5,
    StaleRevision = 6,
    StaleEpoch = 7,
    CommandIdReuse = 8,
    Busy = 9,
    DeviceUnavailable = 10,
    AssetMissing = 11,
    PluginUnavailable = 12,
    DiskFull = 13,
    WorkerFailed = 14,
    Cancelled = 15,
    OutcomeUnknown = 16,
    DecodeFailed = 100,
    NoAdapter = 101,
    OutputLost = 102,
}

#[derive(Debug, Error)]
pub enum VisualError {
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("stale revision: expected {expected}, current {current}")]
    StaleRevision { expected: u64, current: u64 },
    #[error("stale engine epoch {0}")]
    StaleEpoch(u64),
    #[error("command id reuse with different payload: {0}")]
    CommandIdReuse(String),
    #[error("busy: pending queue full")]
    Busy,
    #[error("device unavailable: {0}")]
    DeviceUnavailable(String),
    #[error("asset missing: {0}")]
    AssetMissing(String),
    #[error("worker failed: {0}")]
    WorkerFailed(String),
    #[error("decode failed: {0}")]
    DecodeFailed(String),
    #[error("no usable GPU adapter")]
    NoAdapter,
    #[error("output lost: {0}")]
    OutputLost(String),
}

pub type Result<T> = std::result::Result<T, VisualError>;

impl VisualError {
    pub fn code(&self) -> VisualErrorCode {
        match self {
            Self::BadRequest(_) => VisualErrorCode::BadRequest,
            Self::NotFound(_) => VisualErrorCode::NotFound,
            Self::StaleRevision { .. } => VisualErrorCode::StaleRevision,
            Self::StaleEpoch(_) => VisualErrorCode::StaleEpoch,
            Self::CommandIdReuse(_) => VisualErrorCode::CommandIdReuse,
            Self::Busy => VisualErrorCode::Busy,
            Self::DeviceUnavailable(_) => VisualErrorCode::DeviceUnavailable,
            Self::AssetMissing(_) => VisualErrorCode::AssetMissing,
            Self::WorkerFailed(_) => VisualErrorCode::WorkerFailed,
            Self::DecodeFailed(_) => VisualErrorCode::DecodeFailed,
            Self::NoAdapter => VisualErrorCode::NoAdapter,
            Self::OutputLost(_) => VisualErrorCode::OutputLost,
        }
    }
}

impl From<VisualErrorCode> for u16 {
    fn from(c: VisualErrorCode) -> u16 {
        c as u16
    }
}

impl TryFrom<u16> for VisualErrorCode {
    type Error = String;
    fn try_from(v: u16) -> std::result::Result<Self, String> {
        Ok(match v {
            0 => Self::None,
            1 => Self::BadRequest,
            2 => Self::UnsupportedVersion,
            3 => Self::UnsupportedCapability,
            4 => Self::NotAuthorized,
            5 => Self::NotFound,
            6 => Self::StaleRevision,
            7 => Self::StaleEpoch,
            8 => Self::CommandIdReuse,
            9 => Self::Busy,
            10 => Self::DeviceUnavailable,
            11 => Self::AssetMissing,
            12 => Self::PluginUnavailable,
            13 => Self::DiskFull,
            14 => Self::WorkerFailed,
            15 => Self::Cancelled,
            16 => Self::OutcomeUnknown,
            100 => Self::DecodeFailed,
            101 => Self::NoAdapter,
            102 => Self::OutputLost,
            other => return Err(format!("unknown visual error code {other}")),
        })
    }
}
