//! T17 — checkpoint failpoints. Kill after every staged write, flush,
//! directory rename, pointer publish and DB reconciliation step; recovery
//! must only ever select a complete verified checkpoint.

use std::fs;
use void_project::{
    committed_command_ids, parse_log, recover, verify_checkpoint, PointerStatus, SaveStep,
};
use void_recovery_tests::support::*;

/// Drive a session through the pipeline; returns Err(Failpoint) at the
/// injected boundary. The session is then DROPPED without abort — the
/// equivalent of the process dying at that point.
fn run_with_failpoint(p: &TempProject, step: SaveStep, cp2: &str, parent: Option<String>) {
    let bundle = p.bundle(2, vec![]);
    let mut s = p.session(2, cp2, parent);
    s.set_failpoint(step);
    let staged = s.stage(&bundle).and_then(|_| s.write_manifest());
    match staged.and_then(|_| s.publish().map(|_| ())) {
        Err(void_project::ProjectError::Failpoint(_)) => {
            // crash: no abort, session dropped mid-flight
            drop(s);
        }
        other => panic!("expected failpoint at {step:?}, got {other:?}"),
    }
}

#[test]
fn kill_after_each_save_boundary() {
    for step in [
        SaveStep::StageDir,
        SaveStep::EngineSnapshot,
        SaveStep::AppState,
        SaveStep::Receipts,
        SaveStep::Manifest,
        SaveStep::RenameCheckpoint,
        SaveStep::PublishCurrent,
    ] {
        let p = TempProject::new();
        let r1 = p.save_at(1); // CP1 durable
        let cp2 = uuid::Uuid::new_v4().to_string();
        run_with_failpoint(&p, step, &cp2, Some(r1.checkpoint_id.clone()));

        let report = recover(&p.root).unwrap();
        match step {
            SaveStep::PublishCurrent => {
                // CURRENT landed durably before the "crash" — the save
                // effectively completed; cp2 is the verified authority.
                let cur = report.current.as_ref().expect("current after publish");
                assert_eq!(cur.checkpoint_id, cp2, "step {step:?}");
            }
            SaveStep::RenameCheckpoint => {
                // Complete-but-unpublished checkpoint: cp1 remains current;
                // cp2 must surface as a labelled orphan, never promoted.
                assert_eq!(
                    report.current.as_ref().unwrap().checkpoint_id,
                    r1.checkpoint_id,
                    "step {step:?}"
                );
                assert!(report
                    .orphan_candidates
                    .iter()
                    .any(|c| c.checkpoint.checkpoint_id == cp2 && c.reason.contains("orphan")));
                assert_eq!(report.current.as_ref().unwrap().revision, 1);
            }
            _ => {
                // Anything earlier never entered checkpoints/ — staged
                // bytes are quarantined and cp1 is untouched.
                assert_eq!(
                    report.current.as_ref().unwrap().checkpoint_id,
                    r1.checkpoint_id,
                    "step {step:?}"
                );
                assert!(
                    !p.root.join("checkpoints").join(&cp2).exists(),
                    "uncommitted checkpoint leaked at {step:?}"
                );
            }
        }
        // Staging is never authoritative and is quarantined on recovery.
        assert!(
            !report.quarantined_staging.is_empty()
                || step == SaveStep::RenameCheckpoint
                || step == SaveStep::PublishCurrent,
            "step {step:?}"
        );
        // The committed checkpoint always verifies end to end.
        let cur = report.current.unwrap();
        verify_checkpoint(&p.root, &cur.checkpoint_id).unwrap();
    }
}

#[test]
fn torn_pointer_and_manifest_variants() {
    // Corrupt CURRENT bytes.
    let p = TempProject::new();
    let r1 = p.save_at(1);
    fs::write(p.root.join("CURRENT"), b"not-json{{{").unwrap();
    let report = recover(&p.root).unwrap();
    assert!(matches!(
        report.pointer,
        Some(PointerStatus::Corrupt { .. })
    ));
    assert!(report.current.is_none());
    // cp1 is a complete verified checkpoint → offered, never promoted.
    assert_eq!(
        report.orphan_candidates[0].checkpoint.checkpoint_id,
        r1.checkpoint_id
    );

    // Missing CURRENT entirely.
    let current_bytes = read(&p.root.join("CURRENT"));
    fs::remove_file(p.root.join("CURRENT")).unwrap();
    let report = recover(&p.root).unwrap();
    assert!(matches!(report.pointer, Some(PointerStatus::Missing)));
    assert_eq!(
        report.orphan_candidates[0].checkpoint.checkpoint_id,
        r1.checkpoint_id
    );

    // Truncated manifest inside the committed checkpoint (CURRENT intact).
    let mf = p
        .root
        .join("checkpoints")
        .join(&r1.checkpoint_id)
        .join("manifest.json");
    fs::write(p.root.join("CURRENT"), &current_bytes).unwrap();
    let orig = read(&mf);
    fs::write(&mf, &orig[..orig.len() / 2]).unwrap();
    let report = recover(&p.root).unwrap();
    assert!(matches!(
        report.pointer,
        Some(PointerStatus::Corrupt { .. })
    ));
    assert!(report
        .corrupt_checkpoints
        .iter()
        .any(|(id, _)| id == &r1.checkpoint_id));

    // Tampered payload (flip a byte in the engine snapshot).
    fs::write(&mf, &orig).unwrap();
    let snap = p
        .root
        .join("checkpoints")
        .join(&r1.checkpoint_id)
        .join("engine.tracktionedit");
    let mut bytes = read(&snap);
    bytes[0] ^= 0xFF;
    fs::write(&snap, &bytes).unwrap();
    let report = recover(&p.root).unwrap();
    assert!(matches!(
        report.pointer,
        Some(PointerStatus::Corrupt { .. })
    ));
}

#[test]
fn db_reconcile_kill_then_repair() {
    use void_jobs::{reconcile, JobDb, ReconcileOutcome};
    let p = TempProject::new();
    p.save_at(1);
    let r2 = p.save_at(2);
    let db = JobDb::open_memory().unwrap();

    // Simulate: index only ever saw CP1 (reconcile died before updating).
    db.upsert_current_checkpoint(&p.project_id, "cp1-stale", 1, "deadbeef", &[])
        .unwrap();

    // Recovery verifies CURRENT → cp2, then reconcile repairs the index.
    let report = recover(&p.root).unwrap();
    let cur = report.current.unwrap();
    assert_eq!(cur.checkpoint_id, r2.checkpoint_id);
    let outcome = reconcile(
        &db,
        &p.project_id,
        Some((
            &cur.checkpoint_id,
            cur.revision,
            &cur.manifest_sha256,
            &cur.manifest.asset_hashes,
        )),
    )
    .unwrap();
    assert!(matches!(
        outcome,
        ReconcileOutcome::Repaired {
            previous_indexed: Some(ref prev)
        } if prev.as_str() == "cp1-stale"
    ));
    assert_eq!(
        db.indexed_current(&p.project_id).unwrap().as_deref(),
        Some(r2.checkpoint_id.as_str())
    );

    // With no valid pointer, reconcile refuses to fabricate authority.
    let outcome = reconcile(&db, &p.project_id, None).unwrap();
    assert_eq!(outcome, ReconcileOutcome::NoAuthority);
}

#[test]
fn committed_receipts_survive_recovery() {
    let p = TempProject::new();
    let r = p.save_at(3);
    let cur = verify_checkpoint(&p.root, &r.checkpoint_id).unwrap();
    let dir = cur.dir;
    let log = parse_log(&read(&dir.join("command-receipts.json"))).unwrap();
    let committed = committed_command_ids(&log);
    // A live-session command newer than the checkpoint is uncertain.
    let live = ["cmd-0002", "cmd-live-new"];
    let unknown = void_project::uncertain_commands(live, &committed);
    assert_eq!(unknown, vec!["cmd-live-new".to_string()]);
}
