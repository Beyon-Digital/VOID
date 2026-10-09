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

fn enum_val<T>(v: &Value, key: &str, map: &[(T, &str)]) -> Result<T, CodecError>
where
    T: for<'de> flatbuffers::Follow<'de> + flatbuffers::Push + Copy + Default,
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
            // rev-2 views: screaming enum name and the pinned snake_case
            // DTO spelling (packages/void-studio jobListRequest/…) both decode.
            (proto::ViewKind::TAKE_LIST, "TAKE_LIST"),
            (proto::ViewKind::TAKE_LIST, "take_list"),
            (proto::ViewKind::INPUT_DEVICE_LIST, "INPUT_DEVICE_LIST"),
            (proto::ViewKind::INPUT_DEVICE_LIST, "input_device_list"),
            (proto::ViewKind::JOB_LIST, "JOB_LIST"),
            (proto::ViewKind::JOB_LIST, "job_list"),
            (proto::ViewKind::MODEL_LIST, "MODEL_LIST"),
            (proto::ViewKind::MODEL_LIST, "model_list"),
            (proto::ViewKind::PROPOSAL_LIST, "PROPOSAL_LIST"),
            (proto::ViewKind::PROPOSAL_LIST, "proposal_list"),
            (proto::ViewKind::SCENE_LIST, "SCENE_LIST"),
            (proto::ViewKind::SCENE_LIST, "scene_list"),
        ],
    )
}

fn monitor_mode(v: &Value, key: &str) -> Result<proto::MonitorMode, CodecError> {
    enum_val(
        v,
        key,
        &[
            (proto::MonitorMode::OFF, "OFF"),
            (proto::MonitorMode::AUTOMATIC, "AUTOMATIC"),
            (proto::MonitorMode::ON, "ON"),
        ],
    )
}

fn launch_quantize(v: &Value) -> Result<proto::LaunchQuantize, CodecError> {
    match v.get("quantize") {
        Some(_) => enum_val(
            v,
            "quantize",
            &[
                (proto::LaunchQuantize::IMMEDIATE, "IMMEDIATE"),
                (proto::LaunchQuantize::BAR, "BAR"),
                (proto::LaunchQuantize::BEAT, "BEAT"),
                (proto::LaunchQuantize::CUSTOM, "CUSTOM"),
            ],
        ),
        None => Ok(proto::LaunchQuantize::BAR),
    }
}

/// Two spellings for rev-2 fields: the tagged-op snake_case convention
/// ("job_id") and the pinned flat-DTO camelCase ("jobId") from
/// packages/void-studio. Returns the first present.
fn str2<'v>(o: &'v Value, snake: &str, camel: &str) -> Result<&'v str, CodecError> {
    if let Some(x) = o.get(snake) {
        return x
            .as_str()
            .ok_or_else(|| err(format!("field {snake}: expected string")));
    }
    strf(o, camel)
}

/// Optional-string variant of `str2`: empty string when neither spelling
/// is present.
fn stro2<'v>(o: &'v Value, snake: &str, camel: &str) -> &'v str {
    o.get(snake)
        .or_else(|| o.get(camel))
        .and_then(Value::as_str)
        .unwrap_or("")
}

fn u64opt(v: &Value, key: &str) -> Result<Option<u64>, CodecError> {
    match v.get(key) {
        Some(Value::Null) | None => Ok(None),
        Some(_) => Ok(Some(u64f(v, key)?)),
    }
}

/// Build the JobSpec nested table from the pinned JobSpecEnvelope serde
/// shape (camelCase, decimal-string numerics — packages/void-studio
/// jobs/job.ts + crates/void-jobs::JobSpec).
fn build_job_spec<'a>(
    b: &mut Fbb<'a>,
    s: &Value,
) -> Result<WIPOffset<proto::JobSpec<'a>>, CodecError> {
    let job_id = b.create_string(strf(s, "jobId")?);
    let project_id = b.create_string(strf(s, "projectId")?);
    let context_sha256 = b.create_string(strf(s, "contextSha256")?);
    let kind = b.create_string(strf(s, "kind")?);
    let runtime_id = b.create_string(strf(s, "runtimeId")?);
    let runtime_sha256 = b.create_string(stro(s, "runtimeSha256"));
    let model_id = b.create_string(stro(s, "modelId"));
    let model_sha256 = b.create_string(stro(s, "modelSha256"));
    let inputs: Vec<WIPOffset<_>> = match s.get("inputs") {
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| b.create_string(x.as_str().unwrap_or("")))
            .collect(),
        _ => Vec::new(),
    };
    let inputs_v = b.create_vector(&inputs);
    // `parameters` is an opaque spec echo — re-serialized verbatim.
    let params = match s.get("parameters") {
        Some(p) if !p.is_null() => {
            serde_json::to_string(p).map_err(|e| err(format!("parameters: {e}")))?
        }
        _ => "{}".to_string(),
    };
    let parameters_json = b.create_string(&params);
    let res = s.get("reservations").cloned().unwrap_or(json!({}));
    let output_scope_token = b.create_string(strf(s, "outputScopeToken")?);
    let cloud_consent_id = b.create_string(stro(s, "cloudConsentId"));
    Ok(proto::JobSpec::create(
        b,
        &proto::JobSpecArgs {
            job_id: Some(job_id),
            project_id: Some(project_id),
            source_revision: u64opt(s, "sourceRevision")?.unwrap_or(0),
            context_sha256: Some(context_sha256),
            kind: Some(kind),
            runtime_id: Some(runtime_id),
            runtime_sha256: Some(runtime_sha256),
            model_id: Some(model_id),
            model_sha256: Some(model_sha256),
            inputs: Some(inputs_v),
            parameters_json: Some(parameters_json),
            ram_bytes: u64opt(&res, "ramBytes")?.unwrap_or(0),
            vram_bytes: u64opt(&res, "vramBytes")?.unwrap_or(0),
            cpu_threads: i32o(&res, "cpuThreads", 0)?,
            deadline_monotonic_ns: u64opt(s, "deadlineMonotonicNs")?.unwrap_or(0),
            output_scope_token: Some(output_scope_token),
            cloud_consent_id: Some(cloud_consent_id),
        },
    ))
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
        // -- rev-2 (protocol minor 1) --------------------------------------
        "ArmTrackOp" => {
            let track = b.create_string(str2(o, "track_id", "trackId")?);
            let dev = b.create_string(stro2(o, "input_device", "inputDevice"));
            let off = proto::ArmTrackOp::create(
                b,
                &proto::ArmTrackOpArgs {
                    track_id: Some(track),
                    record_enabled: match o.get("record_enabled").or(o.get("recordEnabled")) {
                        Some(_) => boolf(
                            o,
                            if o.get("record_enabled").is_some() {
                                "record_enabled"
                            } else {
                                "recordEnabled"
                            },
                        )?,
                        None => true,
                    },
                    input_device: Some(dev),
                    monitor_mode: match o.get("monitor_mode").or(o.get("monitorMode")) {
                        Some(_) => monitor_mode(
                            o,
                            if o.get("monitor_mode").is_some() {
                                "monitor_mode"
                            } else {
                                "monitorMode"
                            },
                        )?,
                        None => proto::MonitorMode::AUTOMATIC,
                    },
                    is_midi: match o.get("is_midi").or(o.get("isMidi")) {
                        Some(_) => boolf(
                            o,
                            if o.get("is_midi").is_some() {
                                "is_midi"
                            } else {
                                "isMidi"
                            },
                        )?,
                        None => false,
                    },
                },
            );
            (P::ArmTrackOp, off.as_union_value())
        }
        "StartRecordingOp" => {
            let take = b.create_string(stro2(o, "take_id", "takeId"));
            let off = proto::StartRecordingOp::create(
                b,
                &proto::StartRecordingOpArgs {
                    take_id: Some(take),
                },
            );
            (P::StartRecordingOp, off.as_union_value())
        }
        "StopRecordingOp" => {
            let off = proto::StopRecordingOp::create(
                b,
                &proto::StopRecordingOpArgs {
                    discard: match o.get("discard") {
                        Some(_) => boolf(o, "discard")?,
                        None => false,
                    },
                },
            );
            (P::StopRecordingOp, off.as_union_value())
        }
        "SetCountInOp" => {
            let mode = b.create_string(strf(o, "mode")?);
            let off = proto::SetCountInOp::create(
                b,
                &proto::SetCountInOpArgs {
                    mode: Some(mode),
                    bars: i32o(o, "bars", 1)?,
                },
            );
            (P::SetCountInOp, off.as_union_value())
        }
        "SetMetronomeOp" => {
            let off = proto::SetMetronomeOp::create(
                b,
                &proto::SetMetronomeOpArgs {
                    enabled: boolf(o, "enabled")?,
                    gain: match o.get("gain") {
                        Some(_) => f32f(o, "gain")?,
                        None => 0.5,
                    },
                    recording_only: match o.get("recording_only").or(o.get("recordingOnly")) {
                        Some(_) => boolf(
                            o,
                            if o.get("recording_only").is_some() {
                                "recording_only"
                            } else {
                                "recordingOnly"
                            },
                        )?,
                        None => false,
                    },
                },
            );
            (P::SetMetronomeOp, off.as_union_value())
        }
        "SetPunchInOutOp" => {
            let off = proto::SetPunchInOutOp::create(
                b,
                &proto::SetPunchInOutOpArgs {
                    enabled: boolf(o, "enabled")?,
                    in_ticks: i64o(o, "in_ticks", -1)?,
                    out_ticks: i64o(o, "out_ticks", -1)?,
                },
            );
            (P::SetPunchInOutOp, off.as_union_value())
        }
        "SubmitJobOp" => {
            let spec = build_job_spec(b, req(o, "spec")?)?;
            let off = proto::SubmitJobOp::create(b, &proto::SubmitJobOpArgs { spec: Some(spec) });
            (P::SubmitJobOp, off.as_union_value())
        }
        "CancelJobOp" => {
            let jid = b.create_string(str2(o, "job_id", "jobId")?);
            let off = proto::CancelJobOp::create(b, &proto::CancelJobOpArgs { job_id: Some(jid) });
            (P::CancelJobOp, off.as_union_value())
        }
        "PauseJobOp" => {
            let jid = b.create_string(str2(o, "job_id", "jobId")?);
            let off = proto::PauseJobOp::create(b, &proto::PauseJobOpArgs { job_id: Some(jid) });
            (P::PauseJobOp, off.as_union_value())
        }
        "InstallModelOp" => {
            let mid = b.create_string(str2(o, "model_id", "modelId")?);
            let ver = b.create_string(str2(o, "model_version", "modelVersion")?);
            let uri = b.create_string(str2(o, "source_uri", "sourceUri")?);
            let off = proto::InstallModelOp::create(
                b,
                &proto::InstallModelOpArgs {
                    model_id: Some(mid),
                    model_version: Some(ver),
                    source_uri: Some(uri),
                },
            );
            (P::InstallModelOp, off.as_union_value())
        }
        "RequestProposalOp" => {
            let digest = b.create_string(str2(o, "context_digest", "contextDigest")?);
            let off = proto::RequestProposalOp::create(
                b,
                &proto::RequestProposalOpArgs {
                    context_digest: Some(digest),
                    seed: u64opt(o, "seed")?.unwrap_or(0),
                    max_proposals: match o.get("max_proposals").or(o.get("maxProposals")) {
                        Some(_) => i32o(
                            o,
                            if o.get("max_proposals").is_some() {
                                "max_proposals"
                            } else {
                                "maxProposals"
                            },
                            1,
                        )?,
                        None => 1,
                    },
                },
            );
            (P::RequestProposalOp, off.as_union_value())
        }
        "ResolveProposalOp" => {
            let pid = b.create_string(str2(o, "proposal_id", "proposalId")?);
            let off = proto::ResolveProposalOp::create(
                b,
                &proto::ResolveProposalOpArgs {
                    proposal_id: Some(pid),
                    accept: boolf(o, "accept")?,
                    candidate_rank: match o.get("candidate_rank").or(o.get("candidateRank")) {
                        Some(_) => i32o(
                            o,
                            if o.get("candidate_rank").is_some() {
                                "candidate_rank"
                            } else {
                                "candidateRank"
                            },
                            -1,
                        )?,
                        None => -1,
                    },
                },
            );
            (P::ResolveProposalOp, off.as_union_value())
        }
        "PreviewLayerOp" => {
            let pid = b.create_string(str2(o, "proposal_id", "proposalId")?);
            let cid = b.create_string(str2(o, "clip_id", "clipId")?);
            let notes: Vec<WIPOffset<proto::PreviewNote>> = match req(o, "notes")? {
                Value::Array(a) => {
                    let mut v = Vec::with_capacity(a.len());
                    for n in a {
                        let nid = b.create_string(stro2(n, "note_id", "noteId"));
                        v.push(proto::PreviewNote::create(
                            b,
                            &proto::PreviewNoteArgs {
                                note_id: Some(nid),
                                pitch: u8f(n, "pitch")?,
                                velocity: u8f(n, "velocity")?,
                                start_ticks: i64f(n, "start_ticks")?,
                                length_ticks: i64f(n, "length_ticks")?,
                            },
                        ));
                    }
                    v
                }
                _ => return Err(err("notes: expected array")),
            };
            let notes_v = b.create_vector(&notes);
            let off = proto::PreviewLayerOp::create(
                b,
                &proto::PreviewLayerOpArgs {
                    proposal_id: Some(pid),
                    clip_id: Some(cid),
                    notes: Some(notes_v),
                    enable: match o.get("enable") {
                        Some(_) => boolf(o, "enable")?,
                        None => true,
                    },
                },
            );
            (P::PreviewLayerOp, off.as_union_value())
        }
        "IngestAssetOp" => {
            let rel = b.create_string(str2(o, "rel_path", "relPath")?);
            let mt = b.create_string(str2(o, "media_type", "mediaType")?);
            let off = proto::IngestAssetOp::create(
                b,
                &proto::IngestAssetOpArgs {
                    rel_path: Some(rel),
                    media_type: Some(mt),
                },
            );
            (P::IngestAssetOp, off.as_union_value())
        }
        "RelinkAssetOp" => {
            let aid = b.create_string(str2(o, "asset_id", "assetId")?);
            let sha = b.create_string(strf(o, "sha256")?);
            let off = proto::RelinkAssetOp::create(
                b,
                &proto::RelinkAssetOpArgs {
                    asset_id: Some(aid),
                    sha256: Some(sha),
                },
            );
            (P::RelinkAssetOp, off.as_union_value())
        }
        "SetPluginBypassOp" => {
            let pid = b.create_string(str2(o, "plugin_instance_id", "pluginInstanceId")?);
            let off = proto::SetPluginBypassOp::create(
                b,
                &proto::SetPluginBypassOpArgs {
                    plugin_instance_id: Some(pid),
                    bypassed: boolf(o, "bypassed")?,
                },
            );
            (P::SetPluginBypassOp, off.as_union_value())
        }
        "RescanPluginsOp" => {
            let uid = b.create_string(stro2(o, "plugin_uid", "pluginUid"));
            let off = proto::RescanPluginsOp::create(
                b,
                &proto::RescanPluginsOpArgs {
                    plugin_uid: Some(uid),
                },
            );
            (P::RescanPluginsOp, off.as_union_value())
        }
        "RestorePluginStateOp" => {
            let pid = b.create_string(str2(o, "plugin_instance_id", "pluginInstanceId")?);
            let sid = b.create_string(str2(o, "state_asset_id", "stateAssetId")?);
            let off = proto::RestorePluginStateOp::create(
                b,
                &proto::RestorePluginStateOpArgs {
                    plugin_instance_id: Some(pid),
                    state_asset_id: Some(sid),
                },
            );
            (P::RestorePluginStateOp, off.as_union_value())
        }
        "SaveProjectAsOp" => {
            let dir = b.create_string(str2(o, "container_dir", "containerDir")?);
            let name = b.create_string(stro(o, "name"));
            let reason = b.create_string(stro(o, "reason"));
            let off = proto::SaveProjectAsOp::create(
                b,
                &proto::SaveProjectAsOpArgs {
                    container_dir: Some(dir),
                    name: Some(name),
                    reason: Some(reason),
                },
            );
            (P::SaveProjectAsOp, off.as_union_value())
        }
        "LaunchSceneOp" => {
            let sid = b.create_string(str2(o, "scene_id", "sceneId")?);
            let off = proto::LaunchSceneOp::create(
                b,
                &proto::LaunchSceneOpArgs {
                    scene_id: Some(sid),
                    quantize: launch_quantize(o)?,
                    quantize_ticks: i64o(o, "quantize_ticks", 0)?,
                },
            );
            (P::LaunchSceneOp, off.as_union_value())
        }
        "StopSceneOp" => {
            let sid = b.create_string(stro2(o, "scene_id", "sceneId"));
            let off = proto::StopSceneOp::create(
                b,
                &proto::StopSceneOpArgs {
                    scene_id: Some(sid),
                    quantize: launch_quantize(o)?,
                    quantize_ticks: i64o(o, "quantize_ticks", 0)?,
                },
            );
            (P::StopSceneOp, off.as_union_value())
        }
        "LaunchClipOp" => {
            let sid = b.create_string(str2(o, "slot_id", "slotId")?);
            let off = proto::LaunchClipOp::create(
                b,
                &proto::LaunchClipOpArgs {
                    slot_id: Some(sid),
                    quantize: launch_quantize(o)?,
                    quantize_ticks: i64o(o, "quantize_ticks", 0)?,
                },
            );
            (P::LaunchClipOp, off.as_union_value())
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
    // Two accepted op shapes:
    //   tagged:   {"SubmitJobOp": {…}}            — the one-key union form
    //   flat rev-2 pinned: {"op":"submit_job","spec":{…}} — the studio's
    //     snake_case discriminator DTOs (jobs/job.ts submitJobOp/cancelJobOp).
    let (tag, opv): (&str, &Value) = if obj.len() == 1 && !obj.contains_key("op") {
        let (t, ov) = obj.iter().next().unwrap();
        (t.as_str(), ov)
    } else if let Some(Value::String(name)) = obj.get("op") {
        let t = match name.as_str() {
            "submit_job" => "SubmitJobOp",
            "cancel_job" => "CancelJobOp",
            "pause_job" => "PauseJobOp",
            "install_model" => "InstallModelOp",
            other => return Err(err(format!("unknown flat op \"{other}\""))),
        };
        (t, op_field)
    } else {
        return Err(err("op: exactly one tagged variant required"));
    };
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
            op_type,
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
            include_terminal: match v.get("include_terminal").or(v.get("includeTerminal")) {
                Some(x) => x.as_bool().unwrap_or(false),
                None => false,
            },
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
        proto::TelemetryEvent::JobEvent => t.event_as_job_event().map(|r| {
            json!({
                "kind": "JobEvent",
                "project_id": r.project_id().unwrap_or(""),
                "job_id": r.job_id().unwrap_or(""),
                "status": r.status().unwrap_or(""),
                "percent": r.percent(),
                "message": r.message().unwrap_or(""),
                "quarantined": r.quarantined(),
            })
        }),
        proto::TelemetryEvent::ProposalStaleEvent => t.event_as_proposal_stale_event().map(|r| {
            json!({
                "kind": "ProposalStaleEvent",
                "project_id": r.project_id().unwrap_or(""),
                "proposal_id": r.proposal_id().unwrap_or(""),
                "cause": r.cause().unwrap_or(""),
            })
        }),
        proto::TelemetryEvent::InputDeviceLostEvent => {
            t.event_as_input_device_lost_event().map(|r| {
                json!({
                    "kind": "InputDeviceLostEvent",
                    "project_id": r.project_id().unwrap_or(""),
                    "device_id": r.device_id().unwrap_or(""),
                    "device_name": r.device_name().unwrap_or(""),
                })
            })
        }
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PID: &str = "3f786950-e387-4fbf-9a9c-4cc3b5a2e1f0";
    const T1: &str = "b0a9b0f5-56c5-4f52-a0a8-6f9d4d9bd11e";
    const J1: &str = "19f4f1d8-0d4c-4e74-9c06-5c3dbb6f9a12";

    fn cmd(op: Value) -> Value {
        json!({
            "command_id": "7bb126f8-f114-4d62-b7f4-1a6d0e4f5b1a",
            "transaction_id": "1b1a5aa2-2d2d-4db1-9d43-95f7a8f4a3c9",
            "project_id": PID,
            "engine_epoch": "1",
            "expected_revision": "0",
            "op": op,
        })
    }

    fn decode(bytes: &[u8]) -> proto::PersistentCommand<'_> {
        let env = flatbuffers::root::<proto::ControlEnvelope>(bytes).unwrap();
        env.frame_as_request_frame()
            .unwrap()
            .request_as_persistent_command()
            .unwrap()
    }

    #[test]
    fn rev2_tagged_ops_roundtrip() {
        let bytes = persistent_command_json(&cmd(json!({
            "ArmTrackOp": {
                "track_id": T1,
                "record_enabled": true,
                "input_device": "mic-1",
                "monitor_mode": "ON",
                "is_midi": false,
            }
        })))
        .unwrap();
        let c = decode(&bytes);
        assert_eq!(c.op_type(), proto::PersistentOp::ArmTrackOp);
        let op = c.op_as_arm_track_op().unwrap();
        assert_eq!(op.track_id(), Some(T1));
        assert_eq!(op.monitor_mode(), proto::MonitorMode::ON);

        let bytes = persistent_command_json(&cmd(json!({
            "SetMetronomeOp": { "enabled": true, "gain": 0.75, "recording_only": true }
        })))
        .unwrap();
        let c = decode(&bytes);
        let op = c.op_as_set_metronome_op().unwrap();
        assert!(op.enabled() && op.recording_only());
        assert!((op.gain() - 0.75).abs() < 1e-6);

        let bytes = persistent_command_json(&cmd(json!({
            "LaunchSceneOp": { "scene_id": "chorus", "quantize": "CUSTOM", "quantize_ticks": "480" }
        })))
        .unwrap();
        let c = decode(&bytes);
        let op = c.op_as_launch_scene_op().unwrap();
        assert_eq!(op.quantize(), proto::LaunchQuantize::CUSTOM);
        assert_eq!(op.quantize_ticks(), 480);
    }

    #[test]
    fn rev2_flat_and_tagged_job_ops() {
        let spec = json!({
            "jobId": J1,
            "projectId": PID,
            "sourceRevision": "12",
            "contextSha256": "a".repeat(64),
            "kind": "av_export",
            "runtimeId": "void-export",
            "runtimeSha256": "b".repeat(64),
            "inputs": ["sha256:c".to_string()],
            "parameters": {"format": "wav"},
            "reservations": {"ramBytes": "1024", "vramBytes": "0", "cpuThreads": 2},
            "deadlineMonotonicNs": "999",
            "outputScopeToken": "export:19f4f1d8-0d4c-4e74-9c06-5c3dbb6f9a12"
        });
        // Flat pinned form: {"op":"submit_job","spec":{…}}
        let flat =
            persistent_command_json(&cmd(json!({"op": "submit_job", "spec": spec}))).unwrap();
        // Tagged form: {"SubmitJobOp":{"spec":{…}}}
        let tagged = persistent_command_json(&cmd(json!({"SubmitJobOp": {"spec": spec}}))).unwrap();
        for bytes in [flat, tagged] {
            let c = decode(&bytes);
            assert_eq!(c.op_type(), proto::PersistentOp::SubmitJobOp);
            let s = c.op_as_submit_job_op().unwrap().spec().unwrap();
            assert_eq!(s.job_id(), Some(J1));
            assert_eq!(s.kind(), Some("av_export"));
            assert_eq!(s.source_revision(), 12);
            assert_eq!(s.inputs().unwrap().len(), 1);
            assert_eq!(s.parameters_json(), Some("{\"format\":\"wav\"}"));
        }
        // Flat cancel.
        let flat = persistent_command_json(&cmd(json!({"op": "cancel_job", "jobId": J1}))).unwrap();
        let c = decode(&flat);
        assert_eq!(c.op_as_cancel_job_op().unwrap().job_id(), Some(J1));
    }

    #[test]
    fn rev2_preview_layer_and_misc_ops() {
        let bytes = persistent_command_json(&cmd(json!({
            "PreviewLayerOp": {
                "proposal_id": "prop-1",
                "clip_id": "clip-1",
                "enable": true,
                "notes": [{"pitch": 60, "velocity": 100, "start_ticks": "0", "length_ticks": "480"}],
            }
        })))
        .unwrap();
        let c = decode(&bytes);
        let op = c.op_as_preview_layer_op().unwrap();
        assert_eq!(op.notes().unwrap().len(), 1);
        assert_eq!(op.notes().unwrap().get(0).pitch(), 60);

        for op in [
            json!({"StartRecordingOp": {"take_id": T1}}),
            json!({"StopRecordingOp": {"discard": true}}),
            json!({"SetCountInOp": {"mode": "bars", "bars": 2}}),
            json!({"SetPunchInOutOp": {"enabled": true, "in_ticks": "0", "out_ticks": "960"}}),
            json!({"PauseJobOp": {"job_id": J1}}),
            json!({"InstallModelOp": {"model_id": "musicgen", "model_version": "1.0", "source_uri": "file:///m"}}),
            json!({"RequestProposalOp": {"context_digest": "abc", "seed": "7", "max_proposals": 2}}),
            json!({"ResolveProposalOp": {"proposal_id": "p1", "accept": true, "candidate_rank": 0}}),
            json!({"IngestAssetOp": {"rel_path": "audio/x.wav", "media_type": "wav"}}),
            json!({"RelinkAssetOp": {"asset_id": T1, "sha256": "d".repeat(64)}}),
            json!({"SetPluginBypassOp": {"plugin_instance_id": T1, "bypassed": true}}),
            json!({"RescanPluginsOp": {}}),
            json!({"RestorePluginStateOp": {"plugin_instance_id": T1, "state_asset_id": J1}}),
            json!({"SaveProjectAsOp": {"container_dir": "/tmp/x.void", "name": "x", "reason": "save-as"}}),
            json!({"StopSceneOp": {}}),
            json!({"LaunchClipOp": {"slot_id": "s0", "quantize": "IMMEDIATE"}}),
        ] {
            persistent_command_json(&cmd(op)).expect("op should encode");
        }
    }

    #[test]
    fn rev2_read_request_views() {
        let req = json!({
            "request_id": "r1",
            "project_id": PID,
            "view": "job_list",
            "includeTerminal": true,
        });
        let bytes = read_request_json(&req).unwrap();
        let env = flatbuffers::root::<proto::ControlEnvelope>(&bytes).unwrap();
        let rr = env
            .frame_as_request_frame()
            .unwrap()
            .request_as_read_request()
            .unwrap();
        assert_eq!(rr.view(), proto::ViewKind::JOB_LIST);
        assert!(rr.include_terminal());
        for view in [
            "take_list",
            "input_device_list",
            "model_list",
            "proposal_list",
            "scene_list",
        ] {
            read_request_json(&json!({
                "request_id": "r", "project_id": PID, "view": view
            }))
            .expect(view);
        }
    }

    #[test]
    fn rev2_telemetry_events_decode() {
        let mut b = FlatBufferBuilder::new();
        let pid = b.create_string(PID);
        let jid = b.create_string(J1);
        let st = b.create_string("running");
        let ev = proto::JobEvent::create(
            &mut b,
            &proto::JobEventArgs {
                project_id: Some(pid),
                job_id: Some(jid),
                status: Some(st),
                percent: 42.5,
                message: None,
                quarantined: false,
            },
        );
        let tf = proto::TelemetryFrame::create(
            &mut b,
            &proto::TelemetryFrameArgs {
                event_type: proto::TelemetryEvent::JobEvent,
                event: Some(ev.as_union_value()),
            },
        );
        b.finish_minimal(tf);
        let out = telemetry_json(b.finished_data()).unwrap().unwrap();
        assert_eq!(out["kind"], "JobEvent");
        assert_eq!(out["job_id"], J1);
        assert_eq!(out["status"], "running");
        assert_eq!(out["percent"], 42.5);
    }
}
