//! T20 — schema migration. Older formats migrate copy-on-write with
//! verified rollback; newer-unknown formats are read-only/refused, and
//! their bytes are never overwritten; injected failures preserve the
//! original.

use std::fs;
use void_project::{
    inspect, migrate, open, rollback, MigrateStep, OpenMode, OpenOutcome, ProjectError,
};
use void_recovery_tests::support::*;

fn write_meta_v0(root: &std::path::Path, project_id: &str) -> Vec<u8> {
    // v0 predates formatVersion — the field is simply absent.
    let meta = serde_json::json!({
        "projectId": project_id,
        "name": "legacy-song",
        "createdAt": "2026-01-01T00:00:00Z"
    });
    let bytes = serde_json::to_vec_pretty(&meta).unwrap();
    fs::write(root.join("project.json"), &bytes).unwrap();
    bytes
}

#[test]
fn older_format_migrates_copy_on_write() {
    let p = TempProject::new();
    let original = write_meta_v0(&p.root, &p.project_id);
    let original_sha = void_assets::hex_sha256(&original);

    assert_eq!(
        inspect(&p.root).unwrap(),
        OpenOutcome::NeedsMigration { found: 0 }
    );
    let m = migrate(&p.root, None).unwrap();
    assert_eq!(m.from_version, 0);
    assert_eq!(m.to_version, 1);

    // Backup preserved the exact original bytes.
    let backup = read(&m.backup_dir.join("project.json"));
    assert_eq!(void_assets::hex_sha256(&backup), original_sha);

    // New format opens writable; all files verified.
    match open(&p.root, OpenMode::Write).unwrap() {
        OpenOutcome::Current(meta) => {
            assert_eq!(meta.format_version, 1);
            assert_eq!(meta.project_id, p.project_id);
            assert_eq!(meta.name, "legacy-song");
        }
        other => panic!("expected current, got {other:?}"),
    }
}

#[test]
fn migration_rollback_restores_exact_bytes() {
    let p = TempProject::new();
    let original = write_meta_v0(&p.root, &p.project_id);
    let m = migrate(&p.root, None).unwrap();
    rollback(&p.root, &m).unwrap();
    assert_eq!(read(&p.root.join("project.json")), original);
    assert_eq!(
        inspect(&p.root).unwrap(),
        OpenOutcome::NeedsMigration { found: 0 }
    );
}

#[test]
fn migration_failure_preserves_original() {
    let p = TempProject::new();
    let original = write_meta_v0(&p.root, &p.project_id);
    // Kill after the backup but before the rewrite.
    let outcome = migrate(&p.root, Some(MigrateStep::AfterBackup));
    assert!(matches!(outcome, Err(ProjectError::Failpoint(_))));
    // Original untouched; a rerun completes normally.
    assert_eq!(read(&p.root.join("project.json")), original);
    migrate(&p.root, None).unwrap();
    assert!(matches!(
        open(&p.root, OpenMode::Write).unwrap(),
        OpenOutcome::Current(_)
    ));
}

#[test]
fn newer_unknown_format_refused_write_never_overwritten() {
    let p = TempProject::new();
    // Forge a v2 container.
    let meta = serde_json::json!({
        "formatVersion": 2,
        "projectId": p.project_id,
        "name": "future-song",
        "createdAt": "2026-06-01T00:00:00Z"
    });
    let bytes = serde_json::to_vec_pretty(&meta).unwrap();
    fs::write(p.root.join("project.json"), &bytes).unwrap();
    let before = read(&p.root.join("project.json"));

    assert_eq!(
        inspect(&p.root).unwrap(),
        OpenOutcome::ReadOnlyNewer { found: 2 }
    );
    assert!(matches!(
        open(&p.root, OpenMode::Read).unwrap(),
        OpenOutcome::ReadOnlyNewer { found: 2 }
    ));
    assert!(matches!(
        open(&p.root, OpenMode::Write),
        Err(ProjectError::UnsupportedNewer {
            found: 2,
            supported_max: 1
        })
    ));
    // Refusal path mutated nothing.
    assert_eq!(read(&p.root.join("project.json")), before);
    // migrate() on a newer format refuses too.
    assert!(matches!(
        migrate(&p.root, None),
        Err(ProjectError::UnsupportedNewer { .. })
    ));
    assert_eq!(read(&p.root.join("project.json")), before);
}

#[test]
fn current_format_needs_no_migration() {
    let p = TempProject::new();
    assert!(matches!(inspect(&p.root).unwrap(), OpenOutcome::Current(_)));
    assert!(migrate(&p.root, None).is_err());
}
