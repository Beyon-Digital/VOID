//! Visual checkpoint payload — the snapshot that joins the project
//! checkpoint (`checkpoints/<id>/app-state.json` "visual" section, or a
//! companion `visual-state.json` alongside it). Canonical JSON encoding
//! (BTreeMap ordering via struct fields + sorted vectors) whose sha256 is
//! the `state_sha256` integrity pin on the wire.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::types::*;

pub const CHECKPOINT_FORMAT: &str = "void-visual-checkpoint";

/// Serializable scene snapshot (mirrors `VisualStateSnapshot` in the fbs).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualCheckpoint {
    pub format: String,
    /// Snapshot format revision (0 = draft rev0).
    pub version: u32,
    pub revision: u64,
    pub engine_epoch: u64,
    pub layers: Vec<Layer>,
    pub anchors: Vec<VisualAnchor>,
    pub transitions: Vec<TransitionState>,
    pub routes: Vec<OutputRoute>,
    pub tempo_map_revision: u64,
}

/// Deterministic JSON bytes — `serde_json::to_vec` on our types is
/// canonical because every map in the payload is a struct (fixed field
/// order) and vectors carry explicit order.
pub fn to_canonical_json(cp: &VisualCheckpoint) -> String {
    serde_json::to_string(cp).expect("checkpoint serialization is infallible")
}

pub fn payload_sha256(json: &str) -> String {
    crate::ops::hex(&Sha256::digest(json.as_bytes()))
}

pub fn from_json(json: &str) -> crate::error::Result<VisualCheckpoint> {
    let cp: VisualCheckpoint = serde_json::from_str(json).map_err(|e| {
        crate::error::VisualError::BadRequest(format!("bad checkpoint payload: {e}"))
    })?;
    if cp.format != CHECKPOINT_FORMAT {
        return Err(crate::error::VisualError::BadRequest(format!(
            "not a visual checkpoint: {}",
            cp.format
        )));
    }
    Ok(cp)
}
