//! Typed sync-layer errors — malformed/misconfigured sync data is
//! rejected, never coerced.

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Error, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum SyncError {
    #[error("MTC: {0}")]
    Mtc(String),
    #[error("MIDI clock: {0}")]
    Clock(String),
    #[error("MMC: {0}")]
    Mmc(String),
    #[error("OSC: {0}")]
    Osc(String),
    #[error("DMX: {0}")]
    Dmx(String),
}
