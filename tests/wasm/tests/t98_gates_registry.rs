//! T98 — advanced synthesis / restricted extensions (Linux half):
//! declarative module spec (manifest+wasm, sha256 bound), versioned
//! registry with tamper detection, revoke semantics, and the SDK/right
//! gates staying honestly closed (ARA, plugin export).

use std::fs;

use void_wasm::*;

use void_wasm_tests::*;

fn policy() -> HostPolicy {
    HostPolicy::default()
}

fn tdir(tag: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let t = tempfile::tempdir().unwrap();
    let d = t.path().join(tag);
    fs::create_dir_all(&d).unwrap();
    (t, d)
}

fn saw_spec(dir: &std::path::Path, version: &str) -> ModuleSpec {
    make_spec(
        dir,
        "saw-gen",
        "Saw Gen",
        version,
        saw_v1_wat(),
        &["params", "state", "render-off"],
        "offline_render",
        STANDARD_LIMITS,
    )
}

// ---------- manifest schema ----------

#[test]
fn manifest_schema_validation_is_strict() {
    let wasm = wasm_of(saw_v1_wat());
    // good manifest parses
    let mj = manifest_json(
        "saw-gen",
        "Saw",
        "1.0.0",
        &wasm,
        &["params", "state"],
        "param_surface",
        STANDARD_LIMITS,
        &[],
    );
    let m = parse_manifest(&mj).unwrap();
    assert_eq!(m.module_id, "saw-gen");
    assert_eq!(m.version, "1.0.0");
    assert_eq!(m.limits.max_memory_bytes, 16 * 1024 * 1024);

    // wrong abi
    let bad = mj.replace("\"void-wasm/1\"", "\"wasi/1\"");
    assert_eq!(
        err_name(&parse_manifest(&bad).unwrap_err()),
        "InvalidManifest"
    );
    // bad module_id charset
    let bad = mj.replace("\"saw-gen\"", "\"saw/../gen\"");
    assert_eq!(
        err_name(&parse_manifest(&bad).unwrap_err()),
        "InvalidManifest"
    );
    // semver-lite violations
    let bad = mj.replace("\"1.0.0\"", "\"1.x.0\"");
    assert_eq!(
        err_name(&parse_manifest(&bad).unwrap_err()),
        "InvalidManifest"
    );
    // no capabilities
    let bad = mj.replace("\"params\",\"state\"", "");
    assert_eq!(
        err_name(&parse_manifest(&bad).unwrap_err()),
        "InvalidManifest"
    );
    // unknown capability
    let bad = mj.replace("\"params\"", "\"time-travel\"");
    assert_eq!(
        err_name(&parse_manifest(&bad).unwrap_err()),
        "InvalidManifest"
    );
    // declared over policy ceiling (policy memory = 64 MiB)
    let over = manifest_json(
        "saw-gen",
        "Saw",
        "1.0.0",
        &wasm,
        &["params"],
        "param_surface",
        &[
            ("max_memory_bytes", "134217728"), // 128 MiB > 64 MiB ceiling
            ("max_fuel", "10000000"),
            ("deadline_ms", "2000"),
            ("max_state_bytes", "65536"),
            ("max_out_bytes", "4194304"),
            ("max_params", "8"),
        ],
        &[],
    );
    assert_eq!(
        err_name(&parse_manifest(&over).unwrap_err()),
        "InvalidManifest"
    );
    // zero deadline is not a usable limit
    let zero = manifest_json(
        "saw-gen",
        "Saw",
        "1.0.0",
        &wasm,
        &["params"],
        "param_surface",
        &[
            ("max_memory_bytes", "16777216"),
            ("max_fuel", "10000000"),
            ("deadline_ms", "0"),
            ("max_state_bytes", "65536"),
            ("max_out_bytes", "4194304"),
            ("max_params", "8"),
        ],
        &[],
    );
    assert_eq!(
        err_name(&parse_manifest(&zero).unwrap_err()),
        "InvalidManifest"
    );
}

#[test]
fn manifest_binds_wasm_bytes_by_sha256() {
    let wasm = wasm_of(saw_v1_wat());
    let mut mj = manifest_json(
        "saw-gen",
        "Saw",
        "1.0.0",
        &wasm,
        &["params"],
        "param_surface",
        STANDARD_LIMITS,
        &[],
    );
    // Flip a wasm byte — the manifest's sha no longer matches.
    let mut tampered = wasm.clone();
    tampered[16] ^= 0x01;
    let (_t, d) = tdir("pkgs");
    let dir = pkg_dir(&d, "sha");
    write_package(&dir, &mj, &tampered);
    let e = ModuleSpec::from_dir(&dir, &policy()).unwrap_err();
    assert_eq!(err_name(&e), "Tampered");

    // And a manifest that lies about the hash outright (64 hex chars,
    // format-valid — must still fail the bind, not merely the schema).
    mj = mj.replace(&sha256_hex(&wasm), &"0".repeat(64));
    write_package(&dir, &mj, &wasm);
    let e = ModuleSpec::from_dir(&dir, &policy()).unwrap_err();
    assert_eq!(err_name(&e), "Tampered");
}

// ---------- registry ----------

#[test]
fn registry_installs_versions_and_serves_latest_active() {
    let (_t, d) = tdir("pkgs");
    let (_t2, regdir) = tdir("reg");
    let mut reg = ModuleRegistry::open(&regdir).unwrap();

    for (tag, v) in [("v1", "1.0.0"), ("v2", "2.0.0"), ("v3", "1.10.0")] {
        install(&mut reg, &saw_spec(&pkg_dir(&d, tag), v));
    }
    // semver max, not lexical: 1.10.0 > 2.0.0? NO — 2 > 1.10.
    assert_eq!(reg.latest_active("saw-gen").unwrap(), "2.0.0");

    let rows = reg.list();
    assert_eq!(rows.len(), 3);
    assert!(rows.iter().all(|r| r.status == EntryStatus::Active));

    // duplicate version rejected
    let e = reg
        .install(&saw_spec(&pkg_dir(&d, "v1dup"), "1.0.0"))
        .unwrap_err();
    assert_eq!(err_name(&e), "DuplicateVersion");

    // Files on disk verified on load
    let spec = reg.load_spec("saw-gen", "1.0.0", &policy()).unwrap();
    assert_eq!(spec.manifest.version, "1.0.0");
}

#[test]
fn registry_tamper_detection_on_disk() {
    let (_t, d) = tdir("pkgs");
    let (_t2, regdir) = tdir("reg");
    let mut reg = ModuleRegistry::open(&regdir).unwrap();
    install(&mut reg, &saw_spec(&pkg_dir(&d, "v1"), "1.0.0"));

    // Tamper the stored wasm.
    let wasm_path = regdir.join("modules/saw-gen/1.0.0/module.wasm");
    let mut bytes = fs::read(&wasm_path).unwrap();
    bytes[10] ^= 0xFF;
    fs::write(&wasm_path, &bytes).unwrap();
    let e = reg.load_spec("saw-gen", "1.0.0", &policy()).unwrap_err();
    assert_eq!(err_name(&e), "Tampered");
    assert!(e.to_string().contains("wasm"));

    // Restore, then tamper the manifest instead.
    let spec = saw_spec(&pkg_dir(&d, "v1b"), "1.0.0");
    fs::write(&wasm_path, &spec.wasm_bytes).unwrap();
    let man_path = regdir.join("modules/saw-gen/1.0.0/manifest.json");
    fs::write(&man_path, b"{\"manifest_version\":1}").unwrap();
    let e = reg.load_spec("saw-gen", "1.0.0", &policy()).unwrap_err();
    assert_eq!(err_name(&e), "Tampered");
    assert!(e.to_string().contains("manifest"));

    // Tamper the index-recorded hash.
    let idx_path = regdir.join("index.json");
    let idx = fs::read_to_string(&idx_path).unwrap();
    let good = sha256_hex(&spec.wasm_bytes);
    fs::write(&idx_path, idx.replace(&good, &"f".repeat(64))).unwrap();
    let e = reg.load_spec("saw-gen", "1.0.0", &policy()).unwrap_err();
    assert_eq!(err_name(&e), "Tampered");
}

#[test]
fn registry_reopen_and_full_verify() {
    let (_t, d) = tdir("pkgs");
    let (_t2, regdir) = tdir("reg");
    {
        let mut reg = ModuleRegistry::open(&regdir).unwrap();
        install(&mut reg, &saw_spec(&pkg_dir(&d, "v1"), "1.0.0"));
        install(&mut reg, &saw_spec(&pkg_dir(&d, "v2"), "2.0.0"));
        reg.revoke("saw-gen", Some("1.0.0"), "superseded").unwrap();
    }
    // Reopen from disk: index + files load, revoke persisted.
    let reg = ModuleRegistry::open(&regdir).unwrap();
    let rows = reg.list();
    assert_eq!(rows.len(), 2);
    let e = reg.load_spec("saw-gen", "1.0.0", &policy()).unwrap_err();
    assert_eq!(err_name(&e), "Revoked");
    assert_eq!(reg.latest_active("saw-gen").unwrap(), "2.0.0");
    // verify_all checks every ACTIVE entry's bytes (revoked files are
    // kept for audit but no longer hash-checked on load).
    let ok = reg.verify_all(&policy()).unwrap();
    assert_eq!(ok.len(), 1);
    // and catches a post-hoc corruption
    let wasm_path = regdir.join("modules/saw-gen/2.0.0/module.wasm");
    let mut bytes = fs::read(&wasm_path).unwrap();
    bytes[9] ^= 0xFF;
    fs::write(&wasm_path, &bytes).unwrap();
    let e = reg.verify_all(&policy()).unwrap_err();
    assert_eq!(err_name(&e), "Tampered");
}

// ---------- capability → export wiring ----------

#[test]
fn declared_capability_requires_the_export() {
    // saw-gen wasm has params/state/render-off but NO midi export —
    // declaring midi-transform must fail at instantiate with the
    // capability-check error, not a generic linker miss.
    let (_t, d) = tdir("pkgs");
    let wasm = wasm_of(saw_v1_wat());
    let mj = manifest_json(
        "saw-gen",
        "Saw",
        "1.0.0",
        &wasm,
        &["params", "midi-transform"],
        "midi_transform",
        STANDARD_LIMITS,
        &[],
    );
    let dir = pkg_dir(&d, "noexport");
    write_package(&dir, &mj, &wasm);
    let spec = ModuleSpec::from_dir(&dir, &policy()).unwrap();
    let host = Host::new(policy()).unwrap();
    let e = match host.instantiate(&spec) {
        Err(e) => e,
        Ok(_) => panic!("instantiate must fail"),
    };
    assert_eq!(err_name(&e), "MissingCapabilityExport");
}

#[test]
fn no_fx_and_generator_capabilities_parse() {
    let wasm = wasm_of(saw_v1_wat());
    let mj = manifest_json(
        "saw-gen",
        "Saw",
        "1.0.0",
        &wasm,
        &["params", "no-fx", "generator"],
        "param_surface",
        STANDARD_LIMITS,
        &[],
    );
    let m = parse_manifest(&mj).unwrap();
    assert!(m.has_capability(Capability::NoFx));
    assert!(m.has_capability(Capability::Generator));
}

// ---------- SDK / rights gates (T98 honesty) ----------

#[test]
fn ara_and_plugin_export_are_gated_without_evidence() {
    let reg = gate_register();
    let ara = reg
        .iter()
        .find(|r| r.integration == GatedIntegration::Ara)
        .unwrap();
    let plug = reg
        .iter()
        .find(|r| r.integration == GatedIntegration::PluginExport)
        .unwrap();
    match &ara.status {
        GateStatus::Gated { reason } => {
            assert!(
                reason.contains("licence") || reason.contains("rights"),
                "ARA gate reason: {reason}"
            );
        }
        _ => panic!("ARA must start gated"),
    }
    match &plug.status {
        GateStatus::Gated { reason } => {
            assert!(
                reason.contains("rights") || reason.contains("licence"),
                "plugin-export gate reason: {reason}"
            );
        }
        _ => panic!("plugin export must start gated"),
    }
    // Calling a gated integration fails — honestly, not with a stub.
    let e = require_open(ara).unwrap_err();
    assert_eq!(err_name(&e), "IntegrationGated");
}

#[test]
fn gates_open_only_with_evidence() {
    // No evidence → refuses.
    let e = try_qualify(GatedIntegration::Ara, "", "", "").unwrap_err();
    assert_eq!(err_name(&e), "IntegrationGated");
    // With sdk + rights + commit evidence → qualified record.
    let rec = try_qualify(
        GatedIntegration::Ara,
        "ARA-2.0.0",
        "licensing/ara-eval-2026Q4.pdf",
        "deadbeef",
    )
    .unwrap();
    match &rec.status {
        GateStatus::Qualified { sdk, .. } => assert_eq!(sdk, "ARA-2.0.0"),
        _ => panic!("ARA should be qualified now"),
    }
    require_open(&rec).unwrap();
    // Other integrations' records stay independently gated.
    let reg = gate_register();
    let plug = reg
        .iter()
        .find(|r| r.integration == GatedIntegration::PluginExport)
        .unwrap();
    let e = require_open(plug).unwrap_err();
    assert_eq!(err_name(&e), "IntegrationGated");
}
