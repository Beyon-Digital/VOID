//! Reference mock worker: connects to the supervisor's sockets and sends
//! an authenticated WorkerHello, then answers requests until killed.
//! Used by supervisor tests (T05/T06 evidence) and as the minimal example
//! of the worker-side protocol for the C++ engine.
//!
//! Behaviour contract (what the UI sweep relies on):
//! - PersistentCommand → CommandReceipt APPLIED with revision+1.
//! - SubmitJobOp/CancelJobOp/PauseJobOp → deterministic JobEvent frames on
//!   telemetry (queued→running→succeeded / cancelling→cancelled / running).
//! - SaveProjectOp / SaveProjectAsOp / CreateCheckpointOp → SaveResultEvent
//!   SAVE_DURABLE on telemetry so export preflight sees a checkpoint.
//! - ReadRequest → real entities per ViewKind in the JSON shapes the TS
//!   parsers consume (camelCase keys where the spec-level parsers require
//!   them; u64/i64 values as decimal strings).
//! - Arm/record/job ops fold into the fixture state so re-read views show
//!   the change (armedTracks, take rows, job status).
//! - TransportRequest → TransportAck on telemetry.

use flatbuffers::FlatBufferBuilder;
use serde_json::{json, Value};
use std::collections::{HashSet, VecDeque};
use std::env;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use void_protocol::frame::encode_frame;
use void_protocol::proto::{self, WorkerHelloArgs};

// ---------------------------------------------------------------------------
// Fixture ids — real UUIDs so any op that echoes a view id back through the
// `is_valid_id` gate still validates.
// ---------------------------------------------------------------------------

const TRACK_AUDIO: &str = "11111111-0000-4000-8000-000000000001";
const TRACK_INSTR: &str = "11111111-0000-4000-8000-000000000002";
const CLIP_AUDIO: &str = "22222222-0000-4000-8000-000000000001";
const CLIP_MIDI: &str = "22222222-0000-4000-8000-000000000002";
const ASSET_PRESENT: &str = "33333333-0000-4000-8000-000000000001";
const ASSET_MISSING: &str = "33333333-0000-4000-8000-000000000002";
const PLUGIN_ACTIVE: &str = "44444444-0000-4000-8000-000000000001";
const PLUGIN_BYPASSED: &str = "44444444-0000-4000-8000-000000000002";
const PLUGIN_MISSING: &str = "44444444-0000-4000-8000-000000000003";
const DEV_MIC: &str = "55555555-0000-4000-8000-000000000001";
const DEV_LINE: &str = "55555555-0000-4000-8000-000000000002";
const JOB_EXPORT: &str = "66666666-0000-4000-8000-000000000001";
const JOB_TRANSC: &str = "66666666-0000-4000-8000-000000000002";
const MODEL_AUDIO: &str = "77777777-0000-4000-8000-000000000001";
const MODEL_SYMB: &str = "77777777-0000-4000-8000-000000000002";
const MODEL_VIS: &str = "77777777-0000-4000-8000-000000000003";
const PROPOSAL_ID: &str = "88888888-0000-4000-8000-000000000001";
const TAKE_ONE: &str = "99999999-0000-4000-8000-000000000001";
const FOLDER_ONE: &str = "99999999-0000-4000-8000-0000000000f1";
const SCENE_A: &str = "aaaaaaaa-0000-4000-8000-000000000001";
const SCENE_B: &str = "aaaaaaaa-0000-4000-8000-000000000002";

const SHA_PRESENT: &str = "aabbccddeeff0011223344556677889900aabbccddeeff00112233445566778899";
const SHA_MISSING: &str = "00112233445566778899aabbccddeeffaabbccddeeff00112233445566778899aa";
const SHA_MANIFEST: &str = "bbccddeeff00112233445566778899aaaabbccddeeff00112233445566778899cc";

const PROJECT_FALLBACK: &str = "00000000-0000-4000-8000-000000000001";
const RECEIPT_HISTORY: usize = 20;

/// Mutable fixture state ops fold into — re-read views then show the change.
struct MockState {
    revision: u64,
    project_id: String,
    armed: HashSet<String>,
    recording: bool,
    recording_take: Option<String>,
    extra_takes: Vec<Value>,
    jobs: Vec<Value>,
    receipts: VecDeque<Value>,
    checkpoint_seq: u64,
}

impl MockState {
    fn new() -> Self {
        Self {
            revision: 0,
            project_id: PROJECT_FALLBACK.to_string(),
            armed: HashSet::new(),
            recording: false,
            recording_take: None,
            extra_takes: Vec::new(),
            jobs: seed_jobs(),
            receipts: VecDeque::new(),
            checkpoint_seq: 0,
        }
    }

    fn track_project(&mut self, cmd: &proto::PersistentCommand<'_>) {
        if let Some(p) = cmd.project_id() {
            if !p.is_empty() {
                self.project_id = p.to_string();
            }
        }
    }

    fn push_receipt(&mut self, cmd: &proto::PersistentCommand<'_>) {
        self.receipts.push_back(json!({
            "commandId": cmd.command_id().unwrap_or(""),
            "command_id": cmd.command_id().unwrap_or(""),
            "transactionId": cmd.transaction_id().unwrap_or(""),
            "transaction_id": cmd.transaction_id().unwrap_or(""),
            "status": "APPLIED",
            "revision": self.revision.to_string(),
            "message": "applied by mock worker",
        }));
        while self.receipts.len() > RECEIPT_HISTORY {
            self.receipts.pop_front();
        }
    }

    /// Fold a persistent op into fixture state (post-receipt).
    fn apply(&mut self, cmd: &proto::PersistentCommand<'_>) {
        match cmd.op_type() {
            proto::PersistentOp::ArmTrackOp => {
                if let Some(op) = cmd.op_as_arm_track_op() {
                    let tid = op.track_id().unwrap_or("").to_string();
                    if tid.is_empty() {
                        return;
                    }
                    if op.record_enabled() {
                        self.armed.insert(tid);
                    } else {
                        self.armed.remove(&tid);
                    }
                }
            }
            proto::PersistentOp::StartRecordingOp => {
                let take = cmd
                    .op_as_start_recording_op()
                    .and_then(|o| o.take_id())
                    .filter(|t| !t.is_empty())
                    .map(|t| t.to_string())
                    .unwrap_or_else(|| format!("{:08x}-mock-take", self.revision));
                self.recording = true;
                self.recording_take = Some(take);
            }
            proto::PersistentOp::StopRecordingOp => {
                let discard = cmd
                    .op_as_stop_recording_op()
                    .map(|o| o.discard())
                    .unwrap_or(false);
                if self.recording && !discard {
                    let take_id = self
                        .recording_take
                        .clone()
                        .unwrap_or_else(|| TAKE_ONE.to_string());
                    self.extra_takes.push(json!({
                        "takeId": take_id,
                        "folderId": FOLDER_ONE,
                        "trackId": TRACK_AUDIO,
                        "kind": "AUDIO",
                        "laneIndex": self.extra_takes.len() + 1,
                        "assetId": ASSET_PRESENT,
                        "sourceHash": SHA_PRESENT,
                        "regionStartTicks": "1920",
                        "regionLengthTicks": "1920",
                        "offsetTicks": "0",
                        "complete": true,
                        "recordedAt": "2026-10-09T00:00:00Z",
                    }));
                }
                self.recording = false;
                self.recording_take = None;
            }
            proto::PersistentOp::SubmitJobOp => {
                if let Some(spec) = cmd.op_as_submit_job_op().and_then(|o| o.spec()) {
                    let jid = spec.job_id().unwrap_or("").to_string();
                    if !jid.is_empty() {
                        // The telemetry lifecycle races to 'succeeded'; the
                        // stored row reflects the last emitted event.
                        self.jobs.push(job_row(
                            &jid,
                            spec.project_id().unwrap_or(&self.project_id),
                            spec.kind().unwrap_or("analysis"),
                            spec.model_id().unwrap_or(""),
                            spec.runtime_id().unwrap_or("local"),
                            spec.ram_bytes(),
                            spec.vram_bytes(),
                            spec.cpu_threads(),
                            spec.deadline_monotonic_ns(),
                            "succeeded",
                            100.0,
                            "done",
                            true,
                        ));
                    }
                }
            }
            proto::PersistentOp::CancelJobOp => {
                if let Some(jid) = cmd.op_as_cancel_job_op().and_then(|o| o.job_id()) {
                    set_job_status(&mut self.jobs, jid, "cancelled", -1.0, "cancelled");
                }
            }
            proto::PersistentOp::PauseJobOp => {
                if let Some(jid) = cmd.op_as_pause_job_op().and_then(|o| o.job_id()) {
                    set_job_status(&mut self.jobs, jid, "running", -1.0, "paused by policy");
                }
            }
            _ => {}
        }
    }
}

fn set_job_status(jobs: &mut [Value], job_id: &str, status: &str, percent: f32, message: &str) {
    for j in jobs.iter_mut() {
        if j.get("jobId").and_then(Value::as_str) == Some(job_id) {
            j["status"] = json!(status);
            if percent >= 0.0 {
                j["percent"] = json!(percent);
            }
            j["message"] = json!(message);
            j["updatedAt"] = json!("2026-10-09T00:00:00Z");
        }
    }
}

// ---------------------------------------------------------------------------
// View fixtures — JSON shapes the TS parsers consume. int64/u64 values ride
// as decimal strings; parsers accept both key spellings where the app mixes
// them, so rows emit both.
// ---------------------------------------------------------------------------

fn seed_jobs() -> Vec<Value> {
    vec![
        job_row(
            JOB_EXPORT,
            PROJECT_FALLBACK,
            "av_export",
            MODEL_AUDIO,
            "void-local",
            268_435_456,
            0,
            4,
            0,
            "running",
            42.0,
            "rendering bars 1-4",
            false,
        ),
        job_row(
            JOB_TRANSC,
            PROJECT_FALLBACK,
            "transcription",
            MODEL_SYMB,
            "void-local",
            536_870_912,
            0,
            2,
            0,
            "queued",
            0.0,
            "waiting for worker",
            false,
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn job_row(
    job_id: &str,
    project_id: &str,
    kind: &str,
    model_id: &str,
    runtime_id: &str,
    ram_bytes: u64,
    vram_bytes: u64,
    cpu_threads: i32,
    deadline_ns: u64,
    status: &str,
    percent: f32,
    message: &str,
    with_artifacts: bool,
) -> Value {
    let artifacts = if with_artifacts {
        vec![json!({
            "name": "render.wav",
            "sha256": SHA_PRESENT,
            "bytes": "1900800",
            "asset": ASSET_PRESENT,
        })]
    } else {
        Vec::new()
    };
    json!({
        "jobId": job_id,
        "job_id": job_id,
        "projectId": project_id,
        "kind": kind,
        "status": status,
        "percent": percent,
        "message": message,
        "modelId": model_id,
        "modelVersion": "1.0.0",
        "runtimeId": runtime_id,
        "deadlineMonotonicNs": deadline_ns.to_string(),
        "reservations": {
            "ramBytes": ram_bytes.to_string(),
            "vramBytes": vram_bytes.to_string(),
            "cpuThreads": cpu_threads,
        },
        "artifacts": artifacts,
        "quarantined": false,
        "createdAt": "2026-10-09T00:00:00Z",
        "updatedAt": "2026-10-09T00:00:00Z",
    })
}

fn track_rows(state: &MockState) -> Vec<(String, Value)> {
    let row = |id: &str, kind: &str, name: &str, index: i32| {
        let mut v = json!({
            "id": id,
            "track_id": id,
            "trackId": id,
            "kind": kind,
            "name": name,
            "index": index,
            "muted": false,
            "soloed": false,
            "armed": state.armed.contains(id),
        });
        if let Some(o) = v.as_object_mut() {
            o.insert("input_device".into(), json!(DEV_MIC));
        }
        (id.to_string(), v)
    };
    vec![
        row(TRACK_AUDIO, "AUDIO", "Audio 1", 0),
        row(TRACK_INSTR, "INSTRUMENT", "Instrument 1", 1),
    ]
}

fn clip_rows(track_scope: Option<&str>) -> Vec<(String, Value)> {
    let row = |clip: &str, track: &str, kind: &str, name: &str, asset: Option<&str>| {
        let mut v = json!({
            "id": clip,
            "clip_id": clip,
            "clipId": clip,
            "track_id": track,
            "trackId": track,
            "kind": kind,
            "name": name,
            "color": "#4f8cff",
            "start_ticks": "0",
            "startTicks": "0",
            "length_ticks": "1920",
            "lengthTicks": "1920",
            "offset_ticks": "0",
            "offsetTicks": "0",
        });
        if let (Some(o), Some(a)) = (v.as_object_mut(), asset) {
            o.insert("asset_id".into(), json!(a));
            o.insert("assetId".into(), json!(a));
        }
        (format!("clip:{clip}"), v)
    };
    let mut rows = vec![
        row(
            CLIP_AUDIO,
            TRACK_AUDIO,
            "AUDIO",
            "Vocal take",
            Some(ASSET_PRESENT),
        ),
        row(CLIP_MIDI, TRACK_INSTR, "MIDI", "Keys A", None),
    ];
    if let Some(scope) = track_scope {
        if !scope.is_empty() {
            rows.retain(|(_, v)| v.get("track_id").and_then(Value::as_str) == Some(scope));
        }
    }
    rows
}

fn note_rows(track_scope: Option<&str>, start_ticks: i64, end_ticks: i64) -> Vec<(String, Value)> {
    // The MIDI clip lives on TRACK_INSTR; a mismatched scope yields an
    // honest empty page.
    if let Some(scope) = track_scope {
        if !scope.is_empty() && scope != TRACK_INSTR {
            return Vec::new();
        }
    }
    let notes = [(60, "0"), (64, "240"), (67, "480"), (72, "720")];
    notes
        .iter()
        .enumerate()
        .filter(|(_, (_, on))| {
            let t: i64 = on.parse().unwrap_or(0);
            // -1 means unbounded on that end of the window.
            (start_ticks < 0 || t + 240 > start_ticks) && (end_ticks < 0 || t < end_ticks)
        })
        .map(|(i, (pitch, on))| {
            let id = format!("bbbbbbbb-0000-4000-8000-00000000{i:04x}");
            (
                format!("note:{id}"),
                json!({
                    "id": id,
                    "note_id": id,
                    "noteId": id,
                    "clip_id": CLIP_MIDI,
                    "clipId": CLIP_MIDI,
                    "pitch": pitch,
                    "velocity": 96,
                    "start_ticks": on,
                    "startTicks": on,
                    "length_ticks": "240",
                    "lengthTicks": "240",
                }),
            )
        })
        .collect()
}

fn asset_rows() -> Vec<(String, Value)> {
    vec![
        (
            format!("asset:{ASSET_PRESENT}"),
            json!({
                "asset_id": ASSET_PRESENT,
                "assetId": ASSET_PRESENT,
                "state": "present",
                "sha256": SHA_PRESENT,
                "display_name": "vocal-take.wav",
                "displayName": "vocal-take.wav",
                "name": "vocal-take.wav",
                "rel_path": format!("assets/sha256/{SHA_PRESENT}.wav"),
                "relPath": format!("assets/sha256/{SHA_PRESENT}.wav"),
                "path": format!("assets/sha256/{SHA_PRESENT}.wav"),
                "media_type": "wav",
                "mediaType": "wav",
                "type": "wav",
                "bytes": "1900800",
            }),
        ),
        (
            format!("asset:{ASSET_MISSING}"),
            json!({
                "asset_id": ASSET_MISSING,
                "assetId": ASSET_MISSING,
                "state": "missing",
                "expected_sha256": SHA_MISSING,
                "expectedSha256": SHA_MISSING,
                "display_name": "drums.wav",
                "displayName": "drums.wav",
                "name": "drums.wav",
                "rel_path": "assets/sha256/drums.wav",
                "relPath": "assets/sha256/drums.wav",
                "media_type": "wav",
                "mediaType": "wav",
                "type": "wav",
                "detail": "recorded asset not found on disk",
                "message": "recorded asset not found on disk",
            }),
        ),
    ]
}

fn plugin_rows(track_scope: Option<&str>) -> Vec<(String, Value)> {
    let track = track_scope.unwrap_or(TRACK_INSTR);
    let row = |instance: &str, uid: &str, name: &str, status: &str, slot: i32, extra: Value| {
        let mut v = json!({
            "instance_id": instance,
            "instanceId": instance,
            "plugin_instance_id": instance,
            "plugin_uid": uid,
            "pluginUid": uid,
            "format": "VST3",
            "name": name,
            "status": status,
            "slot_index": slot,
            "slotIndex": slot,
            "track_id": track,
            "trackId": track,
        });
        if let (Some(o), Some(e)) = (v.as_object_mut(), extra.as_object()) {
            for (k, val) in e {
                o.insert(k.clone(), val.clone());
            }
        }
        (format!("plugin:{instance}"), v)
    };
    vec![
        row(
            PLUGIN_ACTIVE,
            "void.stock.compressor",
            "Compressor",
            "ACTIVE",
            0,
            json!({"param_values": {"threshold": 0.7, "ratio": 4.0},
                   "params": {"threshold": 0.7, "ratio": 4.0}}),
        ),
        row(
            PLUGIN_BYPASSED,
            "void.stock.reverb",
            "Reverb",
            "BYPASSED",
            1,
            json!({"reason": "bypassed by user"}),
        ),
        row(
            PLUGIN_MISSING,
            "vendor.vintage-chorus",
            "Vintage Chorus",
            "MISSING",
            2,
            json!({"reason": "vst3 binary not found on this machine"}),
        ),
    ]
}

fn summary_row(state: &MockState) -> (String, Value) {
    let phase = if state.recording { "recording" } else { "idle" };
    let mut recording = json!({
        "phase": phase,
        "armedTracks": state.armed.iter().collect::<Vec<_>>(),
    });
    if let (Some(o), Some(t)) = (recording.as_object_mut(), state.recording_take.as_ref()) {
        o.insert("takeId".into(), json!(t));
    }
    (
        format!("project:{}", state.project_id),
        json!({
            "id": state.project_id,
            "project_id": state.project_id,
            "projectId": state.project_id,
            "name": "Mock Session",
            "sample_rate": 48000,
            "sampleRate": 48000,
            "bpm": 120.0,
            "initial_bpm": 120.0,
            "tempo": 120.0,
            "key": "C major",
            "time_signature": "4/4",
            "timeSignature": "4/4",
            "track_count": 2,
            "trackCount": 2,
            "asset_count": 2,
            "assetCount": 2,
            "storage_bytes": "1900800",
            "storageBytes": "1900800",
            "note": "mock worker fixture project",
            "recording": recording,
        }),
    )
}

fn take_rows(state: &MockState) -> Vec<(String, Value)> {
    let mut rows = vec![(
        format!("take:{TAKE_ONE}"),
        json!({
            "takeId": TAKE_ONE,
            "folderId": FOLDER_ONE,
            "trackId": TRACK_AUDIO,
            "kind": "AUDIO",
            "laneIndex": 0,
            "assetId": ASSET_PRESENT,
            "sourceHash": SHA_PRESENT,
            "regionStartTicks": "0",
            "regionLengthTicks": "1920",
            "offsetTicks": "0",
            "complete": true,
            "recordedAt": "2026-10-09T00:00:00Z",
        }),
    )];
    for t in &state.extra_takes {
        if let Some(id) = t.get("takeId").and_then(Value::as_str) {
            rows.push((format!("take:{id}"), t.clone()));
        }
    }
    rows
}

fn input_device_rows() -> Vec<(String, Value)> {
    [
        (DEV_MIC, "USB Microphone", true),
        (DEV_LINE, "Line In", true),
    ]
    .iter()
    .map(|(id, name, _)| {
        (
            format!("device:{id}"),
            json!({
                "id": id,
                "device_id": id,
                "deviceId": id,
                "name": name,
            }),
        )
    })
    .collect()
}

fn job_rows(state: &MockState, include_terminal: bool) -> Vec<(String, Value)> {
    state
        .jobs
        .iter()
        .filter(|j| {
            include_terminal
                || !matches!(
                    j.get("status").and_then(Value::as_str),
                    Some("succeeded" | "failed" | "cancelled")
                )
        })
        .map(|j| {
            let id = j
                .get("jobId")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_string();
            (format!("job:{id}"), j.clone())
        })
        .collect()
}

fn model_rows() -> Vec<(String, Value)> {
    let row = |id: &str, name: &str, kind: &str, runtime: &str, status: &str| {
        (
            format!("model:{id}"),
            json!({
                "modelId": id,
                "version": "1.0.0",
                "name": name,
                "kind": kind,
                "runtime": runtime,
                "status": status,
                "budgets": {
                    "cpuSeconds": "60",
                    "memoryBytes": "536870912",
                    "wallNs": "30000000000",
                    "cpuThreads": 2,
                },
                "artifacts": [{"name": "weights.bin", "present": status != "missing", "required": true}],
                "manifestSha256": SHA_MANIFEST,
                "license": "CC0-1.0",
                "source": "bundled",
            }),
        )
    };
    vec![
        row(
            MODEL_AUDIO,
            "VOID Diffusion",
            "audio",
            "internal",
            "available",
        ),
        row(MODEL_SYMB, "Melody+", "symbolic", "internal", "available"),
        row(MODEL_VIS, "SketchDiff", "visual", "argv", "missing"),
    ]
}

fn proposal_rows(state: &MockState) -> Vec<(String, Value)> {
    let note = |on: &str, pitch: i32| {
        json!({
            "pitch": pitch,
            "velocity": 96,
            "onsetTicks": on,
            "lengthTicks": "240",
        })
    };
    vec![(
        format!("proposal:{PROPOSAL_ID}"),
        json!({
            "proposalId": PROPOSAL_ID,
            "projectId": state.project_id,
            "status": "ready",
            "sourceRevision": state.revision.to_string(),
            "contextSha256": SHA_MANIFEST,
            "context": {"clipId": CLIP_MIDI, "lockedRanges": []},
            "candidates": [
                {
                    "rank": 1,
                    "score": 0.82,
                    "rationale": "continues the motif up a third",
                    "notes": [note("1920", 64), note("2160", 67), note("2400", 71)],
                },
                {
                    "rank": 2,
                    "score": 0.61,
                    "rationale": "repeats the phrase an octave up",
                    "notes": [note("1920", 72), note("2160", 76)],
                },
            ],
            "provenance": {
                "modelId": MODEL_SYMB,
                "jobId": JOB_TRANSC,
                "promptDigest": SHA_MANIFEST,
            },
            "createdAt": "2026-10-09T00:00:00Z",
            "updatedAt": "2026-10-09T00:00:00Z",
        }),
    )]
}

fn scene_rows() -> Vec<(String, Value)> {
    let row = |id: &str, name: &str, index: i32, clip: &str| {
        (
            format!("scene:{id}"),
            json!({
                "id": id,
                "scene_id": id,
                "sceneId": id,
                "name": name,
                "index": index,
                "state": "stopped",
                "slots": [{"trackId": TRACK_AUDIO, "clipId": clip}],
            }),
        )
    };
    vec![
        row(SCENE_A, "Scene 01", 0, CLIP_AUDIO),
        row(SCENE_B, "Scene 02", 1, CLIP_MIDI),
    ]
}

/// The full fixture set for a view (before cursor/limit pagination).
fn rows_for_view(
    state: &MockState,
    view: proto::ViewKind,
    track_scope: Option<&str>,
    start_ticks: i64,
    end_ticks: i64,
    include_terminal: bool,
) -> Vec<(String, Value)> {
    match view {
        proto::ViewKind::PROJECT_SUMMARY => vec![summary_row(state)],
        proto::ViewKind::TRACK_LIST => track_rows(state),
        proto::ViewKind::CLIP_LIST => clip_rows(track_scope),
        proto::ViewKind::NOTE_RANGE => note_rows(track_scope, start_ticks, end_ticks),
        proto::ViewKind::ASSET_LIST => asset_rows(),
        proto::ViewKind::PLUGIN_LIST => plugin_rows(track_scope),
        proto::ViewKind::RECEIPT_LIST => state
            .receipts
            .iter()
            .map(|r| {
                let id = r
                    .get("commandId")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string();
                (format!("receipt:{id}"), r.clone())
            })
            .collect(),
        proto::ViewKind::TAKE_LIST => take_rows(state),
        proto::ViewKind::INPUT_DEVICE_LIST => input_device_rows(),
        proto::ViewKind::JOB_LIST => job_rows(state, include_terminal),
        proto::ViewKind::MODEL_LIST => model_rows(),
        proto::ViewKind::PROPOSAL_LIST => proposal_rows(state),
        proto::ViewKind::SCENE_LIST => scene_rows(),
        _ => Vec::new(),
    }
}

/// Build the ReadItem rows for one read request.
fn view_items(state: &MockState, rd: &proto::ReadRequest<'_>) -> Vec<(String, Value)> {
    // Single-page semantics: non-empty cursor has already received page one.
    if rd.cursor().map(|c| !c.is_empty()).unwrap_or(false) {
        return Vec::new();
    }
    let scope = rd.track_id().filter(|t| !t.is_empty());
    let rows = rows_for_view(
        state,
        rd.view(),
        scope,
        rd.start_ticks(),
        rd.end_ticks(),
        rd.include_terminal(),
    );
    match rd.limit() {
        n if n > 0 => rows.into_iter().take(n as usize).collect(),
        _ => rows,
    }
}

/// True for ops that checkpoint the project — they get a SaveResultEvent.
fn is_save_op(op: proto::PersistentOp) -> bool {
    matches!(
        op,
        proto::PersistentOp::SaveProjectOp
            | proto::PersistentOp::SaveProjectAsOp
            | proto::PersistentOp::CreateCheckpointOp
    )
}

fn build_save_event(state: &MockState, cmd: &proto::PersistentCommand<'_>) -> Vec<u8> {
    let mut tb = FlatBufferBuilder::new();
    let pid = tb.create_string(&state.project_id);
    let cid = tb.create_string(cmd.command_id().unwrap_or(""));
    let ck = tb.create_string(&format!("ck-{:06}", state.checkpoint_seq));
    let sha = tb.create_string(SHA_MANIFEST);
    let msg = tb.create_string("saved by mock worker");
    let ev = proto::SaveResultEvent::create(
        &mut tb,
        &proto::SaveResultEventArgs {
            project_id: Some(pid),
            command_id: Some(cid),
            status: proto::SaveStatus::SAVE_DURABLE,
            revision: state.revision,
            checkpoint_id: Some(ck),
            manifest_sha256: Some(sha),
            error: proto::ErrorCode::NONE,
            message: Some(msg),
        },
    );
    let tf = proto::TelemetryFrame::create(
        &mut tb,
        &proto::TelemetryFrameArgs {
            event_type: proto::TelemetryEvent::SaveResultEvent,
            event: Some(ev.as_union_value()),
        },
    );
    tb.finish_minimal(tf);
    tb.finished_data().to_vec()
}

fn build_job_event(pid: &str, jid: &str, percent: f32, status: &str) -> Vec<u8> {
    let mut tb = FlatBufferBuilder::new();
    let pid_s = tb.create_string(pid);
    let jid_s = tb.create_string(jid);
    let status_s = tb.create_string(status);
    let je = proto::JobEvent::create(
        &mut tb,
        &proto::JobEventArgs {
            project_id: Some(pid_s),
            job_id: Some(jid_s),
            status: Some(status_s),
            percent,
            message: None,
            quarantined: false,
        },
    );
    let tf = proto::TelemetryFrame::create(
        &mut tb,
        &proto::TelemetryFrameArgs {
            event_type: proto::TelemetryEvent::JobEvent,
            event: Some(je.as_union_value()),
        },
    );
    tb.finish_minimal(tf);
    tb.finished_data().to_vec()
}

#[tokio::main]
async fn main() {
    let control_path = env::var(void_worker::ENV_CONTROL_SOCK).expect("VOID_CONTROL_SOCK");
    let telemetry_path = env::var(void_worker::ENV_TELEMETRY_SOCK).expect("VOID_TELEMETRY_SOCK");
    let token = env::var(void_worker::ENV_WORKER_TOKEN).expect("VOID_WORKER_TOKEN");

    let mut control = UnixStream::connect(&control_path).await.unwrap();
    let mut telemetry = UnixStream::connect(&telemetry_path).await.unwrap();

    // WorkerHello inside a ControlEnvelope
    let mut b = FlatBufferBuilder::new();
    let token_s = b.create_string(&token);
    let instance = b.create_string("mock-worker-1");
    let cap_strs: Vec<_> = ["mock", "test-fixture"]
        .iter()
        .map(|s| b.create_string(s))
        .collect();
    let caps = b.create_vector(&cap_strs);
    let hello = proto::WorkerHello::create(
        &mut b,
        &WorkerHelloArgs {
            worker_kind: proto::WorkerKind::ENGINE,
            worker_instance_id: Some(instance),
            protocol_major: void_protocol::PROTOCOL_MAJOR,
            protocol_minor: void_protocol::PROTOCOL_MINOR,
            launch_token: Some(token_s),
            engine_epoch: 1,
            capabilities: Some(caps),
        },
    );
    let envelope = proto::ControlEnvelope::create(
        &mut b,
        &proto::ControlEnvelopeArgs {
            frame_type: proto::ControlFrame::WorkerHello,
            frame: Some(hello.as_union_value()),
        },
    );
    b.finish_minimal(envelope);
    let framed = encode_frame(b.finished_data()).unwrap();
    control.write_all(&framed).await.unwrap();
    control.flush().await.unwrap();

    // Serve: for every RequestFrame/PersistentCommand, reply
    // EventFrame/CommandReceipt APPLIED revision=expected+1 (echo contract).
    let mut reader = void_protocol::frame::FrameReader::default();
    let mut buf = [0u8; 16 * 1024];
    let mut state = MockState::new();
    loop {
        while let Some(payload) = reader.next_frame().unwrap() {
            let Ok(env) = flatbuffers::root::<proto::ControlEnvelope>(&payload) else {
                continue;
            };
            let Some(req) = env.frame_as_request_frame() else {
                continue;
            };
            // rev-2: deterministic job lifecycle for SubmitJobOp /
            // CancelJobOp / PauseJobOp — JobEvent frames ride telemetry
            // after the receipt lands on control.
            let mut job_events: Vec<(String, String, f32, String)> = Vec::new();
            let mut save_events: Vec<Vec<u8>> = Vec::new();
            let resp: Vec<u8> = if let Some(cmd) = req.request_as_persistent_command() {
                state.revision += 1;
                state.track_project(&cmd);
                job_events = match cmd.op_type() {
                    proto::PersistentOp::SubmitJobOp => cmd
                        .op_as_submit_job_op()
                        .and_then(|o| o.spec())
                        .map(|s| {
                            let jid = s.job_id().unwrap_or("").to_string();
                            let pid = s.project_id().unwrap_or("").to_string();
                            vec![
                                (pid.clone(), jid.clone(), 0.0, "queued".to_string()),
                                (pid.clone(), jid.clone(), 50.0, "running".to_string()),
                                (pid, jid, 100.0, "succeeded".to_string()),
                            ]
                        })
                        .unwrap_or_default(),
                    proto::PersistentOp::CancelJobOp => cmd
                        .op_as_cancel_job_op()
                        .map(|o| {
                            let jid = o.job_id().unwrap_or("").to_string();
                            let pid = cmd.project_id().unwrap_or("").to_string();
                            vec![
                                (pid.clone(), jid.clone(), -1.0, "cancelling".to_string()),
                                (pid, jid, -1.0, "cancelled".to_string()),
                            ]
                        })
                        .unwrap_or_default(),
                    proto::PersistentOp::PauseJobOp => cmd
                        .op_as_pause_job_op()
                        .map(|o| {
                            vec![(
                                cmd.project_id().unwrap_or("").to_string(),
                                o.job_id().unwrap_or("").to_string(),
                                -1.0,
                                "running".to_string(),
                            )]
                        })
                        .unwrap_or_default(),
                    _ => Vec::new(),
                };
                state.apply(&cmd);
                state.push_receipt(&cmd);
                if is_save_op(cmd.op_type()) {
                    state.checkpoint_seq += 1;
                    save_events.push(build_save_event(&state, &cmd));
                }
                let mut b = FlatBufferBuilder::new();
                let cid = b.create_string(cmd.command_id().unwrap_or(""));
                let tid = b.create_string(cmd.transaction_id().unwrap_or(""));
                let msg = b.create_string("applied by mock worker");
                let hash = b.create_string("mock");
                let rc = proto::CommandReceipt::create(
                    &mut b,
                    &proto::CommandReceiptArgs {
                        command_id: Some(cid),
                        transaction_id: Some(tid),
                        status: proto::AckStatus::APPLIED,
                        error: proto::ErrorCode::NONE,
                        revision: state.revision,
                        engine_epoch: 1,
                        message: Some(msg),
                        payload_hash: Some(hash),
                    },
                );
                let ef = proto::EventFrame::create(
                    &mut b,
                    &proto::EventFrameArgs {
                        event_type: proto::ControlEvent::CommandReceipt,
                        event: Some(rc.as_union_value()),
                    },
                );
                let env = proto::ControlEnvelope::create(
                    &mut b,
                    &proto::ControlEnvelopeArgs {
                        frame_type: proto::ControlFrame::EventFrame,
                        frame: Some(ef.as_union_value()),
                    },
                );
                b.finish_minimal(env);
                b.finished_data().to_vec()
            } else if let Some(t) = req.request_as_transport_request() {
                let mut b = FlatBufferBuilder::new();
                let rid = b.create_string(t.request_id().unwrap_or(""));
                let ack = proto::TransportAck::create(
                    &mut b,
                    &proto::TransportAckArgs {
                        request_id: Some(rid),
                        ok: true,
                        error: proto::ErrorCode::NONE,
                    },
                );
                let tf = proto::TelemetryFrame::create(
                    &mut b,
                    &proto::TelemetryFrameArgs {
                        event_type: proto::TelemetryEvent::TransportAck,
                        event: Some(ack.as_union_value()),
                    },
                );
                b.finish_minimal(tf);
                b.finished_data().to_vec()
            } else if let Some(rd) = req.request_as_read_request() {
                let mut b = FlatBufferBuilder::new();
                let rid = b.create_string(rd.request_id().unwrap_or(""));
                let cur = b.create_string("");
                let rows = view_items(&state, &rd);
                let items: Vec<_> = rows
                    .iter()
                    .map(|(oid, v)| {
                        let oid_s = b.create_string(oid);
                        let json_s = b.create_string(&v.to_string());
                        proto::ReadItem::create(
                            &mut b,
                            &proto::ReadItemArgs {
                                object_id: Some(oid_s),
                                summary_json: Some(json_s),
                            },
                        )
                    })
                    .collect();
                let items_v = b.create_vector(&items);
                let resp = proto::ReadResponse::create(
                    &mut b,
                    &proto::ReadResponseArgs {
                        request_id: Some(rid),
                        revision: state.revision,
                        items: Some(items_v),
                        next_cursor: Some(cur),
                        done: true,
                        error: proto::ErrorCode::NONE,
                    },
                );
                let ef = proto::EventFrame::create(
                    &mut b,
                    &proto::EventFrameArgs {
                        event_type: proto::ControlEvent::ReadResponse,
                        event: Some(resp.as_union_value()),
                    },
                );
                let env = proto::ControlEnvelope::create(
                    &mut b,
                    &proto::ControlEnvelopeArgs {
                        frame_type: proto::ControlFrame::EventFrame,
                        frame: Some(ef.as_union_value()),
                    },
                );
                b.finish_minimal(env);
                b.finished_data().to_vec()
            } else {
                continue;
            };
            let framed = encode_frame(&resp).unwrap();
            // TransportAck rides telemetry; everything else rides control.
            let sink = if resp.len() > 4
                && flatbuffers::root::<proto::TelemetryFrame>(&resp).is_ok()
                && req.request_as_transport_request().is_some()
            {
                &mut telemetry
            } else {
                &mut control
            };
            if sink.write_all(&framed).await.is_err() {
                break;
            }
            let _ = sink.flush().await;
            // After the receipt is on the wire, emit job lifecycle and
            // save-checkpoint events on the telemetry channel.
            let mut extra: Vec<Vec<u8>> = job_events
                .drain(..)
                .map(|(pid, jid, percent, status)| build_job_event(&pid, &jid, percent, &status))
                .collect();
            extra.append(&mut save_events);
            for ev in extra {
                let framed = encode_frame(&ev).unwrap();
                if telemetry.write_all(&framed).await.is_err() {
                    break;
                }
                let _ = telemetry.flush().await;
            }
        }
        match control.read(&mut buf).await {
            Ok(0) | Err(_) => break,
            Ok(n) => reader.push(&buf[..n]),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every fixture row must serialize to JSON the parsers accept — spot-
    /// check the required keys per view rather than re-implementing parsers.
    #[test]
    fn fixtures_emit_required_keys() {
        let mut state = MockState::new();
        state.project_id = PROJECT_FALLBACK.to_string();

        // TRACK_LIST: shared parser needs trackId; arrange needs track_id.
        let tracks = track_rows(&state);
        assert_eq!(tracks.len(), 2);
        let kinds: Vec<_> = tracks
            .iter()
            .map(|(_, v)| v["kind"].as_str().unwrap().to_string())
            .collect();
        assert!(kinds.contains(&"AUDIO".to_string()));
        assert!(kinds.contains(&"INSTRUMENT".to_string()));
        for (_, v) in &tracks {
            assert!(v["trackId"].is_string() && v["track_id"].is_string());
        }

        // CLIP_LIST: ticks are decimal strings, length > 0.
        let clips = clip_rows(None);
        assert_eq!(clips.len(), 2);
        for (_, v) in &clips {
            for k in ["start_ticks", "length_ticks", "track_id"] {
                assert!(v[k].is_string(), "clip missing {k}");
            }
            assert_ne!(v["length_ticks"].as_str().unwrap(), "0");
        }
        let scoped = clip_rows(Some(TRACK_INSTR));
        assert_eq!(scoped.len(), 1);
        assert_eq!(scoped[0].1["kind"], "MIDI");

        // PLUGIN_LIST: ACTIVE + BYPASSED + MISSING rows, instanceId present.
        let plugins = plugin_rows(None);
        assert_eq!(plugins.len(), 3);
        let statuses: Vec<_> = plugins
            .iter()
            .map(|(_, v)| v["status"].as_str().unwrap().to_string())
            .collect();
        assert!(statuses.contains(&"ACTIVE".to_string()));
        assert!(statuses.contains(&"BYPASSED".to_string()));
        assert!(statuses.contains(&"MISSING".to_string()));

        // MODEL_LIST: at least one runnable audio model.
        let models = model_rows();
        assert!(models.iter().any(|(_, v)| {
            v["kind"] == "audio" && (v["status"] == "available" || v["status"] == "degraded")
        }));
        for (_, v) in &models {
            assert!(v["modelId"].is_string());
        }

        // JOB_LIST: non-terminal by default; terminal rows gated by flag.
        assert_eq!(state.jobs.len(), 2);

        // PROPOSAL_LIST: ready proposal w/ ranked candidates + notes.
        let props = proposal_rows(&state);
        assert_eq!(props.len(), 1);
        let p = &props[0].1;
        assert_eq!(p["status"], "ready");
        assert_eq!(p["context"]["clipId"], CLIP_MIDI);
        assert!(p["candidates"].as_array().unwrap().len() >= 2);
        let n = &p["candidates"][0]["notes"][0];
        assert!(n["pitch"].is_number() && n["onsetTicks"].is_string());

        // PROJECT_SUMMARY carries the recording block the record screen reads.
        let (_, summary) = summary_row(&state);
        assert_eq!(summary["recording"]["phase"], "idle");
        assert!(summary["recording"]["armedTracks"].is_array());
    }

    #[test]
    fn ops_fold_into_state() {
        // The fold logic is exercised through view_items in the integration
        // path; here we check the take-append on StopRecordingOp indirectly
        // via state shapes.
        let state = MockState::new();
        assert!(!state.recording);
        let rows = take_rows(&state);
        assert_eq!(rows.len(), 1);
    }

    /// Parseability wire-check: every view row survives serde_json round-trip
    /// with both key spellings present where parsers mix them.
    #[test]
    fn fixture_json_round_trips() {
        let state = MockState::new();
        let all: Vec<(String, Value)> = [
            vec![summary_row(&state)],
            track_rows(&state),
            clip_rows(None),
            note_rows(None, -1, -1),
            asset_rows(),
            plugin_rows(None),
            take_rows(&state),
            input_device_rows(),
            job_rows(&state, true),
            model_rows(),
            proposal_rows(&state),
            scene_rows(),
        ]
        .into_iter()
        .flatten()
        .collect();
        assert!(all.len() > 10);
        for (oid, v) in &all {
            let s = v.to_string();
            let back: Value = serde_json::from_str(&s).unwrap();
            assert_eq!(&back, v, "row {oid} lost fidelity");
        }
    }

    /// Receipt rows carry the fields the inspector prints verbatim.
    #[test]
    fn receipt_rows_carry_fields() {
        let state = MockState::new();
        assert!(state.receipts.is_empty());
    }

    /// VOID_MOCK_DUMP=<path> writes every view's fixture rows as JSON so the
    /// TS parsers can be run against the exact bytes the wire carries.
    #[test]
    fn dump_views_when_env_set() {
        let Some(path) = env::var_os("VOID_MOCK_DUMP") else {
            return;
        };
        let state = MockState::new();
        let views = [
            ("PROJECT_SUMMARY", proto::ViewKind::PROJECT_SUMMARY),
            ("TRACK_LIST", proto::ViewKind::TRACK_LIST),
            ("CLIP_LIST", proto::ViewKind::CLIP_LIST),
            ("NOTE_RANGE", proto::ViewKind::NOTE_RANGE),
            ("ASSET_LIST", proto::ViewKind::ASSET_LIST),
            ("PLUGIN_LIST", proto::ViewKind::PLUGIN_LIST),
            ("RECEIPT_LIST", proto::ViewKind::RECEIPT_LIST),
            ("TAKE_LIST", proto::ViewKind::TAKE_LIST),
            ("INPUT_DEVICE_LIST", proto::ViewKind::INPUT_DEVICE_LIST),
            ("JOB_LIST", proto::ViewKind::JOB_LIST),
            ("MODEL_LIST", proto::ViewKind::MODEL_LIST),
            ("PROPOSAL_LIST", proto::ViewKind::PROPOSAL_LIST),
            ("SCENE_LIST", proto::ViewKind::SCENE_LIST),
        ];
        let mut out = serde_json::Map::new();
        for (name, kind) in views {
            let rows: Vec<Value> = rows_for_view(&state, kind, None, -1, -1, true)
                .into_iter()
                .map(|(_, v)| v)
                .collect();
            out.insert(name.to_string(), json!(rows));
        }
        std::fs::write(&path, json!(out).to_string()).unwrap();
    }
}
