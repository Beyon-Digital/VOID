//! T18 — disk-full and permissions. A save under a full or unwritable
//! filesystem must never report durable success; the last verified
//! checkpoint stays readable; recording salvage stays explicit.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use void_recovery_tests::support::*;

fn readonly(path: &std::path::Path, ro: bool) {
    let mut perms = fs::metadata(path).unwrap().permissions();
    perms.set_mode(if ro { 0o555 } else { 0o755 });
    fs::set_permissions(path, perms).unwrap();
}

#[test]
fn disk_full_via_kernel_rlimit_refuses_durable() {
    // Real OS enforcement: helper subprocess sets RLIMIT_FSIZE=64KiB and
    // ignores SIGXFSZ, so staged writes fail EFBIG mid-flight.
    let p = TempProject::new();
    let r1 = p.save_at(1);
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_diskfull_helper"))
        .arg(&p.root)
        .arg("2")
        .arg(&p.project_id)
        .arg(&r1.checkpoint_id)
        .output()
        .expect("spawn diskfull_helper");
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "helper failed: {stdout} {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(stdout.contains("SAVE_REFUSED"), "stdout: {stdout}");
    assert!(
        stdout.contains(&format!("LAST_OK {}", r1.checkpoint_id)),
        "stdout: {stdout}"
    );
    // In-process check too: cp1 still the verified authority.
    let report = void_project::recover(&p.root).unwrap();
    assert_eq!(report.current.unwrap().checkpoint_id, r1.checkpoint_id);
}

#[test]
fn permission_denied_on_staging_refuses_durable() {
    let p = TempProject::new();
    let r1 = p.save_at(1);
    let staging = p.root.join("staging");
    readonly(&staging, true);
    let outcome = void_project::save(
        &p.root,
        void_project::SaveRequest {
            project_id: p.project_id.clone(),
            revision: 2,
            engine_revision: "t".into(),
            parent_checkpoint_id: Some(r1.checkpoint_id.clone()),
        },
        &p.bundle(2, vec![]),
    );
    readonly(&staging, false);
    assert!(outcome.is_err(), "save must refuse under denied staging");
    // No false durable success: CURRENT untouched, cp1 verifies.
    let report = void_project::recover(&p.root).unwrap();
    assert_eq!(report.current.unwrap().checkpoint_id, r1.checkpoint_id);
}

#[test]
fn permission_denied_on_assets_leaves_store_consistent() {
    let p = TempProject::new();
    let store = p.assets();
    let sha_dir = p.root.join("assets").join("sha256");
    readonly(&sha_dir, true);
    let outcome = store.import_bytes(&wav_bytes(2, 44100, 16, 8), "wav");
    readonly(&sha_dir, false);
    assert!(outcome.is_err());
    // No partial blob published; store lists nothing.
    assert!(store.list().unwrap().is_empty());
    // Incoming temp is cleaned up.
    assert!(fs::read_dir(p.root.join("assets").join(".incoming"))
        .unwrap()
        .next()
        .is_none());
}

#[test]
fn recording_finalize_failure_salvage_is_explicit() {
    let p = TempProject::new();
    // Take that was interrupted (journal says finalized=false, one good
    // chunk + one truncated chunk already on disk).
    let take_dir = seed_recording(&p.root, "take-1", true);

    // Salvage lists verified chunks explicitly — never auto-promotes.
    let salvage = void_project::recover(&p.root).unwrap().salvaged_recordings;
    let take = salvage.iter().find(|t| t.take_id == "take-1").unwrap();
    assert!(!take.finalized);
    assert_eq!(take.valid_chunks, vec!["chunk-0.wav".to_string()]);
    assert_eq!(take.corrupt_chunks.len(), 1);

    // A finalize attempt under read-only take dir fails without damage
    // (writing a NEW marker needs dir write permission).
    readonly(&take_dir, true);
    let write = fs::write(take_dir.join("finalized.marker"), b"{}");
    readonly(&take_dir, false);
    assert!(write.is_err());
    // Chunks still verify — salvage unchanged.
    let salvage2 = void_project::recover(&p.root).unwrap().salvaged_recordings;
    let take2 = salvage2.iter().find(|t| t.take_id == "take-1").unwrap();
    assert_eq!(take2.valid_chunks, vec!["chunk-0.wav".to_string()]);
}
