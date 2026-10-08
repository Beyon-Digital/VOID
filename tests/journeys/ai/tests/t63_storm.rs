//! T63 — AI concurrency regression: job storms across models while projects
//! open/close, worker SIGKILL mid-run, worker crash between stdout lines.
//! Asserts: admission bounds hold, budgets enforced, revisions don't cross
//! projects, stale jobs quarantined, no fake 'done' states surface,
//! retry produces provenance. Real JobRunner + real argv workers.

use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use void_jobs::{
    decide, Admit, CancelToken, JobBudget, JobDb, JobError, JobKind, JobStatus,
    RunContext, RunOutcome, WorkerRuntime,
};
use void_journeys_ai::*;

fn budget(cpu: u64, mem: u64, wall_ns: u64) -> JobBudget {
    JobBudget { cpu_seconds: cpu, memory_bytes: mem, vram_bytes: 0,
        deadline_monotonic_ns: 0, wall_ns }
}

fn ctx<'a>(rt: &'a WorkerRuntime, b: JobBudget) -> RunContext<'a> {
    RunContext { runtime: rt, budget: b, model_version: Some("1.0.0".into()) }
}

fn p(mode: &str) -> serde_json::Value {
    serde_json::json!({"mode": mode})
}

fn sleep_p(ms: u64) -> serde_json::Value {
    serde_json::json!({"mode": "sleep", "durationMs": ms})
}

fn submit(db: &JobDb, env: &ProjectEnv, runtime: &str, params: serde_json::Value, ram: u64) -> String {
    let s = spec(&env.id, JobKind::AudioGeneration, runtime, "fake-model", params, ram, "1");
    s.validate().unwrap();
    let jid = s.job_id.clone();
    db.submit(&s).unwrap();
    jid
}

fn wait_status(db: &JobDb, jid: &str, want: JobStatus, timeout: Duration) {
    let t0 = Instant::now();
    loop {
        let r = db.get(jid).unwrap();
        if r.status == want {
            return;
        }
        assert!(t0.elapsed() < timeout, "job {jid} never reached {want:?} (stuck at {:?})", r.status);
        thread::sleep(Duration::from_millis(25));
    }
}

fn wait_terminal(db: &JobDb, jid: &str, timeout: Duration) -> JobStatus {
    let t0 = Instant::now();
    loop {
        let s = db.get(jid).unwrap().status;
        if !matches!(s, JobStatus::Queued | JobStatus::Running | JobStatus::Cancelling) {
            return s;
        }
        assert!(t0.elapsed() < timeout, "job {jid} never terminated (at {s:?})");
        thread::sleep(Duration::from_millis(25));
    }
}

/// One runner thread on the shared app db. Returns its join handle plus the
/// runner's cooperative cancel token (grabbed before the move).
struct Lane {
    join: thread::JoinHandle<RunOutcome>,
    token: CancelToken,
}

fn spawn_lane(dbp: &PathBuf, env: &ProjectEnv, jid: &str, rt: WorkerRuntime, b: JobBudget) -> Lane {
    let dbp = dbp.clone();
    let store = env.store.clone();
    let root = env.root.path().to_path_buf();
    let jid = jid.to_string();
    let (tok_tx, tok_rx) = mpsc::channel();
    let join = thread::spawn(move || {
        let db = JobDb::open(&dbp).unwrap();
        let runner = void_jobs::JobRunner::new(db, store, root);
        tok_tx.send(runner.cancel_token()).unwrap();
        let spec = runner_spec(&dbp, &jid);
        runner.run(&spec, &ctx(&rt, b)).unwrap_or_else(|e| RunOutcome::Failed {
            error: format!("runner error: {e}"),
            exit_code: None,
            wall_ns: 0,
        })
    });
    Lane { join, token: tok_rx.recv().unwrap() }
}

fn runner_spec(dbp: &PathBuf, jid: &str) -> void_jobs::JobSpec {
    JobDb::open(dbp).unwrap().get(jid).unwrap().spec
}

// ---------------------------------------------------------------------
// T63.1 — admission bounds hold under storm submission
// ---------------------------------------------------------------------
#[test]
fn t63_admission_bounds_hold_under_storm() {
    let app = tempfile::tempdir().unwrap();
    let dbp = app.path().join("index.db");
    let db = app_db(app.path());
    let env = ProjectEnv::new();
    let fake = WorkerRuntime::new(fake_worker());

    // One heavy running (8GiB reservation) -> next heavy must queue.
    let h1 = submit(&db, &env, "void-fake-worker", sleep_p(30000), 8 * 1024 * 1024 * 1024);
    let mut lanes: Vec<(String, Lane)> = Vec::new();
    lanes.push((h1.clone(), spawn_lane(&dbp, &env, &h1,
        WorkerRuntime::new(fake_worker()), budget(4, 8 << 30, 90_000_000_000))));
    wait_status(&db, &h1, JobStatus::Running, Duration::from_secs(15));

    let heavy2 = spec(&env.id, JobKind::AudioGeneration, "void-fake-worker", "fake-model",
        sleep_p(30000), 8 * 1024 * 1024 * 1024, "1");
    assert_eq!(decide(&db, &heavy2).unwrap(), Admit::Queue,
        "heavy #2 must queue while a heavy runs");

    // Two small running -> next small queues.
    for _ in 0..2 {
        let j = submit(&db, &env, "void-fake-worker", sleep_p(30000), 64 << 20);
        let l = spawn_lane(&dbp, &env, &j, WorkerRuntime::new(fake_worker()),
            budget(4, 8 << 30, 90_000_000_000));
        wait_status(&db, &j, JobStatus::Running, Duration::from_secs(15));
        lanes.push((j, l));
    }

    let small3 = spec(&env.id, JobKind::AudioGeneration, "void-fake-worker", "fake-model",
        sleep_p(30000), 64 << 20, "1");
    assert_eq!(decide(&db, &small3).unwrap(), Admit::Queue,
        "small #3 must queue while two small run");

    // Fill the 4-deep wait queue -> Busy for both classes.
    for i in 0..4u8 {
        submit(&db, &env, "void-fake-worker",
            serde_json::json!({"mode":"sleep","durationMs":30000,"_q":i}), 64 << 20);
    }
    match decide(&db, &small3) {
        Err(JobError::Busy(_)) => {}
        other => panic!("expected Busy with full wait queue, got {other:?}"),
    }
    match decide(&db, &heavy2) {
        Err(JobError::Busy(_)) => {}
        other => panic!("expected Busy (heavy) with full wait queue, got {other:?}"),
    }

    // Drain: cancel everything, assert real terminal states (no fake done).
    for rec in db.nonterminal().unwrap() {
        db.cancel(&rec.spec.job_id).unwrap(); // queued->Cancelled, running->Cancelling
    }
    for (jid, lane) in lanes {
        lane.token.cancel();
        let _ = lane.join.join();
        let st = wait_terminal(&db, &jid, Duration::from_secs(10));
        assert!(matches!(st, JobStatus::Cancelled | JobStatus::Failed),
            "drained job {jid} ended {st:?}");
        assert!(!env.job_dir(&jid).join("provenance.json").exists());
    }
    for rec in db.list_by_status(JobStatus::Cancelled).unwrap() {
        assert!(!env.job_dir(&rec.spec.job_id).join("provenance.json").exists());
    }
    let _ = fake;
}

// ---------------------------------------------------------------------
// T63.2 — worker SIGKILL mid-run + retry produces provenance
// ---------------------------------------------------------------------
#[test]
fn t63_worker_sigkill_mid_run_and_retry_provenance() {
    let app = tempfile::tempdir().unwrap();
    let dbp = app.path().join("index.db");
    let db = app_db(app.path());
    let env = ProjectEnv::new();

    let jid = submit(&db, &env, "void-fake-worker", sleep_p(30000), 64 << 20);
    let lane = spawn_lane(&dbp, &env, &jid, WorkerRuntime::new(fake_worker()),
        budget(4, 8 << 30, 90_000_000_000));
    wait_status(&db, &jid, JobStatus::Running, Duration::from_secs(15));

    // Find the real worker child by its unique --staging argv, SIGKILL it.
    let marker = format!("job-{jid}");
    let mut pid = None;
    for _ in 0..300 {
        if let Some(p) = find_pid_with_argv(&marker) {
            pid = Some(p);
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    kill9(pid.expect("worker child never spawned"));

    let out = lane.join.join().unwrap();
    match out {
        RunOutcome::Failed { error, .. } => {
            assert!(error.contains("without a result") || error.contains("exit"),
                "SIGKILL must surface as a failed run, got: {error}");
        }
        other => panic!("SIGKILLed worker reported {other:?} — must never be success"),
    }
    assert_eq!(wait_terminal(&db, &jid, Duration::from_secs(5)), JobStatus::Failed);
    assert!(!env.job_dir(&jid).join("provenance.json").exists(),
        "killed job must not produce provenance (no fake 'done')");

    // Retry: fresh job, same shape — must produce real provenance.
    let jid2 = submit(&db, &env, "void-fake-worker", p("wav"), 64 << 20);
    let lane = spawn_lane(&dbp, &env, &jid2, WorkerRuntime::new(fake_worker()),
        budget(4, 8 << 30, 90_000_000_000));
    let out = lane.join.join().unwrap();
    assert!(matches!(out, RunOutcome::Succeeded { .. }), "retry must succeed: {out:?}");
    assert_eq!(wait_terminal(&db, &jid2, Duration::from_secs(5)), JobStatus::Succeeded);
    assert!(env.job_dir(&jid2).join("provenance.json").exists(),
        "retry must produce provenance");
    assert!(env.job_dir(&jid2).join("result.json").exists());
}

// ---------------------------------------------------------------------
// T63.3 — worker crash between stdout lines (truncated result line)
// ---------------------------------------------------------------------
#[test]
fn t63_worker_crash_between_stdout_lines() {
    let app = tempfile::tempdir().unwrap();
    let dbp = app.path().join("index.db");
    let db = app_db(app.path());
    let env = ProjectEnv::new();
    let exe = fixture_exe("void-fixture-midline-crash");

    let jid = submit(&db, &env, "void-fixture-midline-crash", serde_json::json!({}), 64 << 20);
    let lane = spawn_lane(&dbp, &env, &jid, WorkerRuntime::new(exe),
        budget(4, 8 << 30, 30_000_000_000));
    let out = lane.join.join().unwrap();
    match out {
        RunOutcome::Failed { error, .. } => {
            assert!(error.contains("without a result") || error.contains("exit")
                || error.contains("protocol") || error.contains("line"),
                "mid-line crash must be a failure: {error}");
        }
        other => panic!("mid-line crash reported {other:?} — truncated result must never pass"),
    }
    assert_eq!(wait_terminal(&db, &jid, Duration::from_secs(5)), JobStatus::Failed);
    assert!(!env.job_dir(&jid).join("provenance.json").exists());

    // The fake worker's own crash mode (progress line then exit(3)) — same verdict.
    let j2 = submit(&db, &env, "void-fake-worker", p("crash"), 64 << 20);
    let lane = spawn_lane(&dbp, &env, &j2, WorkerRuntime::new(fake_worker()),
        budget(4, 8 << 30, 30_000_000_000));
    let out = lane.join.join().unwrap();
    assert!(matches!(out, RunOutcome::Failed { .. }), "crash mode must fail: {out:?}");
    assert_eq!(wait_terminal(&db, &j2, Duration::from_secs(5)), JobStatus::Failed);
    assert!(!env.job_dir(&j2).join("provenance.json").exists());
}

// ---------------------------------------------------------------------
// T63.4 — budgets enforced (wall deadline, memory rlimit, cpu rlimit)
// ---------------------------------------------------------------------
#[test]
fn t63_budgets_enforced() {
    let app = tempfile::tempdir().unwrap();
    let dbp = app.path().join("index.db");
    let db = app_db(app.path());
    let env = ProjectEnv::new();

    // (a) wall deadline: 30s sleep under a 0.6s wall budget.
    let j = submit(&db, &env, "void-fake-worker", sleep_p(30000), 64 << 20);
    let lane = spawn_lane(&dbp, &env, &j, WorkerRuntime::new(fake_worker()),
        budget(0, 8 << 30, 600_000_000));
    let out = lane.join.join().unwrap();
    match out {
        RunOutcome::Failed { error, .. } => {
            assert!(error.contains("deadline") || error.contains("budget") || error.contains("exit"),
                "wall-budget kill must be failure: {error}")
        }
        other => panic!("0.6s wall on a 30s job must fail, got {other:?}"),
    }
    assert_eq!(wait_terminal(&db, &j, Duration::from_secs(5)), JobStatus::Failed);
    assert!(!env.job_dir(&j).join("provenance.json").exists());

    // (b) memory cap: oom worker under 512MiB RLIMIT_AS.
    let j = submit(&db, &env, "void-fake-worker", p("oom"), 64 << 20);
    let lane = spawn_lane(&dbp, &env, &j, WorkerRuntime::new(fake_worker()),
        budget(0, 512 << 20, 60_000_000_000));
    let out = lane.join.join().unwrap();
    assert!(matches!(out, RunOutcome::Failed { .. }), "oom under 512MiB must fail: {out:?}");
    assert_eq!(wait_terminal(&db, &j, Duration::from_secs(5)), JobStatus::Failed);
    assert!(!env.job_dir(&j).join("provenance.json").exists());

    // (c) cpu seconds: spin worker under 1s RLIMIT_CPU.
    let j = submit(&db, &env, "void-fake-worker", p("spin"), 64 << 20);
    let lane = spawn_lane(&dbp, &env, &j, WorkerRuntime::new(fake_worker()),
        budget(1, 8 << 30, 60_000_000_000));
    let out = lane.join.join().unwrap();
    assert!(matches!(out, RunOutcome::Failed { .. }), "spin under 1s cpu must fail: {out:?}");
    assert_eq!(wait_terminal(&db, &j, Duration::from_secs(5)), JobStatus::Failed);
    assert!(!env.job_dir(&j).join("provenance.json").exists());
}

// ---------------------------------------------------------------------
// T63.5 — the storm: two projects open concurrently on the app DB,
// mixed outcomes + project close mid-run; provenance stays isolated.
// ---------------------------------------------------------------------
#[test]
fn t63_storm_two_projects_isolation() {
    let app = tempfile::tempdir().unwrap();
    let dbp = app.path().join("index.db");
    let db = app_db(app.path());
    let a = ProjectEnv::new();
    let b = ProjectEnv::new();

    // Project A: succeed wav, succeed midi, stray-sweep, escape-fail,
    // crash-fail, cooperative cancel.
    let ja_wav = submit(&db, &a, "void-fake-worker", p("wav"), 64 << 20);
    let ja_mid = submit(&db, &a, "void-fake-worker", p("midi"), 64 << 20);
    let ja_stray = submit(&db, &a, "void-fake-worker", p("stray"), 64 << 20);
    let ja_escape = submit(&db, &a, "void-fake-worker", p("escape"), 64 << 20);
    let ja_crash = submit(&db, &a, "void-fake-worker", p("crash"), 64 << 20);
    let ja_cancel = submit(&db, &a, "void-fake-worker", sleep_p(30000), 64 << 20);
    // Project B: wav + midi succeed, one sleeper SIGKILLed ("project closes").
    let jb_wav = submit(&db, &b, "void-fake-worker", p("wav"), 64 << 20);
    let jb_mid = submit(&db, &b, "void-fake-worker", p("midi"), 64 << 20);
    let jb_sleep = submit(&db, &b, "void-fake-worker", sleep_p(30000), 64 << 20);

    let lanes: Vec<(String, Lane)> = [
        (&a, &ja_wav), (&a, &ja_mid), (&a, &ja_stray), (&a, &ja_escape),
        (&a, &ja_crash), (&a, &ja_cancel), (&b, &jb_wav), (&b, &jb_mid), (&b, &jb_sleep),
    ]
    .iter()
    .map(|(env, jid)| {
        (jid.to_string(), spawn_lane(&dbp, env, jid,
            WorkerRuntime::new(fake_worker()), budget(4, 8 << 30, 90_000_000_000)))
    })
    .collect();

    // Mid-storm: cooperative cancel on A's sleeper, SIGKILL on B's.
    wait_status(&db, &ja_cancel, JobStatus::Running, Duration::from_secs(15));
    wait_status(&db, &jb_sleep, JobStatus::Running, Duration::from_secs(15));
    let tok_cancel = lanes.iter().find(|(j, _)| j == &ja_cancel).unwrap().1.token.clone();
    db.cancel(&ja_cancel).unwrap();
    tok_cancel.cancel();
    let mut pid = None;
    for _ in 0..300 {
        if let Some(p) = find_pid_with_argv(&format!("job-{jb_sleep}")) {
            pid = Some(p);
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    kill9(pid.expect("B sleeper child never spawned"));

    let mut outcomes = Vec::new();
    for (jid, lane) in lanes {
        outcomes.push((jid, lane.join.join().unwrap()));
    }

    // Terminal truth per job — each event matches db, no fake 'done'.
    let expect = [
        (ja_wav.clone(), JobStatus::Succeeded),
        (ja_mid.clone(), JobStatus::Succeeded),
        (ja_stray.clone(), JobStatus::Succeeded),
        (ja_escape.clone(), JobStatus::Failed),
        (ja_crash.clone(), JobStatus::Failed),
        (ja_cancel.clone(), JobStatus::Cancelled),
        (jb_wav.clone(), JobStatus::Succeeded),
        (jb_mid.clone(), JobStatus::Succeeded),
        (jb_sleep.clone(), JobStatus::Failed),
    ];
    for (jid, want) in &expect {
        let rec = db.get(jid).unwrap();
        assert_eq!(&rec.status, want, "job {jid} final status");
        let (_, out) = outcomes.iter().find(|(j, _)| j == jid).unwrap();
        let ev = match out {
            RunOutcome::Succeeded { .. } => JobStatus::Succeeded,
            RunOutcome::Failed { .. } => JobStatus::Failed,
            RunOutcome::Cancelled { .. } => JobStatus::Cancelled,
        };
        assert_eq!(&ev, want, "job {jid} outcome/db mismatch — the UI-visible state must be real");
    }

    // Provenance isolation: succeeded results land ONLY in the owning container.
    for jid in [&ja_wav, &ja_mid, &ja_stray] {
        assert!(a.job_dir(jid).join("provenance.json").exists(), "A provenance {jid}");
        assert!(!b.job_dir(jid).join("provenance.json").exists(), "leak A->B {jid}");
    }
    for jid in [&jb_wav, &jb_mid] {
        assert!(b.job_dir(jid).join("provenance.json").exists(), "B provenance {jid}");
        assert!(!a.job_dir(jid).join("provenance.json").exists(), "leak B->A {jid}");
    }
    for jid in [&ja_escape, &ja_crash, &ja_cancel, &jb_sleep] {
        assert!(!a.job_dir(jid).join("provenance.json").exists());
        assert!(!b.job_dir(jid).join("provenance.json").exists());
    }

    // Revision scoping: list_for_project partitions by container.
    let la = db.list_for_project(&a.id).unwrap();
    let lb = db.list_for_project(&b.id).unwrap();
    assert_eq!(la.len(), 6);
    assert_eq!(lb.len(), 3);
    assert!(la.iter().all(|r| r.spec.project_id == a.id));
    assert!(lb.iter().all(|r| r.spec.project_id == b.id));

    // Stray worker: succeeded, stray files swept + reported in provenance.
    let (_, stray_out) = outcomes.iter().find(|(j, _)| j == &ja_stray).unwrap();
    match stray_out {
        RunOutcome::Succeeded { stray_files, .. } => {
            assert!(*stray_files >= 1, "stray file must be swept + reported in RunOutcome")
        }
        other => panic!("stray-mode job must succeed, got {other:?}"),
    }
    let prov: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(a.job_dir(&ja_stray).join("provenance.json")).unwrap()).unwrap();
    let stray = prov["measured"]["strayFiles"].as_u64()
        .or_else(|| prov["measured"]["stray_files"].as_u64())
        .unwrap_or(0);
    assert!(stray >= 1, "provenance must record strays: {prov}");
    // strays are excluded from the committed artifact list — never auto-committed
    let res: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(a.job_dir(&ja_stray).join("result.json")).unwrap()).unwrap();
    let committed: Vec<String> = res["artifacts"].as_array().unwrap().iter()
        .map(|a| a["name"].as_str().unwrap().to_string()).collect();
    assert!(committed.iter().all(|n| n == "out.wav"), "stray leaked into result: {committed:?}");
    // runner semantics: staging torn down for every non-Succeeded record.
    for (jid, want) in &expect {
        for e in [&a, &b] {
            if *want != JobStatus::Succeeded {
                assert!(!e.staging_dir(jid).exists(), "staging residue for {jid}");
            }
        }
    }
}

// ---------------------------------------------------------------------
// T63.6 — late result on a cancelled job is quarantined (never 'done')
// ---------------------------------------------------------------------
#[test]
fn t63_late_result_on_cancelled_job_is_quarantined() {
    let app = tempfile::tempdir().unwrap();
    let db = app_db(app.path());
    let env = ProjectEnv::new();
    let jid = submit(&db, &env, "void-fake-worker", p("wav"), 64 << 20);

    db.cancel(&jid).unwrap();
    assert_eq!(db.get(&jid).unwrap().status, JobStatus::Cancelled);

    // A stale worker's result lands afterwards: stays Cancelled + quarantined.
    let late = serde_json::json!({"status": "succeeded", "artifacts": [], "warnings": []});
    let st = db.record_result(&jid, &late).unwrap();
    assert_eq!(st, JobStatus::Cancelled, "late result must not flip status");
    assert!(db.get(&jid).unwrap().quarantined, "late result must quarantine the row");
    assert!(!env.job_dir(&jid).join("provenance.json").exists());
}
