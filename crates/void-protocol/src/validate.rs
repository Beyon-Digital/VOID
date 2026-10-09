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

/// Bounded non-empty string for identifiers that are not coordinator UUIDs
/// (engine-issued proposal ids, model slugs, device ids, scene labels).
fn check_nonempty(field: &str, value: &str) -> Result<(), Rejection> {
    if !value.is_empty() && value.len() <= 128 && !value.contains('\0') {
        Ok(())
    } else {
        Err(reject(ErrorCode::BAD_REQUEST, format!("invalid {field}")))
    }
}

fn check_sha256(field: &str, value: &str) -> Result<(), Rejection> {
    if value.len() == 64 && value.bytes().all(|b| b.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(reject(
            ErrorCode::BAD_REQUEST,
            format!("{field} must be a hex sha256"),
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
        // -- rev-2 (minor 1) -------------------------------------------------
        Op::ArmTrackOp => {
            let op = cmd.op_as_arm_track_op().unwrap();
            check_id("track", op.track_id().unwrap_or_default())?;
        }
        Op::StartRecordingOp => {
            let op = cmd.op_as_start_recording_op().unwrap();
            // take_id empty = engine mints one; when present it must be an id.
            let take = op.take_id().unwrap_or_default();
            if !take.is_empty() {
                check_id("take", take)?;
            }
        }
        Op::SetCountInOp => {
            let op = cmd.op_as_set_count_in_op().unwrap();
            match op.mode().unwrap_or_default() {
                "off" => {}
                "bars" if (1..=16).contains(&op.bars()) => {}
                _ => {
                    return Err(reject(
                        ErrorCode::BAD_REQUEST,
                        "count-in mode must be \"off\" or \"bars\" (1..=16)",
                    ));
                }
            }
        }
        Op::SetMetronomeOp => {
            let op = cmd.op_as_set_metronome_op().unwrap();
            check_finite("gain", op.gain())?;
            if !(0.0..=1.0).contains(&op.gain()) {
                return Err(reject(ErrorCode::BAD_REQUEST, "metronome gain 0..1"));
            }
        }
        Op::SetPunchInOutOp => {
            let op = cmd.op_as_set_punch_in_out_op().unwrap();
            if op.enabled() && (op.in_ticks() < 0 || op.out_ticks() <= op.in_ticks()) {
                return Err(reject(
                    ErrorCode::BAD_REQUEST,
                    "punch range requires 0 <= in < out",
                ));
            }
        }
        Op::SubmitJobOp => {
            let op = cmd.op_as_submit_job_op().unwrap();
            let spec = op
                .spec()
                .ok_or_else(|| reject(ErrorCode::BAD_REQUEST, "job spec required"))?;
            check_id("job", spec.job_id().unwrap_or_default())?;
            check_id("project", spec.project_id().unwrap_or_default())?;
            match spec.kind().unwrap_or_default() {
                "symbolic" | "transcription" | "separation" | "audio_generation"
                | "visual_generation" | "analysis" | "av_export" => {}
                _ => {
                    return Err(reject(ErrorCode::BAD_REQUEST, "unknown job kind"));
                }
            }
            check_nonempty("runtime_id", spec.runtime_id().unwrap_or_default())?;
            check_sha256("context_sha256", spec.context_sha256().unwrap_or_default())?;
            if spec.cpu_threads() < 0 {
                return Err(reject(ErrorCode::BAD_REQUEST, "cpu_threads < 0"));
            }
            if let Some(inputs) = spec.inputs() {
                if inputs.len() > 1024 {
                    return Err(reject(ErrorCode::BAD_REQUEST, "too many job inputs"));
                }
            }
            if spec.parameters_json().unwrap_or_default().len() > 256 * 1024 {
                return Err(reject(ErrorCode::BAD_REQUEST, "parameters_json too large"));
            }
        }
        Op::CancelJobOp => {
            check_id(
                "job",
                cmd.op_as_cancel_job_op()
                    .unwrap()
                    .job_id()
                    .unwrap_or_default(),
            )?;
        }
        Op::PauseJobOp => {
            check_id(
                "job",
                cmd.op_as_pause_job_op()
                    .unwrap()
                    .job_id()
                    .unwrap_or_default(),
            )?;
        }
        Op::InstallModelOp => {
            let op = cmd.op_as_install_model_op().unwrap();
            check_nonempty("model_id", op.model_id().unwrap_or_default())?;
            check_nonempty("model_version", op.model_version().unwrap_or_default())?;
            check_nonempty("source_uri", op.source_uri().unwrap_or_default())?;
        }
        Op::RequestProposalOp => {
            let op = cmd.op_as_request_proposal_op().unwrap();
            check_nonempty("context_digest", op.context_digest().unwrap_or_default())?;
            if !(1..=8).contains(&op.max_proposals()) {
                return Err(reject(ErrorCode::BAD_REQUEST, "max_proposals 1..=8"));
            }
        }
        Op::ResolveProposalOp => {
            let op = cmd.op_as_resolve_proposal_op().unwrap();
            check_nonempty("proposal_id", op.proposal_id().unwrap_or_default())?;
            if op.candidate_rank() < -1 {
                return Err(reject(ErrorCode::BAD_REQUEST, "candidate_rank < -1"));
            }
        }
        Op::PreviewLayerOp => {
            let op = cmd.op_as_preview_layer_op().unwrap();
            check_nonempty("proposal_id", op.proposal_id().unwrap_or_default())?;
            check_nonempty("clip_id", op.clip_id().unwrap_or_default())?;
            let notes = op.notes();
            if notes.as_ref().map(|n| n.len()).unwrap_or(0) > 4096 {
                return Err(reject(ErrorCode::BAD_REQUEST, "preview note count > 4096"));
            }
            if let Some(notes) = notes {
                for n in notes {
                    if n.pitch() > 127
                        || n.velocity() == 0
                        || n.velocity() > 127
                        || n.length_ticks() <= 0
                        || n.start_ticks() < 0
                    {
                        return Err(reject(ErrorCode::BAD_REQUEST, "preview note out of range"));
                    }
                }
            }
        }
        Op::IngestAssetOp => {
            let op = cmd.op_as_ingest_asset_op().unwrap();
            let rel = op.rel_path().unwrap_or_default();
            if rel.is_empty() || rel.contains('\0') {
                return Err(reject(ErrorCode::BAD_REQUEST, "rel_path empty or invalid"));
            }
            check_nonempty("media_type", op.media_type().unwrap_or_default())?;
        }
        Op::RelinkAssetOp => {
            let op = cmd.op_as_relink_asset_op().unwrap();
            check_id("asset", op.asset_id().unwrap_or_default())?;
            check_sha256("sha256", op.sha256().unwrap_or_default())?;
        }
        Op::SetPluginBypassOp => {
            check_id(
                "plugin_instance",
                cmd.op_as_set_plugin_bypass_op()
                    .unwrap()
                    .plugin_instance_id()
                    .unwrap_or_default(),
            )?;
        }
        Op::RescanPluginsOp => {
            // empty plugin_uid = full rescan; otherwise just bound the length.
            let uid = cmd
                .op_as_rescan_plugins_op()
                .unwrap()
                .plugin_uid()
                .unwrap_or_default();
            if uid.len() > 128 {
                return Err(reject(ErrorCode::BAD_REQUEST, "plugin_uid too long"));
            }
        }
        Op::RestorePluginStateOp => {
            let op = cmd.op_as_restore_plugin_state_op().unwrap();
            check_id(
                "plugin_instance",
                op.plugin_instance_id().unwrap_or_default(),
            )?;
            check_id("state_asset", op.state_asset_id().unwrap_or_default())?;
        }
        Op::SaveProjectAsOp => {
            let op = cmd.op_as_save_project_as_op().unwrap();
            let dir = op.container_dir().unwrap_or_default();
            if dir.is_empty() || dir.contains('\0') {
                return Err(reject(
                    ErrorCode::BAD_REQUEST,
                    "container_dir empty or invalid",
                ));
            }
        }
        Op::LaunchSceneOp => {
            let op = cmd.op_as_launch_scene_op().unwrap();
            check_nonempty("scene_id", op.scene_id().unwrap_or_default())?;
            if op.quantize() == proto::LaunchQuantize::CUSTOM && op.quantize_ticks() <= 0 {
                return Err(reject(
                    ErrorCode::BAD_REQUEST,
                    "CUSTOM quantize needs quantize_ticks > 0",
                ));
            }
        }
        Op::StopSceneOp => {
            let op = cmd.op_as_stop_scene_op().unwrap();
            // empty scene_id = stop all; otherwise must be present and bounded.
            let scene = op.scene_id().unwrap_or_default();
            if !scene.is_empty() {
                check_nonempty("scene_id", scene)?;
            }
            if op.quantize() == proto::LaunchQuantize::CUSTOM && op.quantize_ticks() <= 0 {
                return Err(reject(
                    ErrorCode::BAD_REQUEST,
                    "CUSTOM quantize needs quantize_ticks > 0",
                ));
            }
        }
        Op::LaunchClipOp => {
            let op = cmd.op_as_launch_clip_op().unwrap();
            check_nonempty("slot_id", op.slot_id().unwrap_or_default())?;
            if op.quantize() == proto::LaunchQuantize::CUSTOM && op.quantize_ticks() <= 0 {
                return Err(reject(
                    ErrorCode::BAD_REQUEST,
                    "CUSTOM quantize needs quantize_ticks > 0",
                ));
            }
        }
        Op::StopRecordingOp => {}
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
