//! W19 content-pack suite (TEST_MATRIX T73 model halves +
//! inventory/rights integrity).
//!
//! Covers, with real sha256 verification on real bytes:
//!   - manifest verify pass → install → semver selection
//!   - tampered payload → rescan quarantine → missing-content report →
//!     relink refuse/restore
//!   - deterministic seeded round-robin rotation
//!   - dependent-project open report: explicit rows, never silence
//!   - authored inventory + license ledger validate and cross-check

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;
use void_assets::hex_sha256;
use void_content::{
    sign_manifest, ContentError, ContentStore, FileKind, FileState, LayerSelector, LicenseLedger,
    LoopMode, Multisample, PackDependency, PackStatus, StockInventory, VoiceAllocator,
    VoiceOutcome, VoiceSteal, Zone,
};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// Write `payload` files into `dir` and return a signed manifest.json
/// covering them.
fn make_pack(dir: &Path, pack_id: &str, version: &str, payload: &[(&str, &[u8])]) {
    fs::create_dir_all(dir).unwrap();
    let files: Vec<serde_json::Value> = payload
        .iter()
        .map(|(path, bytes)| {
            let tgt = dir.join(path);
            if let Some(p) = tgt.parent() {
                fs::create_dir_all(p).unwrap();
            }
            fs::write(&tgt, bytes).unwrap();
            json!({
                "path": path,
                "sha256": hex_sha256(bytes),
                "bytes": bytes.len().to_string(),
                "kind": "sample",
            })
        })
        .collect();
    let doc = sign_manifest(json!({
        "formatVersion": 1,
        "packId": pack_id,
        "version": version,
        "name": "Test Pack",
        "family": "STOCK-03",
        "licenseId": "VOID-ORIG",
        "voiceBudget": "16",
        "cacheBudgetBytes": "1048576",
        "files": files,
    }));
    fs::write(
        dir.join("manifest.json"),
        serde_json::to_vec_pretty(&doc).unwrap(),
    )
    .unwrap();
}

#[test]
fn manifest_verify_pass_and_install() {
    let tmp = tempfile::tempdir().unwrap();
    let pack_src = tmp.path().join("src-pack");
    make_pack(
        &pack_src,
        "void-drums-core",
        "1.0.0",
        &[
            ("samples/kick.wav", b"kick-bytes"),
            ("samples/snare.wav", b"snare-bytes"),
        ],
    );
    let store = ContentStore::open(tmp.path().join("library")).unwrap();
    let rec = store.install(&pack_src).unwrap();
    assert_eq!(rec.status, PackStatus::Verified);
    assert_eq!(rec.license_id, "VOID-ORIG");
    assert_eq!(
        store.installed_version("void-drums-core").unwrap(),
        Some("1.0.0".into())
    );
    // Reinstall same id@version refuses.
    assert!(matches!(
        store.install(&pack_src),
        Err(ContentError::PackExists { .. })
    ));
}

#[test]
fn tamper_quarantine_relink_report() {
    let tmp = tempfile::tempdir().unwrap();
    let pack_src = tmp.path().join("src-pack");
    let good = b"real-pcm-bytes";
    make_pack(
        &pack_src,
        "void-strings",
        "2.1.0",
        &[("zones/cello/C2.wav", good), ("zones/cello/D2.wav", b"d2")],
    );
    let root = tmp.path().join("library");
    let store = ContentStore::open(&root).unwrap();
    store.install(&pack_src).unwrap();

    // A dependent project opens clean right now.
    let deps = [PackDependency {
        pack_id: "void-strings".into(),
        version: None,
    }];
    let report = store.dependency_report(&deps).unwrap();
    assert!(report.complete);
    assert_eq!(report.ok_packs, vec!["void-strings@2.1.0".to_string()]);

    // Corrupt a required file on disk; rescan quarantines it.
    let served = root.join("packs/void-strings/2.1.0/zones/cello/C2.wav");
    fs::write(&served, b"corrupted").unwrap();
    let rec = store.rescan("void-strings", "2.1.0").unwrap();
    assert_eq!(rec.status, PackStatus::Quarantined);
    assert_eq!(rec.files["zones/cello/C2.wav"].state, FileState::Tampered);
    // Tampered bytes are preserved under quarantine/, not served.
    assert!(!served.exists());
    assert!(root
        .join("quarantine/void-strings/2.1.0/zones/cello/C2.wav")
        .exists());

    // Dependent open now gets an explicit missing row, not silence.
    let report = store.dependency_report(&deps).unwrap();
    assert!(!report.complete);
    assert_eq!(report.missing.len(), 1);
    let row = &report.missing[0];
    assert_eq!(row.path, "zones/cello/C2.wav");
    assert_eq!(row.expected_sha256, hex_sha256(good));
    assert_eq!(row.reason, FileState::Tampered);

    // Wrong relink bytes refused with HashMismatch.
    assert!(matches!(
        store.relink(
            "void-strings",
            "2.1.0",
            "zones/cello/C2.wav",
            b"nope",
            false
        ),
        Err(ContentError::HashMismatch { .. })
    ));
    // Correct bytes restore the file and the pack verifies again.
    let state = store
        .relink("void-strings", "2.1.0", "zones/cello/C2.wav", good, false)
        .unwrap();
    assert_eq!(state, FileState::Ok);
    let rec = store.rescan("void-strings", "2.1.0").unwrap();
    assert_eq!(rec.status, PackStatus::Verified);
    assert!(store.dependency_report(&deps).unwrap().complete);
}

#[test]
fn install_refuses_tampered_payload() {
    let tmp = tempfile::tempdir().unwrap();
    let pack_src = tmp.path().join("src-pack");
    make_pack(&pack_src, "void-bad", "1.0.0", &[("a.wav", b"declared")]);
    // Corrupt the payload after manifesting.
    fs::write(pack_src.join("a.wav"), b"different-bytes").unwrap();
    let store = ContentStore::open(tmp.path().join("library")).unwrap();
    assert!(matches!(
        store.install(&pack_src),
        Err(ContentError::FileTampered { .. })
    ));
    assert!(store.list().unwrap().is_empty());
    // Nothing published into packs/.
    assert!(!tmp.path().join("library/packs/void-bad").exists());
}

#[test]
fn remove_pack_reports_missing_dependency() {
    let tmp = tempfile::tempdir().unwrap();
    let pack_src = tmp.path().join("src-pack");
    make_pack(&pack_src, "void-keys", "1.0.0", &[("piano.wav", b"p")]);
    let store = ContentStore::open(tmp.path().join("library")).unwrap();
    store.install(&pack_src).unwrap();
    store.remove("void-keys", "1.0.0").unwrap();

    let report = store
        .dependency_report(&[PackDependency {
            pack_id: "void-keys".into(),
            version: None,
        }])
        .unwrap();
    assert!(!report.complete);
    assert_eq!(report.missing.len(), 1);
    assert_eq!(report.missing[0].pack_id, "void-keys");
    assert_eq!(report.missing[0].reason, FileState::Missing);
    // Entirely-unknown pack is also an explicit row.
    let report = store
        .dependency_report(&[PackDependency {
            pack_id: "void-ghost".into(),
            version: Some("9.9.9".into()),
        }])
        .unwrap();
    assert_eq!(report.missing[0].version.as_deref(), Some("9.9.9"));
}

#[test]
fn explicit_replace_is_ledger_visible() {
    let tmp = tempfile::tempdir().unwrap();
    let pack_src = tmp.path().join("src-pack");
    make_pack(&pack_src, "void-repl", "1.0.0", &[("a.wav", b"orig")]);
    let root = tmp.path().join("library");
    let store = ContentStore::open(&root).unwrap();
    store.install(&pack_src).unwrap();
    fs::remove_file(root.join("packs/void-repl/1.0.0/a.wav")).unwrap();
    let rec = store.rescan("void-repl", "1.0.0").unwrap();
    assert_eq!(rec.files["a.wav"].state, FileState::Missing);

    let state = store
        .relink("void-repl", "1.0.0", "a.wav", b"user-replacement", true)
        .unwrap();
    assert_eq!(state, FileState::Replaced);
    let rec = store.rescan("void-repl", "1.0.0").unwrap();
    // Replaced bytes are a *recorded deviation*: pack is degraded and
    // flagged, never silently "verified".
    assert_eq!(rec.status, PackStatus::Degraded);
    assert!(rec.contains_user_replacements);
}

fn zone(id: &str, lo: u8, hi: u8, group: Option<&str>) -> Zone {
    Zone {
        zone_id: id.into(),
        key_lo: lo,
        key_hi: hi,
        vel_lo: 0,
        vel_hi: 127,
        layer: 0,
        root_key: (lo + hi) / 2,
        tune_cents: 0,
        gain_cdb: 0,
        file: format!("{id}.wav"),
        file_sha256: "0".repeat(64),
        round_robin: group.map(|g| g.into()),
        loop_mode: LoopMode::Off,
    }
}

#[test]
fn round_robin_deterministic_per_seed() {
    let zones = vec![
        zone("rr-a", 60, 72, Some("rr")),
        zone("rr-b", 60, 72, Some("rr")),
        zone("rr-c", 60, 72, Some("rr")),
    ];
    let mut a = Multisample::new(zones.clone(), 7).unwrap();
    let mut b = Multisample::new(zones, 7).unwrap();
    a.validate_groups().unwrap();

    let seq_a: Vec<String> = (0..9)
        .map(|_| {
            a.select(64, 100, LayerSelector::Any)
                .unwrap()
                .zone_id
                .clone()
        })
        .collect();
    let seq_b: Vec<String> = (0..9)
        .map(|_| {
            b.select(64, 100, LayerSelector::Any)
                .unwrap()
                .zone_id
                .clone()
        })
        .collect();
    assert_eq!(seq_a, seq_b, "same seed must reproduce the same rotation");
    // Every member visited before any repeat.
    let first3: std::collections::BTreeSet<_> = seq_a[..3].iter().collect();
    assert_eq!(first3.len(), 3);
    // Selection outside the zone's key range returns nothing.
    assert!(a.select(30, 100, LayerSelector::Any).is_none());
}

#[test]
fn voice_budget_is_a_hard_bound() {
    let mut alloc = VoiceAllocator::new(2, VoiceSteal::Deny).unwrap();
    assert!(matches!(
        alloc.note_on("z", 60, 100),
        VoiceOutcome::Allocated(_)
    ));
    assert!(matches!(
        alloc.note_on("z", 62, 100),
        VoiceOutcome::Allocated(_)
    ));
    assert!(matches!(alloc.note_on("z", 64, 100), VoiceOutcome::Denied));
    let mut steal = VoiceAllocator::new(2, VoiceSteal::StealOldest).unwrap();
    steal.note_on("z", 60, 100);
    steal.note_on("z", 62, 100);
    match steal.note_on("z", 64, 100) {
        VoiceOutcome::Stole { evicted, .. } => assert_eq!(evicted.key, 60),
        other => panic!("expected steal, got {other:?}"),
    }
}

#[test]
fn authored_inventory_and_ledger_validate() {
    let inv_text = fs::read_to_string(repo_root().join("content/inventory.json")).unwrap();
    let inv = StockInventory::parse(&inv_text).unwrap();
    assert!(
        inv.entries.len() >= 50,
        "inventory is substantive, not a stub"
    );

    // W19-tasked stock families each have coverage.
    for stock in [
        "STOCK-01", "STOCK-02", "STOCK-03", "STOCK-04", "STOCK-05", "STOCK-07", "STOCK-08",
        "STOCK-09", "STOCK-10", "STOCK-11", "STOCK-12", "STOCK-13", "STOCK-14", "STOCK-15",
        "STOCK-16", "STOCK-17", "STOCK-19", "STOCK-22",
    ] {
        assert!(
            inv.for_stock(stock).next().is_some(),
            "no entry for {stock}"
        );
    }
    // Nothing claims to be ready — honest descriptor state only.
    assert!(inv
        .entries
        .iter()
        .all(|e| e.status != void_content::EntryStatus::Ready));

    let ledger_text =
        fs::read_to_string(repo_root().join("docs/content-rights/ledger.json")).unwrap();
    let ledger = LicenseLedger::parse(&ledger_text).unwrap();
    ledger.check_against(&inv).unwrap();
}
