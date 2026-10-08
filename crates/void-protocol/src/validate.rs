//! Pre-apply validation (CONTRACTS.md §3 processing sequence):
//! authenticate -> frame/schema -> project/epoch -> revision -> targets.
//! This module covers the schema/value layer: IDs, finite floats, ranges,
//! version negotiation. Project/epoch/revision checks live in void-app.

use crate::ids::is_valid_id;
use crate::proto::{self, AckStatus, ErrorCode};
use crate::{PROTOCOL_MAJOR, PROTOCOL_MINOR};

#[derive(Debug, Clone)]
pub struct Rejection {
    pub code: ErrorCode,
    pub message: String,
}

fn reject(code: ErrorCode, msg: impl Into<String>) -> Rejection {
    Rejection {
        code,
        message: msg.into(),
    }
}

fn check_id(field: &str, value: &str) -> Result<(), Rejection> {
    if is_valid_id(value) {
        Ok(())
    } else {
        Err(reject(
            ErrorCode::BAD_REQUEST,
            format!("invalid {field} id"),
        ))
    }
}

fn check_finite(field: &str, v: f32) -> Result<(), Rejection> {
    if v.is_finite() {
        Ok(())
    } else {
        Err(reject(
            ErrorCode::BAD_REQUEST,
            format!("{field} must be finite"),
        ))
    }
}

/// Negotiate protocol versions from a WorkerHello.
pub fn negotiate(hello: proto::WorkerHello) -> Result<(), Rejection> {
    if hello.protocol_major() != PROTOCOL_MAJOR {
        return Err(reject(
            ErrorCode::UNSUPPORTED_VERSION,
            format!(
                "protocol major {} unsupported (we speak {}.{})",
                hello.protocol_major(),
                PROTOCOL_MAJOR,
                PROTOCOL_MINOR
            ),
        ));
    }
    Ok(())
}

/// Validate a persistent command's envelope + op payload values. Returns
/// the op variant name for hashing/logging on success.
pub fn validate_persistent_command(
    cmd: proto::PersistentCommand,
) -> Result<&'static str, Rejection> {
    check_id("command", cmd.command_id().unwrap_or_default())?;
    check_id("transaction", cmd.transaction_id().unwrap_or_default())?;
    check_id("project", cmd.project_id().unwrap_or_default())?;

    use proto::PersistentOp as Op;
    let variant = cmd.op_type().variant_name();
    match cmd.op_type() {
        Op::AddTrackOp => {
            let op = cmd.op_as_add_track_op().unwrap();
            check_id("track", op.track_id().unwrap_or_default())?;
        }
        Op::RemoveTrackOp => {
            check_id(
                "track",
                cmd.op_as_remove_track_op()
                    .unwrap()
                    .track_id()
                    .unwrap_or_default(),
            )?;
        }
        Op::SetTrackNameOp => {
            check_id(
                "track",
                cmd.op_as_set_track_name_op()
                    .unwrap()
                    .track_id()
                    .unwrap_or_default(),
            )?;
        }
        Op::SetTrackGainOp => {
            let op = cmd.op_as_set_track_gain_op().unwrap();
            check_id("track", op.track_id().unwrap_or_default())?;
            check_finite("gain_linear", op.gain_linear())?;
            if !(0.0..=4.0).contains(&op.gain_linear()) {
                return Err(reject(
                    ErrorCode::BAD_REQUEST,
                    "gain out of supported range",
                ));
            }
        }
        Op::SetTrackPanOp => {
            let op = cmd.op_as_set_track_pan_op().unwrap();
            check_id("track", op.track_id().unwrap_or_default())?;
            check_finite("pan", op.pan())?;
            if !(-1.0..=1.0).contains(&op.pan()) {
                return Err(reject(ErrorCode::BAD_REQUEST, "pan outside -1..1"));
            }
        }
        Op::SetTrackMuteOp => {
            check_id(
                "track",
                cmd.op_as_set_track_mute_op()
                    .unwrap()
                    .track_id()
                    .unwrap_or_default(),
            )?;
        }
        Op::SetTrackSoloOp => {
            check_id(
                "track",
                cmd.op_as_set_track_solo_op()
                    .unwrap()
                    .track_id()
                    .unwrap_or_default(),
            )?;
        }
        Op::InsertAudioClipOp => {
            let op = cmd.op_as_insert_audio_clip_op().unwrap();
            check_id("clip", op.clip_id().unwrap_or_default())?;
            check_id("track", op.track_id().unwrap_or_default())?;
            check_id("asset", op.asset_id().unwrap_or_default())?;
            if op.length_ticks() <= 0 {
                return Err(reject(
                    ErrorCode::BAD_REQUEST,
                    "clip length must be positive",
                ));
            }
        }
        Op::InsertMidiClipOp => {
            let op = cmd.op_as_insert_midi_clip_op().unwrap();
            check_id("clip", op.clip_id().unwrap_or_default())?;
            check_id("track", op.track_id().unwrap_or_default())?;
            if op.length_ticks() <= 0 {
                return Err(reject(
                    ErrorCode::BAD_REQUEST,
                    "clip length must be positive",
                ));
            }
        }
        Op::RemoveClipOp => {
            check_id(
                "clip",
                cmd.op_as_remove_clip_op()
                    .unwrap()
                    .clip_id()
                    .unwrap_or_default(),
            )?;
        }
        Op::MoveClipOp => {
            let op = cmd.op_as_move_clip_op().unwrap();
            check_id("clip", op.clip_id().unwrap_or_default())?;
            check_id("track", op.track_id().unwrap_or_default())?;
        }
        Op::TrimClipOp => {
            let op = cmd.op_as_trim_clip_op().unwrap();
            check_id("clip", op.clip_id().unwrap_or_default())?;
            if op.length_ticks() <= 0 {
                return Err(reject(
                    ErrorCode::BAD_REQUEST,
                    "clip length must be positive",
                ));
            }
        }
        Op::SplitClipOp => {
            let op = cmd.op_as_split_clip_op().unwrap();
            check_id("clip", op.clip_id().unwrap_or_default())?;
            check_id("new_clip", op.new_clip_id().unwrap_or_default())?;
        }
        Op::InsertNoteOp => {
            let op = cmd.op_as_insert_note_op().unwrap();
            check_id("clip", op.clip_id().unwrap_or_default())?;
            check_id("note", op.note_id().unwrap_or_default())?;
            if op.pitch() > 127 {
                return Err(reject(ErrorCode::BAD_REQUEST, "pitch out of range"));
            }
            if op.velocity() == 0 || op.velocity() > 127 {
                return Err(reject(ErrorCode::BAD_REQUEST, "velocity out of range"));
            }
            if op.length_ticks() <= 0 {
                return Err(reject(
                    ErrorCode::BAD_REQUEST,
                    "note length must be positive",
                ));
            }
        }
        Op::RemoveNoteOp => {
            let op = cmd.op_as_remove_note_op().unwrap();
            check_id("clip", op.clip_id().unwrap_or_default())?;
            check_id("note", op.note_id().unwrap_or_default())?;
        }
        Op::SetNoteOp => {
            let op = cmd.op_as_set_note_op().unwrap();
            check_id("clip", op.clip_id().unwrap_or_default())?;
            check_id("note", op.note_id().unwrap_or_default())?;
            if op.pitch() > 127 || op.velocity() > 127 || op.velocity() == 0 {
                return Err(reject(ErrorCode::BAD_REQUEST, "note field out of range"));
            }
        }
        Op::SetTempoOp => {
            let op = cmd.op_as_set_tempo_op().unwrap();
            check_finite("bpm", op.bpm())?;
            if !(1.0..=999.0).contains(&op.bpm()) {
                return Err(reject(ErrorCode::BAD_REQUEST, "bpm out of range"));
            }
        }
        Op::SetTimeSignatureOp => {
            let op = cmd.op_as_set_time_signature_op().unwrap();
            let denom = op.denominator();
            if op.numerator() == 0 || denom == 0 || !denom.is_power_of_two() {
                return Err(reject(ErrorCode::BAD_REQUEST, "invalid time signature"));
            }
        }
        Op::AttachAssetOp => {
            let op = cmd.op_as_attach_asset_op().unwrap();
            check_id("asset", op.asset_id().unwrap_or_default())?;
            let rel = op.rel_path().unwrap_or_default();
            if rel.is_empty() || rel.starts_with('/') || rel.contains("..") || rel.contains('\\') {
                return Err(reject(
                    ErrorCode::BAD_REQUEST,
                    "asset path must be container-relative, no traversal",
                ));
            }
        }
        Op::InsertPluginOp => {
            let op = cmd.op_as_insert_plugin_op().unwrap();
            check_id("track", op.track_id().unwrap_or_default())?;
            check_id(
                "plugin_instance",
                op.plugin_instance_id().unwrap_or_default(),
            )?;
        }
        Op::RemovePluginOp => {
            check_id(
                "plugin_instance",
                cmd.op_as_remove_plugin_op()
                    .unwrap()
                    .plugin_instance_id()
                    .unwrap_or_default(),
            )?;
        }
        Op::SetPluginParamOp => {
            let op = cmd.op_as_set_plugin_param_op().unwrap();
            check_id(
                "plugin_instance",
                op.plugin_instance_id().unwrap_or_default(),
            )?;
            check_finite("value", op.value())?;
        }
        Op::OpenPluginEditorOp => {
            check_id(
                "plugin_instance",
                cmd.op_as_open_plugin_editor_op()
                    .unwrap()
                    .plugin_instance_id()
                    .unwrap_or_default(),
            )?;
        }
        Op::ClosePluginEditorOp => {
            check_id(
                "plugin_instance",
                cmd.op_as_close_plugin_editor_op()
                    .unwrap()
                    .plugin_instance_id()
                    .unwrap_or_default(),
            )?;
        }
        // Ops with no numeric/ID fields to check still pass through.
        Op::CreateProjectOp
        | Op::OpenProjectOp
        | Op::CloseProjectOp
        | Op::SaveProjectOp
        | Op::CreateCheckpointOp
        | Op::UndoOp
        | Op::RedoOp
        | Op::SetLoopRangeOp => {}
        Op::NONE => {
            return Err(reject(ErrorCode::BAD_REQUEST, "empty operation"));
        }
        _ => {
            return Err(reject(
                ErrorCode::UNSUPPORTED_CAPABILITY,
                format!("operation {variant:?} not supported by this build"),
            ));
        }
    }
    Ok(variant.unwrap_or("UNKNOWN"))
}

/// Build a rejection receipt body for callers (status always REJECTED).
pub fn reject_status() -> AckStatus {
    AckStatus::REJECTED
}
