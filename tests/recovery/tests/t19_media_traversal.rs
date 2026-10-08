//! T19 — media integrity and traversal. Corrupt media is rejected before
//! decode; archives are validated entry-by-entry before a single byte is
//! extracted; missing media yields relink placeholders, never substitutes.

use std::fs;
use void_assets::{
    import_zip, probe_wav, relink, resolve, AssetError, LinkResolution, MediaLink, RelinkOutcome,
    ARCHIVE_MAX_ENTRY_BYTES,
};
use void_recovery_tests::support::*;

#[test]
fn corrupt_and_truncated_wav_rejected() {
    for (name, bytes) in corrupt_wavs() {
        assert!(
            probe_wav(&bytes).is_err(),
            "corrupt wav {name} accepted by probe"
        );
    }
    // Sanity: a valid wav passes and reports real metadata.
    let good = wav_bytes(2, 48000, 24, 128);
    let info = probe_wav(&good).unwrap();
    assert_eq!(info.channels, 2);
    assert_eq!(info.sample_rate, 48000);
    assert_eq!(info.frames, 128);
}

#[test]
fn traversal_archive_rejected_wholesale() {
    let p = TempProject::new();
    let store = p.assets();
    let zip_path = p._dir.path().join("evil.zip");
    evil_zip(&zip_path, 0);
    let outcome = import_zip(&store, &zip_path);
    assert!(
        matches!(outcome, Err(AssetError::UnsafeEntry(_))),
        "got {outcome:?}"
    );
    // NOTHING was extracted — validation precedes any byte write.
    assert!(store.list().unwrap().is_empty());
    // Nothing escaped the container either.
    assert!(!p.root.join("escape.txt").exists());
    assert!(!p._dir.path().join("escape.txt").exists());
}

#[test]
fn oversized_and_bomb_entries_rejected() {
    let p = TempProject::new();
    let store = p.assets();
    let zip_path = p._dir.path().join("bomb.zip");
    // One entry larger than the per-entry cap.
    evil_zip(&zip_path, (ARCHIVE_MAX_ENTRY_BYTES + 1024) as usize);
    let outcome = import_zip(&store, &zip_path);
    assert!(outcome.is_err());
    assert!(store.list().unwrap().is_empty());
}

#[test]
fn clean_archive_imports_verified_blobs() {
    let p = TempProject::new();
    let store = p.assets();
    let zip_path = p._dir.path().join("good.zip");
    good_zip(&zip_path);
    let refs = import_zip(&store, &zip_path).unwrap();
    assert_eq!(refs.len(), 2);
    for r in &refs {
        store.verify(&r.sha256).unwrap();
    }
    // The wav entry's content actually decodes.
    let wav = refs.iter().find(|r| r.ext == "wav").unwrap();
    probe_wav(&fs::read(&wav.path).unwrap()).unwrap();
}

#[test]
fn stored_blob_tamper_detected() {
    let p = TempProject::new();
    let store = p.assets();
    let r = store
        .import_bytes(&wav_bytes(1, 44100, 16, 16), "wav")
        .unwrap();
    // Flip a byte in the stored blob — verify must catch it.
    let mut bytes = read(&r.path);
    bytes[20] ^= 0xFF;
    fs::write(&r.path, &bytes).unwrap();
    assert!(matches!(
        store.verify(&r.sha256),
        Err(AssetError::HashMismatch { .. })
    ));
}

#[test]
fn missing_media_is_placeholder_never_substitute() {
    let p = TempProject::new();
    let store = p.assets();
    let expected = void_assets::hex_sha256(b"the real audio");

    // A project reference to media that isn't in the store.
    let link = MediaLink::Missing {
        expected_sha256: expected.clone(),
        display_name: "vocal-take.wav".into(),
        detail: "import interrupted".into(),
    };
    match resolve(&link, &store) {
        LinkResolution::Placeholder {
            expected_sha256,
            display_name,
            ..
        } => {
            assert_eq!(expected_sha256, expected);
            assert_eq!(display_name, "vocal-take.wav");
        }
        other => panic!("missing media resolved to {other:?} — silent substitute"),
    }

    // Wrong bytes + no explicit replace → refused, nothing imported.
    let wrong = b"different audio";
    let outcome = relink(&link, &store, wrong, "wav", false);
    assert!(matches!(outcome, Err(AssetError::HashMismatch { .. })));
    assert!(store.list().unwrap().is_empty());

    // Wrong bytes + explicit user replacement → recorded replacement.
    let (new_link, outcome) = relink(&link, &store, wrong, "wav", true).unwrap();
    assert!(matches!(
        outcome,
        RelinkOutcome::Replaced { ref was_expected, .. } if *was_expected == expected
    ));
    assert!(matches!(new_link, MediaLink::Present { .. }));

    // Correct bytes relink exactly.
    let store2 = p.assets();
    let link2 = MediaLink::Missing {
        expected_sha256: expected.clone(),
        display_name: "vocal-take.wav".into(),
        detail: String::new(),
    };
    let (_, outcome2) = relink(&link2, &store2, b"the real audio", "wav", false).unwrap();
    assert!(matches!(outcome2, RelinkOutcome::Relinked { .. }));
}
