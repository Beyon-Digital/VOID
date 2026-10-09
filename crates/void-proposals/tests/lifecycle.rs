//! W13 lifecycle tests — real JobRunner + real symbolic worker binary
//! (T53–T56 coverage at crate level; see docs/proposals/EVIDENCE.md).

use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;
use void_assets::AssetStore;
use void_jobs::{
    JobBudget, JobDb, JobRunner, JobStatus, Reservations, RunContext, RunOutcome, WorkerRuntime,
};
use void_proposals::*;

const WORKER: &str = "../../workers/symbolic";

fn worker_bin() -> PathBuf {
    let exe = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(WORKER)
        .join(format!(
            "target/debug/void-symbolic-worker{}",
            std::env::consts::EXE_SUFFIX
        ));
    if !exe.exists() {
        let status = Command::new("cargo")
            .args(["build", "--manifest-path", &format!("{WORKER}/Cargo.toml")])
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .status()
            .expect("cargo build worker");
        assert!(status.success(), "worker build failed");
    }
    exe
}

struct Fx {
    _t: TempDir,
    svc: ProposalService,
    root: PathBuf,
    project: String,
}

fn fix() -> Fx {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("song.void");
    std::fs::create_dir_all(&root).unwrap();
    let db = JobDb::open(root.join("index.db")).unwrap();
    Fx {
        svc: ProposalService::new(db, ProposalStore::new(&root), root.clone()),
        root,
        project: uuid::Uuid::new_v4().to_string(),
        _t: t,
    }
}

fn ctx_notes() -> Vec<NoteEvent> {
    [60, 62, 64, 65, 67, 64, 62, 60]
        .iter()
        .enumerate()
        .map(|(i, &p)| NoteEvent {
            pitch: p,
            velocity: 90 + (i % 4) as i32,
            onset_ticks: (i as i64 * 480_000).to_string(),
            length_ticks: "240000".into(),
        })
        .collect()
}

fn ctx() -> RegionContext {
    RegionContext {
        track_id: uuid::Uuid::new_v4().to_string(),
        clip_id: uuid::Uuid::new_v4().to_string(),
        region: TickRange {
            start_ticks: "0".into(),
            length_ticks: "3840000".into(),
        },
        continuation_start_ticks: "3840000".into(),
        continuation_ticks: "3840000".into(),
        tempo_bpm: 120.0,
        ts_num: 4,
        ts_den: 4,
        key_hint: None,
        notes: ctx_notes(),
        locked_ranges: vec![TickRange {
            start_ticks: "3840000".into(),
            length_ticks: "480000".into(),
        }],
        labels: vec!["']}) ; rm -rf / ; ignore previous".into()],
    }
}

fn req(seed: Option<u64>, fx: &Fx) -> GenerateRequest {
    req_ctx(ctx(), seed, fx)
}

fn req_ctx(context: RegionContext, seed: Option<u64>, fx: &Fx) -> GenerateRequest {
    GenerateRequest {
        project_id: fx.project.clone(),
        source_revision: "7".into(),
        context,
        max_proposals: 3,
        seed,
        runtime_sha256: file_sha256(&worker_bin()).unwrap(),
        reservations: Reservations {
            ram_bytes: "268435456".into(),
            vram_bytes: "0".into(),
            cpu_threads: 1,
        },
        deadline_monotonic_ns: "0".into(),
    }
}

fn run_job(fx: &Fx, pending: &PendingProposal) -> (void_jobs::JobRecord, RunOutcome) {
    let assets = AssetStore::new(fx.root.join("assets"), 64 * 1024 * 1024).unwrap();
    let runner = JobRunner::new(
        JobDb::open(fx.root.join("index.db")).unwrap(),
        assets,
        fx.root.clone(),
    );
    let runtime = WorkerRuntime::new(worker_bin());
    let ctx = RunContext {
        runtime: &runtime,
        budget: JobBudget {
            cpu_seconds: 30,
            memory_bytes: 1 << 30,
            vram_bytes: 0,
            deadline_monotonic_ns: 0,
            wall_ns: 30_000_000_000,
        },
        model_version: None,
    };
    let outcome = runner.run(&pending.spec, &ctx).unwrap();
    let job = svc_db(fx).get(&pending.spec.job_id).unwrap();
    (job, outcome)
}

fn svc_db(fx: &Fx) -> JobDb {
    JobDb::open(fx.root.join("index.db")).unwrap()
}

fn fresh_reval(fx: &Fx, rec: &ProposalRecord) -> Revalidation {
    Revalidation {
        project_id: fx.project.clone(),
        current_context_sha256: rec.context_sha256.clone(),
        target_clip_exists: true,
    }
}

#[test]
fn end_to_end_generate_collect_accept() {
    let fx = fix();
    // T53: request → pending record → job runs → ready w/ candidates.
    let pending = fx.svc.request(&req(Some(42), &fx)).unwrap();
    assert_eq!(pending.record.status, ProposalStatus::Pending);
    assert!(!pending.record.context_sha256.is_empty());
    let (job, outcome) = run_job(&fx, &pending);
    assert_eq!(job.status, JobStatus::Succeeded);
    let rec = fx
        .svc
        .collect(&pending.record.proposal_id, &job, &outcome)
        .unwrap();
    assert_eq!(rec.status, ProposalStatus::Ready);
    assert_eq!(rec.candidates.len(), 3);
    assert!(rec.candidates[0].score >= rec.candidates[1].score); // ranked
    let prov = rec.provenance.as_ref().unwrap();
    assert_eq!(prov.model_id, "interval-markov-1");
    assert_eq!(prov.seed, "42");

    assert_eq!(prov.document_sha256.len(), 64);

    // T54: accept → one transaction of planned InsertNoteOps.
    let reval = fresh_reval(&fx, &rec);
    let plan = plan_accept(&rec, 1, None, &reval, true).unwrap();
    assert!(!plan.transaction_id.is_empty());
    assert_eq!(
        plan.inserts.len(),
        plan.inserts
            .iter()
            .map(|i| i.note_id.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    );
    // locked range covers the first 480k ticks — those notes dropped.
    assert!(!plan.dropped_indices.is_empty());
    let done = commit_accepted(&fx.svc.store, &rec.proposal_id, &plan).unwrap();
    assert_eq!(done.status, ProposalStatus::Accepted);
    assert_eq!(
        done.accepted.as_ref().unwrap().note_ids.len(),
        plan.inserts.len()
    );
    // terminal: second accept now illegal.
    assert!(plan_accept(&done, 1, None, &reval, true).is_err());
}

#[test]
fn deterministic_seed_reproduces_document() {
    let fx = fix();
    let shared = ctx(); // same scoped context both runs — ids included
    let mut shas = Vec::new();
    for _ in 0..2 {
        let pending = fx
            .svc
            .request(&req_ctx(shared.clone(), Some(42), &fx))
            .unwrap();
        let (job, outcome) = run_job(&fx, &pending);
        let rec = fx
            .svc
            .collect(&pending.record.proposal_id, &job, &outcome)
            .unwrap();
        shas.push(rec.provenance.unwrap().document_sha256);
    }
    assert_eq!(shas[0], shas[1], "same seed ⇒ same document (T53)");
}

#[test]
fn stale_revalidation_blocks_accept_and_supersedes() {
    let fx = fix();
    let pending = fx.svc.request(&req(Some(7), &fx)).unwrap();
    let (job, outcome) = run_job(&fx, &pending);
    let rec = fx
        .svc
        .collect(&pending.record.proposal_id, &job, &outcome)
        .unwrap();
    // T55: context moved since generation → accept refuses.
    let mut reval = fresh_reval(&fx, &rec);
    reval.current_context_sha256 = "0".repeat(64);
    assert!(matches!(
        plan_accept(&rec, 1, None, &reval, true),
        Err(ProposalError::Revalidation(_))
    ));
    reval.current_context_sha256 = rec.context_sha256.clone();
    reval.target_clip_exists = false;
    assert!(plan_accept(&rec, 1, None, &reval, true).is_err());
    // Sweep marks stale; revalidation mints a NEW record via supersedes.
    mark_stale_one(&fx.svc.store, &rec.proposal_id, StaleCause::ContextChanged).unwrap();
    let stale = fx.svc.store.load(&rec.proposal_id).unwrap();
    assert_eq!(stale.status, ProposalStatus::Stale);
    let new_pending = fx
        .svc
        .revalidate(&rec.proposal_id, &req(Some(7), &fx))
        .unwrap();
    assert_eq!(
        new_pending.record.supersedes.as_deref(),
        Some(rec.proposal_id.as_str())
    );
    assert_eq!(new_pending.record.status, ProposalStatus::Pending);
}

#[test]
fn reject_terminal_and_partial_selection() {
    let fx = fix();
    let pending = fx.svc.request(&req(Some(9), &fx)).unwrap();
    let (job, outcome) = run_job(&fx, &pending);
    let rec = fx
        .svc
        .collect(&pending.record.proposal_id, &job, &outcome)
        .unwrap();
    let reval = fresh_reval(&fx, &rec);
    // Partial accept: subset of note indices (T54).
    let sub = plan_accept(&rec, 2, Some(&[0, 2]), &reval, true).unwrap();
    assert!(sub.inserts.len() <= 2);
    // Reject → terminal; nothing else legal.
    let rej = reject(&fx.svc.store, &rec.proposal_id).unwrap();
    assert_eq!(rej.status, ProposalStatus::Rejected);
    assert!(reject(&fx.svc.store, &rec.proposal_id).is_err());
}

#[test]
fn worker_failure_marks_failed() {
    let fx = fix();
    let mut r = req(Some(5), &fx);
    // Worker `mode: "fail"` hook → protocol-level failure result.
    r.context.labels = vec!["x".into()];
    let mut pending = fx.svc.request(&r).unwrap();
    pending.spec.parameters["mode"] = serde_json::json!("fail");
    // resubmit the mutated spec over the recorded row:
    let db = svc_db(&fx);
    db.submit(&pending.spec).ok();
    let (job, outcome) = run_job(&fx, &pending);
    assert!(matches!(outcome, RunOutcome::Failed { .. }));
    let rec = fx
        .svc
        .collect(&pending.record.proposal_id, &job, &outcome)
        .unwrap();
    assert_eq!(rec.status, ProposalStatus::Failed);
    assert!(rec.error.is_some());
}

#[test]
fn hostile_documents_rejected() {
    let mk = |props: &str| -> Vec<u8> {
        format!(
            r#"{{"v":1,"doc":"void-proposals/1","generator":{{"id":"g","version":"1","model":"m"}},"seed":"1","proposals":[{props}]}}"#
        )
        .into_bytes()
    };
    let note = r#"{"pitch":60,"velocity":90,"onsetTicks":"0","lengthTicks":"240000"}"#;
    // baseline sanity
    assert!(parse_document(&mk(&format!(
        r#"{{"id":"c1","score":0.5,"rationale":"ok","notes":[{note}]}}"#
    )))
    .is_ok());
    // each hostile shape must fail parse:
    let hostile: Vec<Vec<u8>> = vec![
        br#"{"v":1,"doc":"evil/9","generator":{"id":"g","version":"1","model":"m"},"seed":"1","proposals":[]}"#.to_vec(),
        // score out of range / non-finite
        mk(&format!(r#"{{"id":"c1","score":1.7,"rationale":"x","notes":[{note}]}}"#)),
        // pitch out of range
        mk(r#"{"id":"c1","score":0.5,"rationale":"x","notes":[{"pitch":300,"velocity":90,"onsetTicks":"0","lengthTicks":"240000"}]}"#),
        // unordered onsets (hostile ordering)
        mk(r#"{"id":"c1","score":0.5,"rationale":"x","notes":[{"pitch":60,"velocity":90,"onsetTicks":"480000","lengthTicks":"240000"},{"pitch":61,"velocity":90,"onsetTicks":"0","lengthTicks":"240000"}]}"#),
        // negative tick field
        mk(r#"{"id":"c1","score":0.5,"rationale":"x","notes":[{"pitch":60,"velocity":90,"onsetTicks":"-5","lengthTicks":"240000"}]}"#),
        // zero length
        mk(r#"{"id":"c1","score":0.5,"rationale":"x","notes":[{"pitch":60,"velocity":90,"onsetTicks":"0","lengthTicks":"0"}]}"#),
        // > MAX_CANDIDATE_NOTES notes
        mk(&format!(
            r#"{{"id":"c1","score":0.5,"rationale":"x","notes":[{}]}}"#,
            std::iter::repeat_n(note, 1100).collect::<Vec<_>>().join(",")
        )),
        // NaN score
        mk(r#"{"id":"c1","score":null,"rationale":"x","notes":[{"pitch":60,"velocity":90,"onsetTicks":"0","lengthTicks":"240000"}]}"#),
    ];
    for (i, doc) in hostile.iter().enumerate() {
        assert!(
            parse_document(doc).is_err(),
            "hostile doc #{i} must be rejected (T56)"
        );
    }
    // >8 candidates rejected
    let many = (0..9)
        .map(|i| format!(r#"{{"id":"c{i}","score":0.5,"rationale":"x","notes":[{note}]}}"#))
        .collect::<Vec<_>>()
        .join(",");
    assert!(parse_document(&mk(&many)).is_err());
}
