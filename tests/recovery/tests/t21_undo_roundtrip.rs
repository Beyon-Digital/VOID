//! T21 — undo boundary + save/close/reopen round-trip. The engine blob
//! (opaque bytes standing in for a .tracktionedit) and the user-facing
//! transaction history round-trip byte-exact; the persisted-history
//! policy states explicitly that musical undo does not survive restart.

use std::fs;
use void_project::{
    verify_checkpoint, AppState, PersistedHistory, TransactionRecord, HISTORY_POLICY, UNDO_BOUNDARY,
};
use void_recovery_tests::support::*;

fn fake_engine_blob(seed: u8) -> Vec<u8> {
    // Opaque bytes — the coordinator supplies real engine output; the
    // checkpoint layer must not interpret or transform them.
    let mut v = vec![0xDE, 0xAD, 0xBE, 0xEF, seed];
    v.extend_from_slice(&[0u8; 4096]);
    v
}

#[test]
fn engine_state_round_trips_byte_exact() {
    let p = TempProject::new();
    let blob = fake_engine_blob(42);

    // Two edits (opaque engine diffs), then save.
    let app = AppState {
        project_id: p.project_id.clone(),
        revision: "7".into(),
        visual: serde_json::json!({"zoom": 4.0, "tracks": 3}),
        history: PersistedHistory {
            transactions: vec![
                TransactionRecord {
                    transaction_id: "tx-a".into(),
                    revision: "6".into(),
                    label: "add clip".into(),
                    engine_undo_token: "eng-101".into(),
                    app_changes: serde_json::json!({"clip": "c1"}),
                },
                TransactionRecord {
                    transaction_id: "tx-b".into(),
                    revision: "7".into(),
                    label: "move clip".into(),
                    engine_undo_token: "eng-102".into(),
                    app_changes: serde_json::json!({"clip": "c1", "pos": 96}),
                },
            ],
            ..Default::default()
        },
        alternatives: vec![],
    };
    let bundle = void_project::SnapshotBundle {
        engine_snapshot: blob.clone(),
        app_state: app.serialize().unwrap(),
        command_receipts: receipts_bytes(&p.project_id, 7),
        asset_hashes: vec![],
    };
    let r = void_project::save(
        &p.root,
        void_project::SaveRequest {
            project_id: p.project_id.clone(),
            revision: 7,
            engine_revision: "t".into(),
            parent_checkpoint_id: None,
        },
        &bundle,
    )
    .unwrap();

    // "Close/reopen": verify the checkpoint from a cold read.
    let cur = verify_checkpoint(&p.root, &r.checkpoint_id).unwrap();
    let dir = &cur.dir;

    // Engine blob identical byte-for-byte.
    assert_eq!(fs::read(dir.join("engine.tracktionedit")).unwrap(), blob);

    // App state decodes with history and metadata intact.
    let app: AppState = AppState::parse(&fs::read(dir.join("app-state.json")).unwrap()).unwrap();
    assert_eq!(app.revision, "7");
    assert_eq!(app.history.transactions.len(), 2);
    assert_eq!(app.history.transactions[1].label, "move clip");
    assert_eq!(app.history.transactions[1].engine_undo_token, "eng-102");
    assert_eq!(app.visual["zoom"], 4.0);
}

#[test]
fn persisted_history_policy_is_explicit() {
    // The policy strings are user-visible truth, not a buried constant.
    assert!(UNDO_BOUNDARY.contains("not restored") || UNDO_BOUNDARY.contains("not persisted"));
    assert!(HISTORY_POLICY.contains("musical result"));
    // And it lands inside every saved app-state.
    let p = TempProject::new();
    let r = p.save_at(1);
    let cur = verify_checkpoint(&p.root, &r.checkpoint_id).unwrap();
    let app: AppState =
        AppState::parse(&fs::read(cur.dir.join("app-state.json")).unwrap()).unwrap();
    assert_eq!(app.history.policy, HISTORY_POLICY);
    assert_eq!(app.history.undo_boundary, UNDO_BOUNDARY);
    // History cursor points at the saved revision — no fabricated entries.
    assert_eq!(app.history.history_cursor, "1");
}

#[test]
fn edits_between_saves_stay_ordered() {
    let p = TempProject::new();
    let r1 = p.save_at(1);
    let r2 = p.save_with_parent(2, Some(r1.checkpoint_id.clone()));
    let r3 = p.save_with_parent(3, Some(r2.checkpoint_id.clone()));

    // Parentage forms a real lineage — not timestamp-derived.
    let m2 = verify_checkpoint(&p.root, &r2.checkpoint_id)
        .unwrap()
        .manifest;
    assert_eq!(
        m2.parent_checkpoint_id.as_deref(),
        Some(r1.checkpoint_id.as_str())
    );
    let m3 = verify_checkpoint(&p.root, &r3.checkpoint_id)
        .unwrap()
        .manifest;
    assert_eq!(
        m3.parent_checkpoint_id.as_deref(),
        Some(r2.checkpoint_id.as_str())
    );

    // CURRENT points at the last verified save; both prior remain retained.
    let report = void_project::recover(&p.root).unwrap();
    assert_eq!(report.current.unwrap().checkpoint_id, r3.checkpoint_id);
    assert!(report
        .retained
        .iter()
        .any(|c| c.checkpoint_id == r1.checkpoint_id));
}
