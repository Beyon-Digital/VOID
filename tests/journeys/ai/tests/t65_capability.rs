//! T65 — F2 capability evidence. Part 1 proves the offline posture
//! (run inside `tests/journeys/ai/offline_gate.sh`, which wraps this
//! binary in `unshare -Urn env -i`); part 2 exercises each qualified
//! capability one at a time through the REAL JobRunner + worker binary
//! and emits a `CAPROW {...}` evidence line per capability that
//! `docs/verification/F2/CAPABILITY_MATRIX.md` is generated from.

use std::path::PathBuf;
use void_jobs::{JobBudget, JobDb, JobKind, JobStatus, RunContext, RunOutcome, WorkerRuntime};
use void_journeys_ai::*;
use void_proposals::RegionContext;

fn ctx<'a>(rt: &'a WorkerRuntime, b: JobBudget) -> RunContext<'a> {
    RunContext { runtime: rt, budget: b, model_version: Some("1.0.0".into()) }
}

// WorkerRuntime has no public exe accessor — keep it alongside.
struct Rt {
    rt: WorkerRuntime,
    path: PathBuf,
}
impl Rt {
    fn new(p: PathBuf) -> Self {
        Self { rt: WorkerRuntime::new(p.clone()), path: p }
    }
}

fn cap_row(
    env: &ProjectEnv,
    dbp: &PathBuf,
    r: &Rt,
    kind: JobKind,
    runtime: &str,
    model: &str,
    inputs: Vec<String>,
    params: serde_json::Value,
    ram: u64,
    b: JobBudget,
    license: &str,
    gap: &str,
) -> Vec<void_jobs::ArtifactRecord> {
    let db = JobDb::open(dbp).unwrap();
    let mut s = spec(&env.id, kind, runtime, model, params, ram, "1");
    s.inputs = inputs;
    s.runtime_sha256 = sha256_file(&r.path);
    s.validate().unwrap();
    db.submit(&s).unwrap();
    let jid = s.job_id.clone();
    let db2 = JobDb::open(dbp).unwrap();
    let runner = env.runner(dbp);
    let t0 = std::time::Instant::now();
    let out = runner.run(&s, &ctx(&r.rt, b)).unwrap();
    let submit_to_result_ms = t0.elapsed().as_millis() as u64;
    match out {
        RunOutcome::Succeeded { artifacts, wall_ns, .. } => {
            assert_eq!(db2.get(&jid).unwrap().status, JobStatus::Succeeded);
            let prov = env.job_dir(&jid).join("provenance.json");
            assert!(prov.exists(), "no provenance for {jid}");
            let metrics = artifacts.iter().find(|a| a.name == "metrics.json");
            let (inf_ms, rss) = metrics
                .map(|m| {
                    let v: serde_json::Value =
                        serde_json::from_slice(&std::fs::read(&m.asset_abs).unwrap()).unwrap();
                    (v["inferenceMs"].as_u64().unwrap_or(0),
                     v["peakRssBytes"].as_u64().unwrap_or(0))
                })
                .unwrap_or((0, 0));
            let primary = artifacts
                .iter()
                .find(|a| a.name.ends_with(".wav") || a.name.ends_with(".mid") || a.name == "proposals.json")
                .expect("no primary artifact");
            println!(
                "CAPROW {}",
                serde_json::json!({
                    "capability": model,
                    "runtime": runtime,
                    "job_id": jid,
                    "artifact": primary.name,
                    "artifact_sha256": primary.sha256,
                    "artifact_bytes": std::fs::metadata(&primary.asset_abs).unwrap().len(),
                    "wall_ms": wall_ns / 1_000_000,
                    "inference_ms": inf_ms,
                    "peak_rss_bytes": rss,
                    "submit_to_result_ms": submit_to_result_ms,
                    "license": license,
                    "gap": gap,
                })
            );
            artifacts
        }
        other => panic!("capability {runtime} failed: {other:?}"),
    }
}

fn env_setup() -> (tempfile::TempDir, PathBuf, ProjectEnv) {
    let app = tempfile::tempdir().unwrap();
    let dbp = app.path().join("index.db");
    app_db(app.path());
    let env = ProjectEnv::new();
    (app, dbp, env)
}

// ---------------------------------------------------------------------
// T65.1 — offline posture (this whole binary also runs inside
// `unshare -Urn env -i` via offline_gate.sh; the env flag proves we
// are inside the denied namespace when set).
// ---------------------------------------------------------------------
#[test]
fn t65_offline_posture_socket_deny() {
    use std::net::TcpStream;
    use std::time::Duration;
    let gated = std::env::var("VOID_OFFLINE_GATE").is_ok();
    let lo = TcpStream::connect_timeout(&"127.0.0.1:9".parse().unwrap(), Duration::from_millis(500));
    let wan = TcpStream::connect_timeout(&"1.1.1.1:443".parse().unwrap(), Duration::from_millis(1500));
    let mut udp_ok = false;
    if let Ok(u) = std::net::UdpSocket::bind("0.0.0.0:0") {
        udp_ok = u.connect("192.0.2.1:53").is_ok() && u.send(&[0u8; 8]).is_ok();
    }
    if gated {
        assert!(lo.is_err(), "inside the gate even loopback must be unreachable");
        assert!(wan.is_err(), "inside the gate WAN must be unreachable");
        assert!(!udp_ok, "inside the gate UDP must be denied");
        println!("T65 offline: all sockets denied (loopback + WAN + UDP)");
    } else {
        println!(
            "T65 offline probe (ungated control run): loopback_err={} wan_err={} udp_denied={}",
            lo.is_err(), wan.is_err(), !udp_ok
        );
    }
}

// ---------------------------------------------------------------------
// T65.2 — manual ops: real save path (dry-run on a temp project) works
// with no network anywhere in the call graph.
// ---------------------------------------------------------------------
#[test]
fn t65_manual_save_path_dry_run() {
    let root = tempfile::tempdir().unwrap();
    // Real container init — `void project create` equivalent.
    let pid = uuid::Uuid::new_v4().to_string();
    void_project::create(root.path(), &void_project::ProjectMeta::new(pid.clone(), "f2-dry-run".into()))
        .expect("project container create");
    let bundle = void_project::SnapshotBundle {
        engine_snapshot: b"engine-state-v1".to_vec(),
        app_state: serde_json::to_vec(&serde_json::json!({
            "clips": [{"id": uuid::Uuid::new_v4().to_string(), "name": "take-1"}],
            "patterns": [{"id": "p1", "steps": 16}],
        }))
        .unwrap(),
        command_receipts: vec![],
        asset_hashes: vec![],
    };
    let rec = void_project::save(
        root.path(),
        void_project::SaveRequest {
            project_id: pid.clone(),
            revision: 1,
            engine_revision: "0".into(),
            parent_checkpoint_id: None,
        },
        &bundle,
    )
    .expect("save path failed offline");
    assert_eq!(rec.revision, 1);
    assert_eq!(rec.manifest_sha256.len(), 64);
    let verified = void_project::verify_checkpoint(root.path(), &rec.checkpoint_id).unwrap();
    let cur = void_project::read_current(root.path()).unwrap().expect("no current ptr");
    assert_eq!(cur.checkpoint_id, verified.manifest.checkpoint_id);
    println!("T65 save dry-run: checkpoint {} verified, current ptr set", rec.checkpoint_id);
}

// ---------------------------------------------------------------------
// T65.3 — capability: symbolic proposals (deterministic, always local)
// ---------------------------------------------------------------------
#[test]
fn t65_capability_symbolic() {
    let (_app, dbp, env) = env_setup();
    let r = Rt::new(symbolic_worker());
    let ctx = RegionContext {
        track_id: uuid::Uuid::new_v4().to_string(),
        clip_id: uuid::Uuid::new_v4().to_string(),
        region: void_proposals::TickRange { start_ticks: "0".into(), length_ticks: "1920".into() },
        continuation_start_ticks: "1920".into(),
        continuation_ticks: "1920".into(),
        tempo_bpm: 118.0,
        ts_num: 4,
        ts_den: 4,
        key_hint: Some("A minor".into()),
        notes: vec![void_proposals::NoteEvent {
            pitch: 57, velocity: 90, onset_ticks: "0".into(), length_ticks: "480".into() }],
        locked_ranges: vec![],
        labels: vec![],
    };
    let params = ctx.to_worker_params(ctx.default_seed(), 4);
    let arts = cap_row(&env, &dbp, &r, JobKind::Symbolic,
        "void-symbolic-worker", "interval-markov-1",
        vec![ctx.sha256()], params, 256 << 20,
        JobBudget { cpu_seconds: 0, memory_bytes: 2 << 30, vram_bytes: 0,
            deadline_monotonic_ns: 0, wall_ns: 60_000_000_000 },
        "MIT (in-repo)", "none — algorithmic");
    let doc = arts.iter().find(|a| a.name == "proposals.json").expect("proposals.json");
    let parsed = void_proposals::parse_document(&std::fs::read(&doc.asset_abs).unwrap())
        .expect("proposals.json failed the real document validator");
    assert!(!parsed.candidates.is_empty());
}

// ---------------------------------------------------------------------
// T65.4 — capability: procedural generation (deterministic bytes)
// ---------------------------------------------------------------------
#[test]
fn t65_capability_generate_procedural() {
    let (_app, dbp, env) = env_setup();
    let r = Rt::new(audio_worker("generate-procedural", "void-audio-generate-procedural"));
    let arts = cap_row(&env, &dbp, &r, JobKind::AudioGeneration,
        "void-audio-generate-procedural", "void.audio.generate-procedural",
        vec![], serde_json::json!({"seed": 11, "secondsPerChord": 0.5, "sampleRate": 48000}),
        512 << 20,
        JobBudget { cpu_seconds: 0, memory_bytes: 2 << 30, vram_bytes: 0,
            deadline_monotonic_ns: 0, wall_ns: 120_000_000_000 },
        "MIT (in-repo)", "none — algorithmic, no weights");
    let wav = arts.iter().find(|a| a.name == "generated.wav").expect("generated.wav");
    let data = std::fs::read(&wav.asset_abs).unwrap();
    assert_eq!(&data[..4], b"RIFF");
    assert!(wav_rms(&data) > 0.005, "procedural output is silence");
}

// ---------------------------------------------------------------------
// T65.5 — capability: transcription (basic-pitch, real inference)
// ---------------------------------------------------------------------
#[test]
fn t65_capability_transcribe() {
    let (_app, dbp, env) = env_setup();
    let r = Rt::new(audio_worker("transcribe", "void-audio-transcribe"));
    let input = env.store.import_bytes(&synth_melody_wav(), "wav").unwrap();
    let arts = cap_row(&env, &dbp, &r, JobKind::Transcription,
        "void-audio-transcribe", "void.audio.transcribe-basic-pitch",
        vec![input.sha256.clone()], serde_json::json!({"input": input.sha256}),
        4 << 30,
        JobBudget { cpu_seconds: 0, memory_bytes: 6 << 30, vram_bytes: 0,
            deadline_monotonic_ns: 0, wall_ns: 300_000_000_000 },
        "MIT (basic-pitch 0.4.0)", "none");
    let mid = arts.iter().find(|a| a.name == "transcription.mid").expect("transcription.mid");
    let onsets = midi_note_ons(&std::fs::read(&mid.asset_abs).unwrap());
    assert!(onsets.len() >= 4, "transcription produced {} notes", onsets.len());
}

// ---------------------------------------------------------------------
// T65.6 — capability: stem separation (htdemucs_6s, real inference)
// ---------------------------------------------------------------------
#[test]
fn t65_capability_separate() {
    let (_app, dbp, env) = env_setup();
    let r = Rt::new(audio_worker("separate", "void-audio-separate"));
    let input = env.store.import_bytes(&synth_melody_wav(), "wav").unwrap();
    let arts = cap_row(&env, &dbp, &r, JobKind::Separation,
        "void-audio-separate", "void.audio.separate-htdemucs-6s",
        vec![input.sha256.clone()],
        serde_json::json!({"input": input.sha256, "model": "htdemucs_6s"}),
        6 << 30,
        JobBudget { cpu_seconds: 0, memory_bytes: 8 << 30, vram_bytes: 0,
            deadline_monotonic_ns: 0, wall_ns: 600_000_000_000 },
        "MIT (demucs weights)", "none");
    let stems: Vec<_> = arts.iter().filter(|a| a.name.starts_with("stems/")).collect();
    assert_eq!(stems.len(), 6, "htdemucs_6s must emit 6 stems");
}

// ---------------------------------------------------------------------
// T65.7 — capability: musicgen-small (CC-BY-NC flagged, scaled fixture)
// ---------------------------------------------------------------------
#[test]
fn t65_capability_generate_musicgen() {
    let (_app, dbp, env) = env_setup();
    let r = Rt::new(audio_worker("generate", "void-audio-generate"));
    // durationS=2 — honestly scaled down; W15 evidence is 4s.
    let arts = cap_row(&env, &dbp, &r, JobKind::AudioGeneration,
        "void-audio-generate", "void.audio.generate-musicgen-small",
        vec![], serde_json::json!({"prompt": "short plucky motif", "durationS": 2, "seed": 5}),
        9 << 30,
        JobBudget { cpu_seconds: 0, memory_bytes: 9 << 30, vram_bytes: 0,
            deadline_monotonic_ns: 0, wall_ns: 600_000_000_000 },
        "CC-BY-NC-4.0 (FLAGGED — non-commercial)", "license blocks commercial use; preview/audition layer absent (NEEDS §12)");
    let wav = arts.iter().find(|a| a.name == "generated.wav").expect("generated.wav");
    let data = std::fs::read(&wav.asset_abs).unwrap();
    assert_eq!(&data[..4], b"RIFF");
    assert_eq!(wav_sample_rate(&data), 32_000);
    assert!(wav_rms(&data) > 0.005, "musicgen output is silence");
}
