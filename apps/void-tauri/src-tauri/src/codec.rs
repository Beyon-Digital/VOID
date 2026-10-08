//! JSON DTO <-> FlatBuffers codec for the WebView IPC boundary.
//!
//! JSON DTOs carry int64 as decimal strings and ops as tagged unions:
//!   {"command_id":"…","op":{"AddTrackOp":{"track_id":"…","kind":"AUDIO","name":"…"}}}
//! Validation beyond JSON shape (finite floats, ranges, ID charset) happens in
//! void_protocol::validate; this layer only converts wire forms.

use flatbuffers::{FlatBufferBuilder, UnionWIPOffset, WIPOffset};
use serde_json::{json, Value};
use void_protocol::proto;

pub type Fbb<'a> = FlatBufferBuilder<'a>;

#[derive(Debug)]
pub struct CodecError(pub String);

impl std::fmt::Display for CodecError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for CodecError {}

fn err(msg: impl Into<String>) -> CodecError {
    CodecError(msg.into())
}

fn req<'v>(v: &'v Value, key: &str) -> Result<&'v Value, CodecError> {
    v.get(key)
        .ok_or_else(|| err(format!("missing field {key}")))
}

/// int64 on the wire; JSON form is a decimal string (or bare integer).
fn i64f(v: &Value, key: &str) -> Result<i64, CodecError> {
    let x = req(v, key)?;
    match x {
        Value::String(s) => s
            .parse::<i64>()
            .map_err(|_| err(format!("field {key}: invalid i64 \"{s}\""))),
        Value::Number(n) => n
            .as_i64()
            .ok_or_else(|| err(format!("field {key}: not an i64"))),
        _ => Err(err(format!("field {key}: expected string/number"))),
    }
}

fn i64o(v: &Value, key: &str, default: i64) -> Result<i64, CodecError> {
    match v.get(key) {
        Some(_) => i64f(v, key),
        None => Ok(default),
    }
}

fn u64f(v: &Value, key: &str) -> Result<u64, CodecError> {
    let x = req(v, key)?;
    match x {
        Value::String(s) => s
            .parse::<u64>()
            .map_err(|_| err(format!("field {key}: invalid u64 \"{s}\""))),
        Value::Number(n) => n
            .as_u64()
            .ok_or_else(|| err(format!("field {key}: not a u64"))),
        _ => Err(err(format!("field {key}: expected string/number"))),
    }
}

fn i32o(v: &Value, key: &str, default: i32) -> Result<i32, CodecError> {
    Ok(i64o(v, key, default as i64)? as i32)
}

fn u8f(v: &Value, key: &str) -> Result<u8, CodecError> {
    let n = i64f(v, key)?;
    u8::try_from(n).map_err(|_| err(format!("field {key}: {n} out of u8 range")))
}

fn f32f(v: &Value, key: &str) -> Result<f32, CodecError> {
    let x = req(v, key)?;
    let d = match x {
        Value::Number(n) => n
            .as_f64()
            .ok_or_else(|| err(format!("field {key}: not a number")))?,
        Value::String(s) => s
            .parse::<f64>()
            .map_err(|_| err(format!("field {key}: invalid float \"{s}\"")))?,
        _ => return Err(err(format!("field {key}: expected number"))),
    };
    if !d.is_finite() {
        return Err(err(format!("field {key}: non-finite")));
    }
    Ok(d as f32)
}

fn boolf(v: &Value, key: &str) -> Result<bool, CodecError> {
    req(v, key)?
        .as_bool()
        .ok_or_else(|| err(format!("field {key}: expected bool")))
}

fn strf<'v>(v: &'v Value, key: &str) -> Result<&'v str, CodecError> {
    req(v, key)?
        .as_str()
        .ok_or_else(|| err(format!("field {key}: expected string")))
}

fn stro<'v>(v: &'v Value, key: &str) -> &'v str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

fn enum_val<T: for<'de> flatbuffers::Follow<'de> + flatbuffers::Push>(
    v: &Value,
    key: &str,
    map: &[(T, &str)],
) -> Result<T, CodecError>
where
    T: Copy + Default,
{
    let name = stro(v, key);
    map.iter()
        .find(|(_, n)| *n == name)
        .map(|(t, _)| *t)
        .ok_or_else(|| err(format!("field {key}: unknown value \"{name}\"")))
}

fn track_kind(v: &Value) -> Result<proto::TrackKind, CodecError> {
    enum_val(
        v,
        "kind",
        &[
            (proto::TrackKind::AUDIO, "AUDIO"),
            (proto::TrackKind::MIDI, "MIDI"),
            (proto::TrackKind::INSTRUMENT, "INSTRUMENT"),
            (proto::TrackKind::BUS, "BUS"),
        ],
    )
}

fn transport_op(v: &Value) -> Result<proto::TransportOp, CodecError> {
    enum_val(
        v,
        "op",
        &[
            (proto::TransportOp::PLAY, "PLAY"),
            (proto::TransportOp::STOP, "STOP"),
            (proto::TransportOp::SEEK, "SEEK"),
            (proto::TransportOp::PANIC, "PANIC"),
            (proto::TransportOp::SET_CYCLE, "SET_CYCLE"),
            (proto::TransportOp::SET_TEMPO_LIVE, "SET_TEMPO_LIVE"),
        ],
    )
}

fn view_kind(v: &Value) -> Result<proto::ViewKind, CodecError> {
    enum_val(
        v,
        "view",
        &[
            (proto::ViewKind::PROJECT_SUMMARY, "PROJECT_SUMMARY"),
            (proto::ViewKind::TRACK_LIST, "TRACK_LIST"),
            (proto::ViewKind::CLIP_LIST, "CLIP_LIST"),
            (proto::ViewKind::NOTE_RANGE, "NOTE_RANGE"),
            (proto::ViewKind::ASSET_LIST, "ASSET_LIST"),
            (proto::ViewKind::PLUGIN_LIST, "PLUGIN_LIST"),
            (proto::ViewKind::RECEIPT_LIST, "RECEIPT_LIST"),
        ],
    )
}

// -- Persistent op builders -------------------------------------------------

macro_rules! empty_op {
    ($b:ident, $op:ty, $args:ty, $variant:expr) => {{
        let off = <$op>::create($b, &<$args>::default());
        ($variant, off.as_union_value())
    }};
}

fn build_op<'a>(
    b: &mut Fbb<'a>,
    tag: &str,
    o: &Value,
) -> Result<(proto::PersistentOp, WIPOffset<UnionWIPOffset>), CodecError> {
    use proto::PersistentOp as P;
    let pair = match tag {
        "CreateProjectOp" => {
            let name = b.create_string(strf(o, "name")?);
            let dir = b.create_string(strf(o, "container_dir")?);
            let off = proto::CreateProjectOp::create(
                b,
                &proto::CreateProjectOpArgs {
                    name: Some(name),
                    container_dir: Some(dir),
                    sample_rate: i32o(o, "sample_rate", 48000)? as u32,
                    initial_bpm: match o.get("initial_bpm") {
                        Some(_) => f32f(o, "initial_bpm")?,
                        None => 120.0,
                    },
                },
            );
            (P::CreateProjectOp, off.as_union_value())
        }
        "OpenProjectOp" => {
            let dir = b.create_string(strf(o, "container_dir")?);
            let off = proto::OpenProjectOp::create(
                b,
                &proto::OpenProjectOpArgs {
                    container_dir: Some(dir),
                },
            );
            (P::OpenProjectOp, off.as_union_value())
        }
        "CloseProjectOp" => {
            empty_op!(
                b,
                proto::CloseProjectOp,
                proto::CloseProjectOpArgs,
                P::CloseProjectOp
            )
        }
        "AddTrackOp" => {
            let tid = b.create_string(strf(o, "track_id")?);
            let name = b.create_string(stro(o, "name"));
            let off = proto::AddTrackOp::create(
                b,
                &proto::AddTrackOpArgs {
                    track_id: Some(tid),
                    kind: track_kind(o)?,
                    name: Some(name),
                    index: i32o(o, "index", -1)?,
                },
            );
            (P::AddTrackOp, off.as_union_value())
        }
        "RemoveTrackOp" => {
            let tid = b.create_string(strf(o, "track_id")?);
            let off = proto::RemoveTrackOp::create(
                b,
                &proto::RemoveTrackOpArgs {
                    track_id: Some(tid),
                },
            );
            (P::RemoveTrackOp, off.as_union_value())
        }
        "SetTrackNameOp" => {
            let tid = b.create_string(strf(o, "track_id")?);
            let name = b.create_string(strf(o, "name")?);
            let off = proto::SetTrackNameOp::create(
                b,
                &proto::SetTrackNameOpArgs {
                    track_id: Some(tid),
                    name: Some(name),
                },
            );
            (P::SetTrackNameOp, off.as_union_value())
        }
        "SetTrackGainOp" => {
            let tid = b.create_string(strf(o, "track_id")?);
            let off = proto::SetTrackGainOp::create(
                b,
                &proto::SetTrackGainOpArgs {
                    track_id: Some(tid),
                    gain_linear: f32f(o, "gain_linear")?,
                },
            );
            (P::SetTrackGainOp, off.as_union_value())
        }
        "SetTrackPanOp" => {
            let tid = b.create_string(strf(o, "track_id")?);
            let off = proto::SetTrackPanOp::create(
                b,
                &proto::SetTrackPanOpArgs {
                    track_id: Some(tid),
                    pan: f32f(o, "pan")?,
                },
            );
            (P::SetTrackPanOp, off.as_union_value())
        }
        "SetTrackMuteOp" => {
            let tid = b.create_string(strf(o, "track_id")?);
            let off = proto::SetTrackMuteOp::create(
                b,
                &proto::SetTrackMuteOpArgs {
                    track_id: Some(tid),
                    muted: boolf(o, "muted")?,
                },
            );
            (P::SetTrackMuteOp, off.as_union_value())
        }
        "SetTrackSoloOp" => {
            let tid = b.create_string(strf(o, "track_id")?);
            let off = proto::SetTrackSoloOp::create(
                b,
                &proto::SetTrackSoloOpArgs {
                    track_id: Some(tid),
                    soloed: boolf(o, "soloed")?,
                },
            );
            (P::SetTrackSoloOp, off.as_union_value())
        }
        "InsertAudioClipOp" => {
            let cid = b.create_string(strf(o, "clip_id")?);
            let tid = b.create_string(strf(o, "track_id")?);
            let aid = b.create_string(strf(o, "asset_id")?);
            let off = proto::InsertAudioClipOp::create(
                b,
                &proto::InsertAudioClipOpArgs {
                    clip_id: Some(cid),
                    track_id: Some(tid),
                    asset_id: Some(aid),
                    start_ticks: i64f(o, "start_ticks")?,
                    length_ticks: i64f(o, "length_ticks")?,
                    offset_ticks: i64o(o, "offset_ticks", 0)?,
                },
            );
            (P::InsertAudioClipOp, off.as_union_value())
        }
        "InsertMidiClipOp" => {
            let cid = b.create_string(strf(o, "clip_id")?);
            let tid = b.create_string(strf(o, "track_id")?);
            let off = proto::InsertMidiClipOp::create(
                b,
                &proto::InsertMidiClipOpArgs {
                    clip_id: Some(cid),
                    track_id: Some(tid),
                    start_ticks: i64f(o, "start_ticks")?,
                    length_ticks: i64f(o, "length_ticks")?,
                },
            );
            (P::InsertMidiClipOp, off.as_union_value())
        }
        "RemoveClipOp" => {
            let cid = b.create_string(strf(o, "clip_id")?);
            let off =
                proto::RemoveClipOp::create(b, &proto::RemoveClipOpArgs { clip_id: Some(cid) });
            (P::RemoveClipOp, off.as_union_value())
        }
        "MoveClipOp" => {
            let cid = b.create_string(strf(o, "clip_id")?);
            let tid = b.create_string(strf(o, "track_id")?);
            let off = proto::MoveClipOp::create(
                b,
                &proto::MoveClipOpArgs {
                    clip_id: Some(cid),
                    track_id: Some(tid),
                    start_ticks: i64f(o, "start_ticks")?,
                },
            );
            (P::MoveClipOp, off.as_union_value())
        }
        "TrimClipOp" => {
            let cid = b.create_string(strf(o, "clip_id")?);
            let off = proto::TrimClipOp::create(
                b,
                &proto::TrimClipOpArgs {
                    clip_id: Some(cid),
                    start_ticks: i64f(o, "start_ticks")?,
                    length_ticks: i64f(o, "length_ticks")?,
                    offset_ticks: i64o(o, "offset_ticks", 0)?,
                },
            );
            (P::TrimClipOp, off.as_union_value())
        }
        "SplitClipOp" => {
            let cid = b.create_string(strf(o, "clip_id")?);
            let nid = b.create_string(strf(o, "new_clip_id")?);
            let off = proto::SplitClipOp::create(
                b,
                &proto::SplitClipOpArgs {
                    clip_id: Some(cid),
                    at_ticks: i64f(o, "at_ticks")?,
                    new_clip_id: Some(nid),
                },
            );
            (P::SplitClipOp, off.as_union_value())
        }
        "InsertNoteOp" => {
            let cid = b.create_string(strf(o, "clip_id")?);
            let nid = b.create_string(strf(o, "note_id")?);
            let off = proto::InsertNoteOp::create(
                b,
                &proto::InsertNoteOpArgs {
                    clip_id: Some(cid),
                    note_id: Some(nid),
                    pitch: u8f(o, "pitch")?,
                    velocity: u8f(o, "velocity")?,
                    start_ticks: i64f(o, "start_ticks")?,
                    length_ticks: i64f(o, "length_ticks")?,
                },
            );
            (P::InsertNoteOp, off.as_union_value())
        }
        "RemoveNoteOp" => {
            let cid = b.create_string(strf(o, "clip_id")?);
            let nid = b.create_string(strf(o, "note_id")?);
            let off = proto::RemoveNoteOp::create(
                b,
                &proto::RemoveNoteOpArgs {
                    clip_id: Some(cid),
                    note_id: Some(nid),
                },
            );
            (P::RemoveNoteOp, off.as_union_value())
        }
        "SetNoteOp" => {
            let cid = b.create_string(strf(o, "clip_id")?);
            let nid = b.create_string(strf(o, "note_id")?);
            let off = proto::SetNoteOp::create(
                b,
                &proto::SetNoteOpArgs {
                    clip_id: Some(cid),
                    note_id: Some(nid),
                    pitch: i32o(o, "pitch", -1)?,
                    velocity: i32o(o, "velocity", -1)?,
                    start_ticks: i64o(o, "start_ticks", -1)?,
                    length_ticks: i64o(o, "length_ticks", -1)?,
                },
            );
            (P::SetNoteOp, off.as_union_value())
        }
        "SetTempoOp" => {
            let off = proto::SetTempoOp::create(
                b,
                &proto::SetTempoOpArgs {
                    at_ticks: i64f(o, "at_ticks")?,
                    bpm: f32f(o, "bpm")?,
                },
            );
            (P::SetTempoOp, off.as_union_value())
        }
        "SetTimeSignatureOp" => {
            let off = proto::SetTimeSignatureOp::create(
                b,
                &proto::SetTimeSignatureOpArgs {
                    at_ticks: i64f(o, "at_ticks")?,
                    numerator: u8f(o, "numerator")?,
                    denominator: u8f(o, "denominator")?,
                },
            );
            (P::SetTimeSignatureOp, off.as_union_value())
        }
        "SetLoopRangeOp" => {
            let off = proto::SetLoopRangeOp::create(
                b,
                &proto::SetLoopRangeOpArgs {
                    start_ticks: i64f(o, "start_ticks")?,
                    end_ticks: i64f(o, "end_ticks")?,
                    enabled: boolf(o, "enabled")?,
                },
            );
            (P::SetLoopRangeOp, off.as_union_value())
        }
        "SaveProjectOp" => {
            let r = b.create_string(stro(o, "reason"));
            let off =
                proto::SaveProjectOp::create(b, &proto::SaveProjectOpArgs { reason: Some(r) });
            (P::SaveProjectOp, off.as_union_value())
        }
        "CreateCheckpointOp" => {
            let r = b.create_string(stro(o, "reason"));
            let off = proto::CreateCheckpointOp::create(
                b,
                &proto::CreateCheckpointOpArgs { reason: Some(r) },
            );
            (P::CreateCheckpointOp, off.as_union_value())
        }
        "UndoOp" => {
            let t = b.create_string(stro(o, "transaction_id"));
            let off = proto::UndoOp::create(
                b,
                &proto::UndoOpArgs {
                    transaction_id: Some(t),
                },
            );
            (P::UndoOp, off.as_union_value())
        }
        "RedoOp" => {
            let t = b.create_string(stro(o, "transaction_id"));
            let off = proto::RedoOp::create(
                b,
                &proto::RedoOpArgs {
                    transaction_id: Some(t),
                },
            );
            (P::RedoOp, off.as_union_value())
        }
        "AttachAssetOp" => {
            let aid = b.create_string(strf(o, "asset_id")?);
            let sha = b.create_string(strf(o, "sha256")?);
            let mt = b.create_string(stro(o, "media_type"));
            let rp = b.create_string(strf(o, "rel_path")?);
            let off = proto::AttachAssetOp::create(
                b,
                &proto::AttachAssetOpArgs {
                    asset_id: Some(aid),
                    sha256: Some(sha),
                    media_type: Some(mt),
                    rel_path: Some(rp),
                    channels: i32o(o, "channels", 0)?,
                    duration_ticks: i64o(o, "duration_ticks", 0)?,
                },
            );
            (P::AttachAssetOp, off.as_union_value())
        }
        "InsertPluginOp" => {
            let tid = b.create_string(strf(o, "track_id")?);
            let pid = b.create_string(strf(o, "plugin_instance_id")?);
            let fmt = b.create_string(strf(o, "format")?);
            let uid = b.create_string(strf(o, "plugin_uid")?);
            let off = proto::InsertPluginOp::create(
                b,
                &proto::InsertPluginOpArgs {
                    track_id: Some(tid),
                    slot: i32o(o, "slot", -1)?,
                    plugin_instance_id: Some(pid),
                    format: Some(fmt),
                    plugin_uid: Some(uid),
                },
            );
            (P::InsertPluginOp, off.as_union_value())
        }
        "RemovePluginOp" => {
            let pid = b.create_string(strf(o, "plugin_instance_id")?);
            let off = proto::RemovePluginOp::create(
                b,
                &proto::RemovePluginOpArgs {
                    plugin_instance_id: Some(pid),
                },
            );
            (P::RemovePluginOp, off.as_union_value())
        }
        "SetPluginParamOp" => {
            let pid = b.create_string(strf(o, "plugin_instance_id")?);
            let par = b.create_string(strf(o, "param_id")?);
            let off = proto::SetPluginParamOp::create(
                b,
                &proto::SetPluginParamOpArgs {
                    plugin_instance_id: Some(pid),
                    param_id: Some(par),
                    value: f32f(o, "value")?,
                },
            );
            (P::SetPluginParamOp, off.as_union_value())
        }
        "OpenPluginEditorOp" => {
            let pid = b.create_string(strf(o, "plugin_instance_id")?);
            let off = proto::OpenPluginEditorOp::create(
                b,
                &proto::OpenPluginEditorOpArgs {
                    plugin_instance_id: Some(pid),
                },
            );
            (P::OpenPluginEditorOp, off.as_union_value())
        }
        "ClosePluginEditorOp" => {
            let pid = b.create_string(strf(o, "plugin_instance_id")?);
            let off = proto::ClosePluginEditorOp::create(
                b,
                &proto::ClosePluginEditorOpArgs {
                    plugin_instance_id: Some(pid),
                },
            );
            (P::ClosePluginEditorOp, off.as_union_value())
        }
        other => return Err(err(format!("unknown op \"{other}\""))),
    };
    Ok(pair)
}

/// Build a full `ControlEnvelope` (RequestFrame{PersistentCommand}) from JSON.
pub fn persistent_command_json(v: &Value) -> Result<Vec<u8>, CodecError> {
    let mut b = FlatBufferBuilder::new();
    let op_field = req(v, "op")?;
    let obj = op_field
        .as_object()
        .ok_or_else(|| err("op: expected { \"<OpName>\": {…} }"))?;
    if obj.len() != 1 {
        return Err(err("op: exactly one tagged variant required"));
    }
    let (tag, opv) = obj.iter().next().unwrap();
    let (op_type, op) = build_op(&mut b, tag, opv)?;

    let cid = b.create_string(strf(v, "command_id")?);
    let tid = b.create_string(strf(v, "transaction_id")?);
    let pid = b.create_string(strf(v, "project_id")?);
    let cmd = proto::PersistentCommand::create(
        &mut b,
        &proto::PersistentCommandArgs {
            command_id: Some(cid),
            transaction_id: Some(tid),
            project_id: Some(pid),
            engine_epoch: u64f(v, "engine_epoch")?,
            expected_revision: u64f(v, "expected_revision")?,
            op_type: op_type,
            op: Some(op),
        },
    );
    wrap_request(
        &mut b,
        proto::ControlRequest::PersistentCommand,
        cmd.as_union_value(),
    )
}

pub fn transport_request_json(v: &Value) -> Result<Vec<u8>, CodecError> {
    let mut b = FlatBufferBuilder::new();
    let rid = b.create_string(strf(v, "request_id")?);
    let pid = b.create_string(strf(v, "project_id")?);
    let req = proto::TransportRequest::create(
        &mut b,
        &proto::TransportRequestArgs {
            request_id: Some(rid),
            project_id: Some(pid),
            engine_epoch: u64f(v, "engine_epoch")?,
            op: transport_op(v)?,
            position_ticks: i64o(v, "position_ticks", -1)?,
            cycle_start_ticks: i64o(v, "cycle_start_ticks", 0)?,
            cycle_end_ticks: i64o(v, "cycle_end_ticks", 0)?,
            value: match v.get("value") {
                Some(_) => f32f(v, "value")?,
                None => 0.0,
            },
        },
    );
    wrap_request(
        &mut b,
        proto::ControlRequest::TransportRequest,
        req.as_union_value(),
    )
}

pub fn read_request_json(v: &Value) -> Result<Vec<u8>, CodecError> {
    let mut b = FlatBufferBuilder::new();
    let rid = b.create_string(strf(v, "request_id")?);
    let pid = b.create_string(strf(v, "project_id")?);
    let cursor = b.create_string(stro(v, "cursor"));
    let track = b.create_string(stro(v, "track_id"));
    let req = proto::ReadRequest::create(
        &mut b,
        &proto::ReadRequestArgs {
            request_id: Some(rid),
            project_id: Some(pid),
            view: view_kind(v)?,
            cursor: Some(cursor),
            limit: i32o(v, "limit", 0)? as u32,
            track_id: Some(track),
            start_ticks: i64o(v, "start_ticks", -1)?,
            end_ticks: i64o(v, "end_ticks", -1)?,
        },
    );
    wrap_request(
        &mut b,
        proto::ControlRequest::ReadRequest,
        req.as_union_value(),
    )
}

fn wrap_request<'a>(
    b: &mut Fbb<'a>,
    ty: proto::ControlRequest,
    off: WIPOffset<UnionWIPOffset>,
) -> Result<Vec<u8>, CodecError> {
    let inner = proto::RequestFrame::create(
        b,
        &proto::RequestFrameArgs {
            request_type: ty,
            request: Some(off),
        },
    );
    let env = proto::ControlEnvelope::create(
        b,
        &proto::ControlEnvelopeArgs {
            frame_type: proto::ControlFrame::RequestFrame,
            frame: Some(inner.as_union_value()),
        },
    );
    b.finish_minimal(env);
    Ok(b.finished_data().to_vec())
}

// -- worker -> app decoding --------------------------------------------------

fn ack_status(s: proto::AckStatus) -> &'static str {
    match s {
        proto::AckStatus::APPLIED => "APPLIED",
        proto::AckStatus::DUPLICATE => "DUPLICATE",
        proto::AckStatus::REJECTED => "REJECTED",
        _ => "OUTCOME_UNKNOWN",
    }
}

fn error_code(e: proto::ErrorCode) -> &'static str {
    e.variant_name().unwrap_or("NONE")
}

/// Decode a control `EventFrame` envelope into JSON for the WebView.
/// Returns `Ok(None)` for frames the UI should not see (e.g. stray requests).
pub fn event_frame_json(frame: &[u8]) -> Result<Option<Value>, CodecError> {
    let env = flatbuffers::root::<proto::ControlEnvelope>(frame)
        .map_err(|e| err(format!("bad envelope: {e}")))?;
    let Some(ev) = env.frame_as_event_frame() else {
        return Ok(None);
    };
    Ok(match ev.event_type() {
        proto::ControlEvent::CommandReceipt => ev.event_as_command_receipt().map(|r| {
            json!({
                "kind": "CommandReceipt",
                "command_id": r.command_id().unwrap_or(""),
                "transaction_id": r.transaction_id().unwrap_or(""),
                "status": ack_status(r.status()),
                "error": error_code(r.error()),
                "revision": r.revision().to_string(),
                "engine_epoch": r.engine_epoch().to_string(),
                "message": r.message().unwrap_or(""),
                "payload_hash": r.payload_hash().unwrap_or(""),
            })
        }),
        proto::ControlEvent::ReadResponse => ev.event_as_read_response().map(|r| {
            let items: Vec<Value> = r
                .items()
                .map(|v| {
                    v.iter()
                        .map(|it| {
                            json!({
                                "object_id": it.object_id().unwrap_or(""),
                                "summary_json": it.summary_json().unwrap_or("{}"),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            json!({
                "kind": "ReadResponse",
                "request_id": r.request_id().unwrap_or(""),
                "revision": r.revision().to_string(),
                "items": items,
                "next_cursor": r.next_cursor().unwrap_or(""),
                "done": r.done(),
                "error": error_code(r.error()),
            })
        }),
        proto::ControlEvent::PluginScanResult => ev.event_as_plugin_scan_result().map(|r| {
            json!({
                "kind": "PluginScanResult",
                "request_id": r.request_id().unwrap_or(""),
                "ok": r.ok(),
                "quarantined": r.quarantined(),
                "plugin_uid": r.plugin_uid().unwrap_or(""),
                "binary_sha256": r.binary_sha256().unwrap_or(""),
                "name": r.name().unwrap_or(""),
                "version": r.version().unwrap_or(""),
                "arch": r.arch().unwrap_or(""),
                "error": error_code(r.error()),
                "message": r.message().unwrap_or(""),
            })
        }),
        _ => None,
    })
}

/// Decode a telemetry frame into JSON (lossy channel; UI-side).
pub fn telemetry_json(frame: &[u8]) -> Result<Option<Value>, CodecError> {
    let t = flatbuffers::root::<proto::TelemetryFrame>(frame)
        .map_err(|e| err(format!("bad telemetry: {e}")))?;
    Ok(match t.event_type() {
        proto::TelemetryEvent::ClockSnapshot => t.event_as_clock_snapshot().map(|r| {
            json!({
                "kind": "ClockSnapshot",
                "project_id": r.project_id().unwrap_or(""),
                "engine_epoch": r.engine_epoch().to_string(),
                "timeline_sample": r.timeline_sample().to_string(),
                "device_sample_counter": r.device_sample_counter().to_string(),
                "sample_rate": r.sample_rate(),
                "transport": r.transport_state().variant_name().unwrap_or("STOPPED"),
                "loop_start_ticks": r.loop_start_ticks().to_string(),
                "loop_end_ticks": r.loop_end_ticks().to_string(),
                "tempo_map_revision": r.tempo_map_revision().to_string(),
                "sequence": r.sequence().to_string(),
                "host_clock_ns": r.host_clock_ns().to_string(),
            })
        }),
        proto::TelemetryEvent::MeterFrame => t.event_as_meter_frame().map(|r| {
            json!({
                "kind": "MeterFrame",
                "project_id": r.project_id().unwrap_or(""),
                "engine_epoch": r.engine_epoch().to_string(),
                "track_id": r.track_id().unwrap_or(""),
                "peak_l": r.peak_l(),
                "peak_r": r.peak_r(),
                "rms_l": r.rms_l(),
                "rms_r": r.rms_r(),
                "clipped": r.clipped(),
                "sequence": r.sequence().to_string(),
            })
        }),
        proto::TelemetryEvent::SaveResultEvent => t.event_as_save_result_event().map(|r| {
            json!({
                "kind": "SaveResultEvent",
                "project_id": r.project_id().unwrap_or(""),
                "command_id": r.command_id().unwrap_or(""),
                "status": r.status().variant_name().unwrap_or("SAVE_FAILED"),
                "revision": r.revision().to_string(),
                "checkpoint_id": r.checkpoint_id().unwrap_or(""),
                "manifest_sha256": r.manifest_sha256().unwrap_or(""),
                "error": error_code(r.error()),
                "message": r.message().unwrap_or(""),
            })
        }),
        proto::TelemetryEvent::TransportAck => t.event_as_transport_ack().map(|r| {
            json!({
                "kind": "TransportAck",
                "request_id": r.request_id().unwrap_or(""),
                "ok": r.ok(),
                "error": error_code(r.error()),
            })
        }),
        _ => None,
    })
}
