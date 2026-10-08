//! Latency measurement + the musical-clock separation check.
//!
//! (a) LATS lines: submit→result latency + worker-reported inferenceMs
//!     per enabled model on THIS CPU box.
//! (b) Musical-clock separation — proven structurally: the engine
//!     callback path cannot reach void-jobs because no crate in the
//!     engine lane depends on it. This test enforces that boundary so a
//!     future regression fails the gate, and documents the check.

use std::path::Path;
use std::time::Instant;
use void_jobs::{JobBudget, JobDb, JobKind, JobStatus, RunContext, RunOutcome, WorkerRuntime};
use void_journeys_ai::*;

fn ctx<'a>(rt: &'a WorkerRuntime, b: JobBudget) -> RunContext<'a> {
    RunContext { runtime: rt, budget: b, model_version: Some("1.0.0".into()) }
}

fn b(mem: u64, wall_s: u64) -> JobBudget {
    JobBudget { cpu_seconds: 0, memory_bytes: mem, vram_bytes: 0,
        deadline_monotonic_ns: 0, wall_ns: wall_s * 1_000_000_000 }
}

fn lats(
    env: &ProjectEnv,
    dbp: &Path,
    exe: &Path,
    kind: JobKind,
    runtime: &str,
    model: &str,
    inputs: Vec<String>,
    params: serde_json::Value,
    ram: u64,
    budget: JobBudget,
) {
    let db = JobDb::open(dbp).unwrap();
    let mut s = spec(&env.id, kind, runtime, model, params, ram, "1");
    s.inputs = inputs;
    s.runtime_sha256 = sha256_file(exe);
    s.validate().unwrap();
    db.submit(&s).unwrap();
    let jid = s.job_id.clone();

    let rt = WorkerRuntime::new(exe);
    let runner = env.runner(&dbp.to_path_buf());
    let t_submit = Instant::now();
    let out = runner.run(&s, &ctx(&rt, budget)).unwrap();
    let submit_to_result_ms = t_submit.elapsed().as_millis() as u64;
    match out {
        RunOutcome::Succeeded { artifacts, wall_ns, .. } => {
            assert_eq!(db.get(&jid).unwrap().status, JobStatus::Succeeded);
            let inf_ms = artifacts
                .iter()
                .find(|a| a.name == "metrics.json")
                .and_then(|m| {
                    serde_json::from_slice::<serde_json::Value>(&std::fs::read(&m.asset_abs).unwrap())
                        .ok()?["inferenceMs"]
                        .as_u64()
                })
                .unwrap_or(0);
            println!(
                "LATS {}",
                serde_json::json!({
                    "model": model, "runtime": runtime,
                    "submit_to_result_ms": submit_to_result_ms,
                    "run_wall_ms": wall_ns / 1_000_000,
                    "inference_ms": inf_ms,
                })
            );
        }
        other => panic!("latency probe failed for {model}: {other:?}"),
    }
}

#[test]
fn f2_latency_per_model_cpu_box() {
    let app = tempfile::tempdir().unwrap();
    let dbp = app.path().join("index.db");
    app_db(app.path());
    let env = ProjectEnv::new();

    // symbolic (algorithmic)
    let sym = symbolic_worker();
    let c = void_proposals::RegionContext {
        track_id: uuid::Uuid::new_v4().to_string(),
        clip_id: uuid::Uuid::new_v4().to_string(),
        region: void_proposals::TickRange { start_ticks: "0".into(), length_ticks: "1920".into() },
        continuation_start_ticks: "1920".into(),
        continuation_ticks: "960".into(),
        tempo_bpm: 120.0, ts_num: 4, ts_den: 4, key_hint: None,
        notes: vec![void_proposals::NoteEvent {
            pitch: 60, velocity: 90, onset_ticks: "0".into(), length_ticks: "240".into() }],
        locked_ranges: vec![], labels: vec![],
    };
    let p = c.to_worker_params(c.default_seed(), 4);
    lats(&env, &dbp, &sym, JobKind::Symbolic, "void-symbolic-worker", "interval-markov-1",
        vec![c.sha256()], p, 256 << 20, b(1 << 30, 60));

    // procedural generation (algorithmic)
    let proc = audio_worker("generate-procedural", "void-audio-generate-procedural");
    lats(&env, &dbp, &proc, JobKind::AudioGeneration, "void-audio-generate-procedural",
        "void.audio.generate-procedural", vec![],
        serde_json::json!({"seed": 3, "secondsPerChord": 0.5, "sampleRate": 48000}),
        512 << 20, b(2 << 30, 120));

    // real model workers
    let wav = env.store.import_bytes(&synth_melody_wav(), "wav").unwrap();
    let tr = audio_worker("transcribe", "void-audio-transcribe");
    lats(&env, &dbp, &tr, JobKind::Transcription, "void-audio-transcribe",
        "void.audio.transcribe-basic-pitch", vec![wav.sha256.clone()],
        serde_json::json!({"input": wav.sha256}), 4 << 30, b(6 << 30, 300));

    let sep = audio_worker("separate", "void-audio-separate");
    lats(&env, &dbp, &sep, JobKind::Separation, "void-audio-separate",
        "void.audio.separate-htdemucs-6s", vec![wav.sha256.clone()],
        serde_json::json!({"input": wav.sha256, "model": "htdemucs_6s"}), 6 << 30, b(8 << 30, 600));

    let gen = audio_worker("generate", "void-audio-generate");
    lats(&env, &dbp, &gen, JobKind::AudioGeneration, "void-audio-generate",
        "void.audio.generate-musicgen-small", vec![],
        serde_json::json!({"prompt": "short plucky motif", "durationS": 2, "seed": 5}),
        9 << 30, b(9 << 30, 600));
}

/// The engine callback path must never reach the job runner. Enforced
/// structurally: no engine-lane crate may depend on void-jobs, and no
/// engine source file may reference it. Scans REAL manifests + sources —
/// a regression that wires jobs into the audio path fails this test.
#[test]
fn f2_jobs_never_on_musical_clock() {
    let root = repo_root();
    // 1) Dependency boundary: only these crates may name void-jobs.
    let allowed_dependents = ["void-export", "void-proposals", "void-journeys-ai", "void-models-tests"];
    let mut dependents = Vec::new();
    for dir in ["crates", "apps", "tests/models", "tests/journeys/ai", "workers"] {
        let d = root.join(dir);
        if !d.is_dir() {
            continue;
        }
        for e in walk(&d) {
            if e.file_name().unwrap() == "Cargo.toml" {
                let text = std::fs::read_to_string(&e).unwrap();
                // A dependency edge: `void-jobs = ...` / `void-jobs.workspace`
                // / `void_jobs` in extern position. (The package's own
                // `name = "void-jobs"` line is not an edge.)
                let edge = text.lines().any(|l| {
                    let t = l.trim();
                    t.starts_with("void-jobs ") || t.starts_with("void-jobs=")
                        || t.starts_with("void-jobs.") || t.starts_with("void_jobs ")
                        || t.starts_with("void_jobs=")
                });
                if edge {
                    dependents.push(cargo_pkg_name(&e));
                }
            }
        }
    }
    for dep in &dependents {
        assert!(
            allowed_dependents.iter().any(|a| dep == a),
            "crate '{dep}' gained a void-jobs dependency — engine-lane coupling regression"
        );
    }

    // 2) Source boundary: the engine lane (native/ = the real-time
    //    callback world, crates/void-worker, crates/void-protocol,
    //    apps/) must not reference the runner at all.
    for dir in ["native", "apps", "crates/void-worker", "crates/void-protocol"] {
        let d = root.join(dir);
        if !d.is_dir() {
            continue;
        }
        for e in walk(&d) {
            let p = e.to_string_lossy().to_string();
            if p.contains("/target/") || !(p.ends_with(".rs") || p.ends_with(".cpp")
                || p.ends_with(".cc") || p.ends_with(".h") || p.ends_with(".ts") || p.ends_with(".tsx"))
            {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&e) {
                assert!(
                    !text.contains("JobRunner") && !text.contains("void_jobs") && !text.contains("void-jobs"),
                    "engine-lane file {p} references the job runner — musical-clock regression"
                );
            }
        }
    }

    // 3) The runner's own clock is CLOCK_MONOTONIC (not a musical
    //    transport clock) — budget deadlines derive from spawn time only.
    let t0 = void_jobs::monotonic_ns();
    std::thread::sleep(std::time::Duration::from_millis(20));
    let dt = void_jobs::monotonic_ns() - t0;
    assert!(dt >= 15_000_000 && dt < 500_000_000,
        "job deadlines run on CLOCK_MONOTONIC wall time (dt={dt}ns)");

    // 4) JobRunner's public surface carries no audio callback: it is
    //    spawn/join/poll over argv processes, with cooperative cancel.
    //    (compile-level fact — documented in docs/verification/F2/F2_GATE.md)
    println!("F2 musical-clock: dependents={dependents:?}; engine lane clean; CLOCK_MONOTONIC verified");
}

fn walk(d: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(d) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if p.file_name().unwrap() == "target" || p.file_name().unwrap() == "node_modules" {
                    continue;
                }
                out.extend(walk(&p));
            } else {
                out.push(p);
            }
        }
    }
    out
}

fn cargo_pkg_name(manifest: &Path) -> String {
    for line in std::fs::read_to_string(manifest).unwrap().lines() {
        let t = line.trim();
        if t.starts_with("name") && t.contains('=') {
            return t.split('=').nth(1).unwrap().trim().trim_matches('"').to_string();
        }
    }
    String::new()
}
