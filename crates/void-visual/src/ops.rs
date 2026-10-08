//! Visual op DTOs — serde mirror of `VisualPersistentOp` /
//! `VisualPersistentCommand` in protocol/visual/void_visual.fbs. The
//! FlatBuffers schema is the wire contract; these are the in-process
//! forms and the JSON surface for packages/void-studio op builders.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::VisualErrorCode;
use crate::types::{
    AnchorKind, BlendMode, GeneratorSpec, OutputTarget, QuantizeMode, TransitionKind,
    VisualChannel, VisualLayerKind, VisualTransform,
};

/// One persistent visual op — member names mirror the fbs union
/// (tagged-union JSON form `{"AddVisualLayerOp": {...}}`, exactly one
/// key, same as the control-schema convention).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VisualOp {
    SetLayerStackOp {
        layer_ids: Vec<String>,
        channel: VisualChannel,
    },
    AddVisualLayerOp {
        layer_id: String,
        kind: VisualLayerKind,
        name: String,
        index: i32,
        channel: VisualChannel,
        generator: Option<GeneratorSpec>,
    },
    RemoveVisualLayerOp {
        layer_id: String,
    },
    SetLayerNameOp {
        layer_id: String,
        name: String,
    },
    SetLayerVisibleOp {
        layer_id: String,
        visible: bool,
    },
    SetLayerBlendOp {
        layer_id: String,
        blend_mode: BlendMode,
    },
    SetLayerTransformOp {
        layer_id: String,
        transform: VisualTransform,
    },
    AttachVisualMediaOp {
        layer_id: String,
        asset_id: String,
        sha256: String,
        media_kind: String,
        rel_path: String,
        duration_ticks: i64,
    },
    SetLayerTrimOp {
        layer_id: String,
        in_ticks: i64,
        out_ticks: i64,
        offset_ticks: i64,
    },
    SetLayerFadeOp {
        layer_id: String,
        fade_in_ticks: i64,
        fade_out_ticks: i64,
    },
    SetVisualAnchorOp {
        anchor_id: String,
        kind: AnchorKind,
        position_ticks: i64,
        position_sample: i64,
        timecode_ns: i64,
    },
    RemoveVisualAnchorOp {
        anchor_id: String,
    },
    BindLayerAnchorOp {
        layer_id: String,
        in_anchor_id: String,
        out_anchor_id: String,
    },
    SetTransitionOp {
        channel: VisualChannel,
        kind: TransitionKind,
        duration_ticks: i64,
        quantize: QuantizeMode,
        wipe_angle: f32,
    },
    TakeTransitionOp {
        channel: VisualChannel,
    },
    CancelTransitionOp {
        channel: VisualChannel,
    },
    SetOutputRouteOp {
        channel: VisualChannel,
        target: OutputTarget,
        display_id: String,
        width: u32,
        height: u32,
        fps_num: u32,
        fps_den: u32,
        readback: bool,
    },
    SnapshotVisualStateOp {
        checkpoint_id: String,
    },
    /// Snapshot payload travels in `snapshot_json` — the canonical
    /// `VisualCheckpoint` serialization whose sha256 must equal
    /// `state_sha256`.
    RestoreVisualStateOp {
        checkpoint_id: String,
        state_sha256: String,
        snapshot_json: String,
    },
    ClearVisualSceneOp {},
    /// Joint audio/visual undo: rewinds this participant's ops that were
    /// committed under `transaction_id` ("" = latest transaction).
    VisualUndoOp {
        transaction_id: String,
    },
    VisualRedoOp {
        transaction_id: String,
    },
}

/// Wire-level command envelope (mirrors VisualPersistentCommand).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualCommand {
    pub command_id: String,
    /// One user gesture == one transaction (joint undo boundary).
    pub transaction_id: String,
    pub project_id: String,
    pub engine_epoch: u64,
    pub expected_revision: u64,
    pub op: VisualOp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VisualAckStatus {
    Applied,
    Duplicate,
    Rejected,
    OutcomeUnknown,
}

/// Command acknowledgement — mirrors void_control `CommandReceipt`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualReceipt {
    pub command_id: String,
    pub transaction_id: String,
    pub status: VisualAckStatus,
    pub error: VisualErrorCode,
    /// Resulting visual revision on APPLIED; current revision on a
    /// revision-conflict REJECTED.
    pub revision: u64,
    pub engine_epoch: u64,
    pub message: String,
    /// sha256 hex of the canonical op payload (duplicate detection).
    pub payload_hash: String,
}

impl VisualReceipt {
    pub fn applied(cmd: &VisualCommand, revision: u64) -> Self {
        Self {
            command_id: cmd.command_id.clone(),
            transaction_id: cmd.transaction_id.clone(),
            status: VisualAckStatus::Applied,
            error: VisualErrorCode::None,
            revision,
            engine_epoch: cmd.engine_epoch,
            message: String::new(),
            payload_hash: cmd.payload_hash(),
        }
    }

    pub fn rejected(
        cmd: &VisualCommand,
        error: VisualErrorCode,
        revision: u64,
        message: &str,
    ) -> Self {
        Self {
            command_id: cmd.command_id.clone(),
            transaction_id: cmd.transaction_id.clone(),
            status: VisualAckStatus::Rejected,
            error,
            revision,
            engine_epoch: cmd.engine_epoch,
            message: message.to_string(),
            payload_hash: cmd.payload_hash(),
        }
    }
}

impl VisualCommand {
    /// Canonical payload hash for idempotency (CONTRACTS.md §3):
    /// sha256 over the deterministic JSON encoding of the op.
    pub fn payload_hash(&self) -> String {
        let bytes = serde_json::to_vec(&self.op).unwrap_or_default();
        hex(&Sha256::digest(&bytes))
    }
}

pub fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}
