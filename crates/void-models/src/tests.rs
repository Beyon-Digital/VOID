//! void-models tests — manifest integrity, runtime allowlist, artifact
//! verification and registry persistence (T51/T52).

use super::*;
use serde_json::json;

fn manifest_doc() -> serde_json::Value {
    json!({
        "formatVersion": 1,
        "modelId": "void.fake-synth",
        "version": "1.0.0",
        "name": "Fake Synth (test)",
        "kind": "symbolic",
        "runtime": {
            "kind": "argv",
            "executable": "void-fake-worker",
            "workerProtocol": 1
        },
        "artifacts": [{
            "sha256": "0".repeat(64),
            "ext": "bin",
            "bytes": "4",
            "required": true
        }],
        "capabilities": {
            "taskKinds": ["symbolic", "analysis"],
            "hardware": ["cpu"],
            "requiresGpu": false,
            "maxInputBytes": "1048576"
        },
        "budgets": {
            "cpuSeconds": "30",
            "memoryBytes": "268435456",
            "vramBytes": "0",
            "wallNs": "60000000000",
            "cpuThreads": 1
        },
        "license": "ISC",
        "source": "bundled"
    })
}

fn manifest_bytes() -> Vec<u8> {
    serde_json::to_vec_pretty(&sign_manifest(manifest_doc())).unwrap()
}

fn store_with(blob: &[u8]) -> (tempfile::TempDir, void_assets::AssetStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = void_assets::AssetStore::new(dir.path().join("assets"), 1 << 20).unwrap();
    if !blob.is_empty() {
        store.import_bytes(blob, "bin").unwrap();
    }
    (dir, store)
}

#[test]
fn valid_manifest_parses_and_verifies() {
    let bytes = manifest_bytes();
    let m = ModelManifest::parse_and_verify(&bytes).unwrap();
    assert_eq!(m.model_id, "void.fake-synth");
    assert_eq!(m.runtime.executable, "void-fake-worker");
    assert_eq!(m.budgets.cpu_seconds, "30");
    assert_eq!(m.budgets.memory_bytes, "268435456");
}

#[test]
fn tampered_manifest_rejected() {
    let mut bytes = manifest_bytes();
    // Flip one byte inside a name field — integrity must catch it.
    let pos = bytes.windows(4).position(|w| w == b"Fake").unwrap();
    bytes[pos] = b'X';
    assert!(matches!(
        ModelManifest::parse_and_verify(&bytes),
        Err(ModelError::ManifestTampered { .. })
    ));
}

#[test]
fn remote_code_fields_rejected() {
    for key in ["url", "download", "script", "command", "shell"] {
        let mut doc = manifest_doc();
        doc[key] = json!("https://evil.example/x");
        let bytes = serde_json::to_vec(&sign_manifest(doc)).unwrap();
        // Integrity holds (we re-signed), but the closed field set must
        // refuse the document entirely.
        let r = ModelManifest::parse_and_verify(&bytes);
        assert!(r.is_err(), "manifest with {key:?} must be rejected");
    }
}

#[test]
fn path_like_executable_rejected() {
    for exe in ["../bin/sh", "/usr/bin/python", "a/b", "sh -c x", ".."] {
        let mut doc = manifest_doc();
        doc["runtime"]["executable"] = json!(exe);
        let bytes = serde_json::to_vec(&sign_manifest(doc)).unwrap();
        assert!(
            matches!(
                ModelManifest::parse_and_verify(&bytes),
                Err(ModelError::DisallowedCapability(_)) | Err(ModelError::InvalidManifest(_))
            ),
            "executable {exe:?} must be rejected"
        );
    }
}

#[test]
fn zero_budget_is_not_a_budget() {
    let mut doc = manifest_doc();
    doc["budgets"]["cpuSeconds"] = json!("0");
    let bytes = serde_json::to_vec(&sign_manifest(doc)).unwrap();
    assert!(ModelManifest::parse_and_verify(&bytes).is_err());
}

#[test]
fn install_resolves_artifact_statuses() {
    // Artifact content "test" → its real sha goes in the manifest.
    let blob = b"test";
    let sha = void_assets::hex_sha256(blob);
    let (_dir, store) = store_with(blob);

    let mut doc = manifest_doc();
    doc["artifacts"][0]["sha256"] = json!(sha);
    doc["artifacts"][0]["bytes"] = json!("4");
    let bytes = serde_json::to_vec(&sign_manifest(doc)).unwrap();

    let reg = ModelRegistry::open_memory().unwrap();
    let desc = reg.install(&bytes, &store).unwrap();
    assert_eq!(desc.status, ModelStatus::Available);
    assert_eq!(
        reg.get("void.fake-synth", "1.0.0").unwrap().status,
        ModelStatus::Available
    );
}

#[test]
fn missing_required_artifact_marks_missing_not_usable() {
    let (_dir, store) = store_with(b""); // empty store — artifact absent
    let reg = ModelRegistry::open_memory().unwrap();
    let desc = reg.install(&manifest_bytes(), &store).unwrap();
    assert_eq!(desc.status, ModelStatus::Missing);
    assert!(!desc.status.runnable());
    // Still listed — the UI must show the unavailable/install state.
    assert_eq!(reg.list().unwrap().len(), 1);
}

#[test]
fn missing_optional_artifact_degrades() {
    let mut doc = manifest_doc();
    doc["artifacts"][0]["required"] = json!(false);
    let (_dir, store) = store_with(b"");
    let reg = ModelRegistry::open_memory().unwrap();
    let desc = reg
        .install(&serde_json::to_vec(&sign_manifest(doc)).unwrap(), &store)
        .unwrap();
    assert_eq!(desc.status, ModelStatus::Degraded);
    assert!(desc.status.runnable());
}

#[test]
fn corrupt_artifact_rejected() {
    // Store a blob under a DIFFERENT sha than the manifest declares —
    // verify() recomputes and catches the mismatch... but import puts
    // content at its real address; to simulate corruption we point the
    // manifest at a sha whose stored bytes differ from the declared
    // expectation: import blob A, manifest claims blob A's sha but
    // declared bytes differ → still verifies. Real corruption: write a
    // file at the sha address with wrong content.
    let blob = b"test";
    let sha = void_assets::hex_sha256(blob);
    let (_dir, store) = store_with(b"");
    // Manually plant a corrupt blob at the declared address.
    let addr = store.root().join("sha256").join(format!("{sha}.bin"));
    std::fs::write(&addr, b"corrupt!").unwrap();

    let mut doc = manifest_doc();
    doc["artifacts"][0]["sha256"] = json!(sha);
    let reg = ModelRegistry::open_memory().unwrap();
    assert!(matches!(
        reg.install(&serde_json::to_vec(&sign_manifest(doc)).unwrap(), &store),
        Err(ModelError::MissingArtifact(_))
    ));
    // Refused manifests leave no row.
    assert!(reg.list().unwrap().is_empty());
}

#[test]
fn resolve_rechecks_files_after_install() {
    let blob = b"test";
    let sha = void_assets::hex_sha256(blob);
    let (_dir, store) = store_with(blob);
    let mut doc = manifest_doc();
    doc["artifacts"][0]["sha256"] = json!(sha);
    let reg = ModelRegistry::open_memory().unwrap();
    reg.install(&serde_json::to_vec(&sign_manifest(doc)).unwrap(), &store)
        .unwrap();

    // Delete the artifact file post-install → resolve must flip to missing.
    let addr = store.find(&sha).unwrap();
    std::fs::remove_file(&addr).unwrap();
    let desc = reg.resolve("void.fake-synth", "1.0.0", &store).unwrap();
    assert_eq!(desc.status, ModelStatus::Missing);
    // The row was updated — stale "available" never survives a resolve.
    assert_eq!(
        reg.get("void.fake-synth", "1.0.0").unwrap().status,
        ModelStatus::Missing
    );
}

#[test]
fn remove_deletes_record_and_budgets_roundtrip() {
    let (_dir, store) = store_with(b"");
    let reg = ModelRegistry::open_memory().unwrap();
    reg.install(&manifest_bytes(), &store).unwrap();
    let b = reg.budget_for("void.fake-synth", "1.0.0").unwrap();
    assert_eq!(b.cpu_seconds, "30");
    assert!(reg.remove("void.fake-synth", "1.0.0").unwrap());
    assert!(matches!(
        reg.get("void.fake-synth", "1.0.0"),
        Err(ModelError::NotFound(_))
    ));
}

#[test]
fn registry_persists_in_index_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("index.sqlite");
    {
        let reg = ModelRegistry::open(&path).unwrap();
        let (_d, store) = store_with(b"");
        reg.install(&manifest_bytes(), &store).unwrap();
    }
    // Reopen — rows survive; the index is reconstructable state.
    let reg = ModelRegistry::open(&path).unwrap();
    assert_eq!(reg.list().unwrap().len(), 1);
}
