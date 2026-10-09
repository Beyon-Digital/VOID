//! T85 — Visual generation job flow: declarative spec → void-jobs job
//! → scene.json + sha256 assets → provenance → accept/reject/stale.
//! Forbidden scene actions and invalid specs rejected; a bad shader
//! body in the doc fails the generation (no fake success).

mod common;
use common::*;
use void_jobs::{JobStatus, RunOutcome};
use void_visfx::{
    Revalidation, SceneGenKind, SceneGenSpec, VisFxError, VisGenStatus,
};

fn spec() -> SceneGenSpec {
    let mut s = SceneGenSpec::new(SceneGenKind::ScenePatch, 960_000, 42);
    s.params.insert("density".into(), 0.75.into());
    s
}

fn request(f: &mut Fixture) -> void_visfx::PendingVis {
    let project = uuid::Uuid::new_v4().to_string();
    f.svc
        .request(&f.db, &project, 7, &"ab".repeat(32), spec(), 0)
        .unwrap()
}

#[test]
fn gen_job_params_to_asset_flow() {
    let mut f = fixture();
    let pending = request(&mut f);

    // spec landed as job inputs: kind + parameters carry the scene spec
    assert_eq!(pending.spec.kind, void_jobs::JobKind::VisualGeneration);
    assert_eq!(pending.spec.parameters["tag"], "void-scene-gen-spec");
    assert_eq!(pending.spec.parameters["seed"], "42");
    assert_eq!(pending.record.status, VisGenStatus::Pending);

    // real run through the fake worker
    let rt = runtime();
    let outcome = runner(&f)
        .run(&pending.spec, &ctx(&rt, budget(60_000_000_000)))
        .unwrap();
    let job = f.db.get(&pending.spec.job_id).unwrap();
    assert_eq!(job.status, JobStatus::Succeeded);

    let rec = f
        .svc
        .collect(&pending.record.record_id, &job, &outcome)
        .unwrap();
    assert_eq!(rec.status, VisGenStatus::Ready);
    let ready = rec.ready.expect("ready facts");

    // generated output is a normal sha256 asset
    assert!(ready.artifacts.iter().any(|a| a.filename == "scene.json"));
    for a in &ready.artifacts {
        assert_eq!(a.sha256.len(), 64);
        assert!(f.store.find(&a.sha256).is_some(), "asset {} missing", a.filename);
    }

    // provenance carries generator + runtime + doc facts (T85 panel)
    let p = &ready.provenance;
    assert_eq!(p.job_id, pending.spec.job_id);
    assert_eq!(p.generator_id, "visfx-fake-worker");
    assert_eq!(p.seed, "42");
    assert_eq!(p.document_sha256.len(), 64);
    assert_eq!(ready.scene.generator_id, "visfx-fake-worker");

    // accept plan revalidates + verifies assets, then commit
    let reval = Revalidation {
        project_id: rec.project_id.clone(),
        current_context_sha256: rec.context_sha256.clone(),
        live_layer_names: vec!["gen-0".into()],
        preview_channel_exists: true,
    };
    let plan = f.svc.plan_accept(&rec.record_id, &reval).unwrap();
    assert!(!plan.verified_artifacts.is_empty());
    assert!(plan.layer_names.contains(&"gen-0".to_string()));
    let rec = f
        .svc
        .commit_accepted(&rec.record_id, &plan.transaction_id)
        .unwrap();
    assert_eq!(rec.status, VisGenStatus::Accepted);
    assert_eq!(rec.transaction_id, Some(plan.transaction_id));
}

#[test]
fn gen_job_media_artifacts_land_as_assets() {
    let mut f = fixture();
    let mut s = spec();
    s.params.insert("withMedia".into(), true.into());
    let project = uuid::Uuid::new_v4().to_string();
    let pending = f
        .svc
        .request(&f.db, &project, 7, &"ab".repeat(32), s, 0)
        .unwrap();

    let rt = runtime();
    let outcome = runner(&f)
        .run(&pending.spec, &ctx(&rt, budget(60_000_000_000)))
        .unwrap();
    let job = f.db.get(&pending.spec.job_id).unwrap();
    let rec = f
        .svc
        .collect(&pending.record.record_id, &job, &outcome)
        .unwrap();
    assert_eq!(rec.status, VisGenStatus::Ready);
    let ready = rec.ready.unwrap();
    assert!(ready.artifacts.iter().any(|a| a.filename == "frame0.bin"));
    // seed drives the artifact bytes — real generated content
    assert!(ready
        .artifacts
        .iter()
        .all(|a| a.sha256.len() == 64 && a.bytes > 0));
}

#[test]
fn forbidden_spec_fields_rejected() {
    let mut f = fixture();
    let mut s = spec();
    // smuggle an ambient capability inside params
    s.params.insert(
        "evil".into(),
        serde_json::json!({"nested": {"url": "https://x"}}),
    );
    let project = uuid::Uuid::new_v4().to_string();
    let res = f
        .svc
        .request(&f.db, &project, 7, &"ab".repeat(32), s, 0);
    let err = res.err().expect("forbidden spec must fail");
    assert!(matches!(err, VisFxError::Forbidden(_)));
    assert!(f.db.nonterminal().unwrap().is_empty());
}

#[test]
fn forbidden_scene_action_fails_collect() {
    let mut f = fixture();
    let mut s = spec();
    s.params.insert("testMode".into(), "bad_doc".into());
    let project = uuid::Uuid::new_v4().to_string();
    let pending = f
        .svc
        .request(&f.db, &project, 7, &"ab".repeat(32), s, 0)
        .unwrap();
    let rt = runtime();
    let outcome = runner(&f)
        .run(&pending.spec, &ctx(&rt, budget(60_000_000_000)))
        .unwrap();
    let job = f.db.get(&pending.spec.job_id).unwrap();
    assert_eq!(job.status, JobStatus::Succeeded); // job ran fine…
    let rec = f
        .svc
        .collect(&pending.record.record_id, &job, &outcome)
        .unwrap();
    // …but the doc's forbidden action fails validation
    assert_eq!(rec.status, VisGenStatus::Failed);
    assert!(rec.failed.unwrap().message.contains("exec_shell"));
}

#[test]
fn bad_shader_body_in_doc_fails_generation() {
    let mut f = fixture();
    let mut s = spec();
    s.params.insert("testMode".into(), "bad_shader".into());
    let project = uuid::Uuid::new_v4().to_string();
    let pending = f
        .svc
        .request(&f.db, &project, 7, &"ab".repeat(32), s, 0)
        .unwrap();
    let rt = runtime();
    let outcome = runner(&f)
        .run(&pending.spec, &ctx(&rt, budget(60_000_000_000)))
        .unwrap();
    let job = f.db.get(&pending.spec.job_id).unwrap();
    let rec = f
        .svc
        .collect(&pending.record.record_id, &job, &outcome)
        .unwrap();
    assert_eq!(rec.status, VisGenStatus::Failed);
    assert_eq!(rec.failed.unwrap().code, "shader_rejected");
}

#[test]
fn worker_failure_marks_failed_not_ready() {
    let mut f = fixture();
    let mut s = spec();
    s.params.insert("testMode".into(), "fail".into());
    let project = uuid::Uuid::new_v4().to_string();
    let pending = f
        .svc
        .request(&f.db, &project, 7, &"ab".repeat(32), s, 0)
        .unwrap();
    let rt = runtime();
    let outcome = runner(&f)
        .run(&pending.spec, &ctx(&rt, budget(60_000_000_000)))
        .unwrap();
    assert!(matches!(outcome, RunOutcome::Failed { .. }));
    let job = f.db.get(&pending.spec.job_id).unwrap();
    let rec = f
        .svc
        .collect(&pending.record.record_id, &job, &outcome)
        .unwrap();
    assert_eq!(rec.status, VisGenStatus::Failed);
}

#[test]
fn accept_reject_and_stale_lifecycle() {
    let mut f = fixture();
    let pending = request(&mut f);
    let rid = pending.record.record_id.clone();
    let project = pending.record.project_id.clone();

    // reject from pending is legal (cancel path)
    let rec = f.svc.reject(&rid, Some("user dismissed".into())).unwrap();
    assert_eq!(rec.status, VisGenStatus::Rejected);
    assert_eq!(rec.rejected.unwrap().reason.as_deref(), Some("user dismissed"));
    // rejected is terminal: reject again → invalid transition
    assert!(f.svc.reject(&rid, None).is_err());

    // stale sweep on context change (same project)
    let p2 = f
        .svc
        .request(&f.db, &project, 8, &"ab".repeat(32), spec(), 0)
        .unwrap();
    let n = f.svc.mark_stale(&project, "context_changed").unwrap();
    assert_eq!(n, 1);
    let rec = f.svc.records.get(&p2.record.record_id).unwrap();
    assert_eq!(rec.status, VisGenStatus::Stale);
    // stale can't accept
    let reval = Revalidation {
        project_id: project.clone(),
        current_context_sha256: "ab".repeat(32),
        live_layer_names: vec![],
        preview_channel_exists: true,
    };
    assert!(f.svc.plan_accept(&p2.record.record_id, &reval).is_err());
}

#[test]
fn spec_shape_validation() {
    let mut s = spec();
    s.duration_ticks = "0".into();
    assert!(s.validate().is_err());
    let mut s = spec();
    s.duration_ticks = "notanumber".into();
    assert!(s.validate().is_err());
    let mut s = spec();
    s.width = 0;
    assert!(s.validate().is_err());
    let mut s = spec();
    s.params.insert("p".into(), f64::NAN.into());
    // serde_json turns NaN into null — either way it must not crash
    let _ = s.validate();
    let mut s = spec();
    s.tag = "wrong".into();
    assert!(s.validate().is_err());
    s = spec();
    assert!(s.validate().is_ok());
    // canonical hash stable + changes with params
    let h1 = s.sha256();
    s.params.insert("x".into(), 1.into());
    assert_ne!(h1, s.sha256());
}
