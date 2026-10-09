//! T97 — WASM capability limits (Linux-verifiable half).
//!
//! Load malicious / infinite-loop / oversized-memory modules, request
//! ambient powers, and hot reload during activity → fuel/memory/time
//! limits hold, ambient access is denied, last-known-good state stays
//! recoverable, and nothing runs unqualified in real time.

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

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

// ---------- good-module round trips ----------

#[test]
fn good_module_instantiates_and_params_round_trip() {
    let (_t, d) = tdir("pkgs");
    let spec = make_spec(
        &pkg_dir(&d, "saw-v1"),
        "saw-gen",
        "Saw Gen",
        "1.0.0",
        saw_v1_wat(),
        &["params", "state", "render-off"],
        "offline_render",
        STANDARD_LIMITS,
    );
    let host = Host::new(policy()).unwrap();
    let mut inst = host.instantiate(&spec).expect("instantiate");

    // ABI + init checked at instantiate; params exposed per manifest.
    assert_eq!(inst.param_count().unwrap(), 2);
    assert_eq!(inst.param_get(0).unwrap(), 440.0);
    inst.param_set(0, 220.0).unwrap();
    inst.param_set(1, 0.25).unwrap();
    assert_eq!(inst.param_get(0).unwrap(), 220.0);
    assert_eq!(inst.param_get(1).unwrap(), 0.25);

    // Undeclared-capability calls fail loudly.
    let e = inst.midi_xform(&[]).unwrap_err();
    assert_eq!(err_name(&e), "CapabilityNotDeclared");
}

#[test]
fn render_off_produces_real_audio() {
    let (_t, d) = tdir("pkgs");
    let spec = make_spec(
        &pkg_dir(&d, "saw"),
        "saw-gen",
        "Saw",
        "1.0.0",
        saw_v1_wat(),
        &["params", "state", "render-off"],
        "offline_render",
        STANDARD_LIMITS,
    );
    let host = Host::new(policy()).unwrap();
    let mut inst = host.instantiate(&spec).unwrap();

    let out = inst.render_off(8).unwrap();
    assert_eq!(out.len(), 8);
    // Saw at amp 0.5 starting phase 0: first sample -0.5, ramping up.
    assert!((out[0] - (-0.5f32)).abs() < 1e-6);
    assert!(out[1] > out[0]);
    assert!(out.iter().all(|s| s.abs() <= 0.5 + 1e-6));
}

#[test]
fn placement_is_a_schema_level_guard() {
    // Placement comes from the manifest — there is no audio-path value
    // a module can write. What IS representable is checked at load.
    let (_t, d) = tdir("pkgs");
    let wasm = wasm_of(saw_v1_wat());
    // render-off module forced onto the param surface → refused.
    let mj = manifest_json(
        "saw-gen",
        "Saw",
        "1.0.0",
        &wasm,
        &["params", "state", "render-off"],
        "param_surface",
        STANDARD_LIMITS,
        &[],
    );
    let dir = pkg_dir(&d, "badplace");
    write_package(&dir, &mj, &wasm);
    let spec = ModuleSpec::from_dir(&dir, &policy()).unwrap();
    let host = Host::new(policy()).unwrap();
    let e = match host.instantiate(&spec) {
        Err(e) => e,
        Ok(_) => panic!("instantiate must fail"),
    };
    assert_eq!(err_name(&e), "InvalidManifest");
    assert!(e.to_string().contains("placement"));
}

// ---------- kills ----------

#[test]
fn infinite_loop_module_dies_on_fuel() {
    let (_t, d) = tdir("pkgs");
    let spec = make_spec(
        &pkg_dir(&d, "spin"),
        "spinner",
        "Spinner",
        "1.0.0",
        spinner_wat(),
        &["render-off"],
        "offline_render",
        &[
            ("max_memory_bytes", "16777216"),
            ("max_fuel", "100000"),
            ("deadline_ms", "5000"),
            ("max_state_bytes", "1024"),
            ("max_out_bytes", "4194304"),
            ("max_params", "8"),
        ],
    );
    let host = Host::new(policy()).unwrap();
    let mut inst = host.instantiate(&spec).unwrap();
    let e = inst.render_off(64).unwrap_err();
    assert_eq!(err_name(&e), "FuelExhausted");
}

#[test]
fn infinite_loop_module_dies_on_epoch_deadline() {
    let (_t, d) = tdir("pkgs");
    let spec = make_spec(
        &pkg_dir(&d, "spin"),
        "spinner",
        "Spinner",
        "1.0.0",
        spinner_wat(),
        &["render-off"],
        "offline_render",
        &[
            ("max_memory_bytes", "16777216"),
            // JIT burns 50M fuel in tens of ms; a 5 ms deadline must
            // win anyway — time-kill is independent of fuel.
            ("max_fuel", "50000000"),
            ("deadline_ms", "5"),
            ("max_state_bytes", "1024"),
            ("max_out_bytes", "4194304"),
            ("max_params", "8"),
        ],
    );
    let host = Host::new(policy()).unwrap();
    let mut inst = host.instantiate(&spec).unwrap();
    let started = Instant::now();
    let e = inst.render_off(64).unwrap_err();
    let elapsed = started.elapsed();
    assert_eq!(err_name(&e), "DeadlineExceeded");
    assert!(elapsed < Duration::from_secs(2), "kill took {elapsed:?}");
}

#[test]
fn cancel_flag_kills_guest_calls() {
    let (_t, d) = tdir("pkgs");
    let spec = make_spec(
        &pkg_dir(&d, "spin"),
        "spinner",
        "Spinner",
        "1.0.0",
        spinner_wat(),
        &["render-off"],
        "offline_render",
        &[
            ("max_memory_bytes", "16777216"),
            ("max_fuel", "50000000"),
            ("deadline_ms", "5000"),
            ("max_state_bytes", "1024"),
            ("max_out_bytes", "4194304"),
            ("max_params", "8"),
        ],
    );
    let host = Host::new(policy()).unwrap();
    let mut inst = host.instantiate(&spec).unwrap();

    // Pre-armed flag: the very first epoch tick refuses the call.
    inst.cancel_token().cancel();
    let e = inst.render_off(64).unwrap_err();
    assert_eq!(err_name(&e), "Cancelled");

    // Mid-flight cancel from another thread: the flag is checked at
    // every ~1 ms tick, far inside the fuel budget's lifetime.
    let mut inst = host.instantiate(&spec).unwrap();
    let token = inst.cancel_token();
    let done = Arc::new(AtomicBool::new(false));
    let done2 = done.clone();
    let h = thread::spawn(move || {
        let r = inst.render_off(64);
        done2.store(true, Ordering::SeqCst);
        (inst, r)
    });
    thread::sleep(Duration::from_millis(10));
    assert!(!done.load(Ordering::SeqCst), "call finished before cancel");
    token.cancel();
    let (inst, r) = h.join().expect("worker panicked");
    assert_eq!(err_name(&r.unwrap_err()), "Cancelled");
    assert_eq!(inst.manifest().module_id, "spinner");
}

// ---------- memory limits ----------

#[test]
fn oversized_module_rejected_before_instantiate() {
    let (_t, d) = tdir("pkgs");
    let wasm = wasm_of(fatty_wat()); // declares 512 pages = 32 MiB minimum
                                     // Manifest honestly declares a 16 MiB budget → static scan sees the
                                     // declared wasm minimum exceeds the manifest ceiling and refuses
                                     // BEFORE wasmtime ever instantiates it.
    let mj = manifest_json(
        "fatty",
        "Fatty",
        "1.0.0",
        &wasm,
        &["render-off"],
        "offline_render",
        STANDARD_LIMITS,
        &[],
    );
    let dir = pkg_dir(&d, "fatty");
    write_package(&dir, &mj, &wasm);
    let e = ModuleSpec::from_dir(&dir, &policy()).unwrap_err();
    assert_eq!(err_name(&e), "MemoryLimitExceeded");
}

#[test]
fn runtime_memory_growth_hits_the_limiter() {
    let (_t, d) = tdir("pkgs");
    let spec = make_spec(
        &pkg_dir(&d, "hog"),
        "hog",
        "Hog",
        "1.0.0",
        hog_wat(),
        &["render-off"],
        "offline_render",
        &[
            ("max_memory_bytes", "2097152"), // 2 MiB ceiling
            ("max_fuel", "50000000"),
            ("deadline_ms", "5000"),
            ("max_state_bytes", "1024"),
            ("max_out_bytes", "1048576"),
            ("max_params", "8"),
        ],
    );
    let host = Host::new(policy()).unwrap();
    let mut inst = host.instantiate(&spec).unwrap();
    let e = inst.render_off(1).unwrap_err();
    assert_eq!(err_name(&e), "MemoryLimitExceeded");
}

// ---------- ambient denial ----------

#[test]
fn ambient_request_in_manifest_is_never_grantable() {
    let wasm = wasm_of(saw_v1_wat());
    for amb in [
        "fs", "net", "env", "wasi", "spawn", "threads", "random", "clock",
    ] {
        let mj = manifest_json(
            "saw-gen",
            "Saw",
            "1.0.0",
            &wasm,
            &["params"],
            "param_surface",
            STANDARD_LIMITS,
            &[amb],
        );
        let e = parse_manifest(&mj).unwrap_err();
        assert_eq!(err_name(&e), "AmbientDenied", "ambient {amb} granted!");
    }
}

#[test]
fn denied_import_fails_to_instantiate_net() {
    let (_t, d) = tdir("pkgs");
    let wasm = wasm_of(sneak_net_wat());
    let mj = manifest_json(
        "sneak",
        "Sneak",
        "1.0.0",
        &wasm,
        &["params"],
        "param_surface",
        STANDARD_LIMITS,
        &[],
    );
    let dir = pkg_dir(&d, "sneak-net");
    write_package(&dir, &mj, &wasm);
    let e = ModuleSpec::from_dir(&dir, &policy()).unwrap_err();
    assert_eq!(err_name(&e), "ImportDenied");
    assert!(e.to_string().contains("env.http_get"));
}

#[test]
fn denied_import_fails_to_instantiate_wasi() {
    let (_t, d) = tdir("pkgs");
    let wasm = wasm_of(sneak_fs_wat());
    let mj = manifest_json(
        "sneak",
        "Sneak",
        "1.0.0",
        &wasm,
        &["params"],
        "param_surface",
        STANDARD_LIMITS,
        &[],
    );
    let dir = pkg_dir(&d, "sneak-fs");
    write_package(&dir, &mj, &wasm);
    let e = ModuleSpec::from_dir(&dir, &policy()).unwrap_err();
    assert_eq!(err_name(&e), "ImportDenied");
    assert!(e.to_string().contains("fd_read"));
}

#[test]
fn native_binary_is_not_a_module() {
    // A module is wasm + manifest. An ELF/.so can NEVER be one.
    let (_t, d) = tdir("pkgs");
    let elf = fs::read("/bin/ls").expect("read an ELF");
    let mj = manifest_json(
        "native",
        "Native",
        "1.0.0",
        &elf,
        &["params"],
        "param_surface",
        STANDARD_LIMITS,
        &[],
    );
    let dir = pkg_dir(&d, "elf");
    write_package(&dir, &mj, &elf);
    let e = ModuleSpec::from_dir(&dir, &policy()).unwrap_err();
    assert_eq!(err_name(&e), "InvalidModulePackage");
}

// ---------- granted surface ----------

#[test]
fn the_one_granted_import_works_and_is_bounded() {
    let (_t, d) = tdir("pkgs");
    let spec = make_spec(
        &pkg_dir(&d, "talker"),
        "talker",
        "Talker",
        "1.0.0",
        talker_wat(),
        &["no-fx"],
        "param_surface",
        STANDARD_LIMITS,
    );
    let host = Host::new(policy()).unwrap();
    let inst = host.instantiate(&spec).unwrap();
    let logs = inst.logs();
    assert!(logs.iter().any(|l| l.text.contains("hello from wasm")));
    assert!(logs.iter().all(|l| l.text.len() <= 256));
}

#[test]
fn missing_required_export_is_abi_violation() {
    let (_t, d) = tdir("pkgs");
    let spec = make_spec(
        &pkg_dir(&d, "spoiled"),
        "spoiled",
        "Spoiled",
        "1.0.0",
        spoiled_wat(),
        &["params"],
        "param_surface",
        STANDARD_LIMITS,
    );
    let host = Host::new(policy()).unwrap();
    let e = match host.instantiate(&spec) {
        Err(e) => e,
        Ok(_) => panic!("instantiate must fail"),
    };
    assert_eq!(err_name(&e), "AbiViolation");
}

#[test]
fn oversized_declared_state_rejected_at_init() {
    let (_t, d) = tdir("pkgs");
    let spec = make_spec(
        &pkg_dir(&d, "hoarder"),
        "hoarder",
        "Hoarder",
        "1.0.0",
        hoarder_wat(),
        &["state"],
        "param_surface",
        &[
            ("max_memory_bytes", "16777216"),
            ("max_fuel", "10000000"),
            ("deadline_ms", "2000"),
            ("max_state_bytes", "1024"), // module claims 2 MiB
            ("max_out_bytes", "1048576"),
            ("max_params", "8"),
        ],
    );
    let host = Host::new(policy()).unwrap();
    let e = match host.instantiate(&spec) {
        Err(e) => e,
        Ok(_) => panic!("instantiate must fail"),
    };
    assert_eq!(err_name(&e), "StateTooLarge");
}

// ---------- hot reload (last-known-good) ----------

#[test]
fn hot_reload_migrates_state_and_swaps_code() {
    let (_t, d) = tdir("pkgs");
    let (_t2, regdir) = tdir("reg");
    let reg = ModuleRegistry::open(&regdir).unwrap();
    let mut rt = ModuleRuntime::new(reg, policy()).unwrap();

    for (tag, v, wat_src) in [("v1", "1.0.0", saw_v1_wat()), ("v2", "2.0.0", saw_v2_wat())] {
        let spec = make_spec(
            &pkg_dir(&d, tag),
            "saw-gen",
            "Saw",
            v,
            &wat_src,
            &["params", "state", "render-off"],
            "offline_render",
            STANDARD_LIMITS,
        );
        rt.registry_mut().install(&spec).unwrap();
    }

    rt.load("saw-gen", Some("1.0.0")).unwrap();
    {
        let live = &mut rt.live_mut("saw-gen").unwrap().instance;
        live.param_set(0, 220.0).unwrap();
        live.param_set(1, 0.25).unwrap();
        live.render_off(16).unwrap(); // phase advances into state
        let st = live.state_save().unwrap();
        assert_eq!(st.len(), 24);
    }

    let rep = rt.hot_reload("saw-gen", None).unwrap();
    assert_eq!(rep.from_version, "1.0.0");
    assert_eq!(rep.to_version, "2.0.0");
    assert_eq!(rep.state_bytes, Some(24));

    // v2 got the migrated params + phase; render is now SQUARE not saw.
    let live = rt.live_mut("saw-gen").unwrap();
    assert_eq!(live.version, "2.0.0");
    assert_eq!(live.instance.param_get(0).unwrap(), 220.0);
    assert_eq!(live.instance.param_get(1).unwrap(), 0.25);
    let out = live.instance.render_off(4).unwrap();
    assert!(
        out.iter().all(|s| s.abs() == 0.25f32),
        "square @ amp .25: {out:?}"
    );
}

#[test]
fn hot_reload_failure_keeps_last_known_good() {
    let (_t, d) = tdir("pkgs");
    let (_t2, regdir) = tdir("reg");
    let reg = ModuleRegistry::open(&regdir).unwrap();
    let mut rt = ModuleRuntime::new(reg, policy()).unwrap();

    for (tag, v, wat_src) in [
        ("v1", "1.0.0", saw_v1_wat()),
        ("v3", "3.0.0", saw_v3_wat()), // state_restore always rejects
    ] {
        let spec = make_spec(
            &pkg_dir(&d, tag),
            "saw-gen",
            "Saw",
            v,
            &wat_src,
            &["params", "state", "render-off"],
            "offline_render",
            STANDARD_LIMITS,
        );
        rt.registry_mut().install(&spec).unwrap();
    }

    rt.load("saw-gen", Some("1.0.0")).unwrap();
    rt.live_mut("saw-gen")
        .unwrap()
        .instance
        .param_set(0, 111.0)
        .unwrap();

    let e = rt.hot_reload("saw-gen", None).unwrap_err();
    assert_eq!(err_name(&e), "HotReloadFailed");

    // v1 is STILL live with its params — last-known-good preserved.
    let live = rt.live_mut("saw-gen").unwrap();
    assert_eq!(live.version, "1.0.0");
    assert_eq!(live.instance.param_get(0).unwrap(), 111.0);
    let out = live.instance.render_off(4).unwrap();
    assert_eq!(out.len(), 4);
}

#[test]
fn hot_reload_onto_tampered_version_keeps_old_live() {
    let (_t, d) = tdir("pkgs");
    let (_t2, regdir) = tdir("reg");
    let reg = ModuleRegistry::open(&regdir).unwrap();
    let mut rt = ModuleRuntime::new(reg, policy()).unwrap();

    for (tag, v, wat_src) in [("v1", "1.0.0", saw_v1_wat()), ("v2", "2.0.0", saw_v2_wat())] {
        let spec = make_spec(
            &pkg_dir(&d, tag),
            "saw-gen",
            "Saw",
            v,
            &wat_src,
            &["params", "state", "render-off"],
            "offline_render",
            STANDARD_LIMITS,
        );
        rt.registry_mut().install(&spec).unwrap();
    }

    // Corrupt v2's wasm on disk AFTER verified install.
    let wasm_path = regdir.join("modules/saw-gen/2.0.0/module.wasm");
    let mut bytes = fs::read(&wasm_path).unwrap();
    bytes[8] ^= 0xFF;
    fs::write(&wasm_path, bytes).unwrap();

    rt.load("saw-gen", Some("1.0.0")).unwrap();
    let e = rt.hot_reload("saw-gen", None).unwrap_err();
    assert_eq!(err_name(&e), "HotReloadFailed");
    assert!(e.to_string().contains("tamper"), "{e}");
    assert_eq!(rt.live("saw-gen").unwrap().version, "1.0.0");
}

#[test]
fn revoke_unloads_live_and_blocks_reload() {
    let (_t, d) = tdir("pkgs");
    let (_t2, regdir) = tdir("reg");
    let reg = ModuleRegistry::open(&regdir).unwrap();
    let mut rt = ModuleRuntime::new(reg, policy()).unwrap();

    for (tag, v, wat_src) in [("v1", "1.0.0", saw_v1_wat()), ("v2", "2.0.0", saw_v2_wat())] {
        let spec = make_spec(
            &pkg_dir(&d, tag),
            "saw-gen",
            "Saw",
            v,
            &wat_src,
            &["params", "state", "render-off"],
            "offline_render",
            STANDARD_LIMITS,
        );
        rt.registry_mut().install(&spec).unwrap();
    }

    rt.load("saw-gen", Some("1.0.0")).unwrap();
    // Revoke v2 — hot_reload sees only the live version as active and
    // refuses to move (never re-picks a revoked target).
    rt.registry_mut()
        .revoke("saw-gen", Some("2.0.0"), "cve")
        .unwrap();
    let e = rt.hot_reload("saw-gen", None).unwrap_err();
    assert_eq!(err_name(&e), "HotReloadFailed");

    // Revoke the live v1 — instance unloads immediately.
    rt.revoke("saw-gen", Some("1.0.0"), "recalled").unwrap();
    assert!(rt.live("saw-gen").is_none());
    let e = rt.load("saw-gen", Some("1.0.0")).unwrap_err();
    assert_eq!(err_name(&e), "Revoked");
}

#[test]
fn midi_transform_runs_real_event_math() {
    let (_t, d) = tdir("pkgs");
    let spec = make_spec(
        &pkg_dir(&d, "arp"),
        "arp",
        "Arp",
        "1.0.0",
        arp_wat(),
        &["midi-transform"],
        "midi_transform",
        STANDARD_LIMITS,
    );
    let host = Host::new(policy()).unwrap();
    let mut inst = host.instantiate(&spec).unwrap();

    let input = vec![
        NoteEvent {
            onset_ticks: 0,
            length_ticks: 96000,
            pitch: 60,
            velocity: 100,
            channel: 0,
            kind: 1,
        },
        NoteEvent {
            onset_ticks: 192000,
            length_ticks: 96000,
            pitch: 64,
            velocity: 90,
            channel: 0,
            kind: 1,
        },
    ];
    let out = inst.midi_xform(&input).unwrap();
    assert_eq!(out.len(), 4);
    // Original then +12 clone at +96000 ticks, per input event.
    assert_eq!(out[0].pitch, 60);
    assert_eq!(out[1].pitch, 72);
    assert_eq!(out[1].onset_ticks, 96000);
    assert_eq!(out[2].pitch, 64);
    assert_eq!(out[3].pitch, 76);
    assert_eq!(out[3].onset_ticks, 288000);
}

#[test]
fn malformed_transform_output_is_rejected() {
    let (_t, d) = tdir("pkgs");
    let spec = make_spec(
        &pkg_dir(&d, "evil"),
        "evil",
        "Evil",
        "1.0.0",
        evil_xform_wat(),
        &["midi-transform"],
        "midi_transform",
        STANDARD_LIMITS,
    );
    let host = Host::new(policy()).unwrap();
    let mut inst = host.instantiate(&spec).unwrap();
    let input = vec![NoteEvent {
        onset_ticks: 0,
        length_ticks: 1,
        pitch: 60,
        velocity: 1,
        channel: 0,
        kind: 1,
    }];
    let e = inst.midi_xform(&input).unwrap_err();
    assert_eq!(err_name(&e), "AbiViolation");
}

#[test]
fn non_finite_param_is_rejected() {
    let (_t, d) = tdir("pkgs");
    let spec = make_spec(
        &pkg_dir(&d, "saw"),
        "saw-gen",
        "Saw",
        "1.0.0",
        saw_v1_wat(),
        &["params"],
        "param_surface",
        STANDARD_LIMITS,
    );
    let host = Host::new(policy()).unwrap();
    let mut inst = host.instantiate(&spec).unwrap();
    assert!(inst.param_set(0, f64::NAN).is_err());
    assert!(inst.param_set(0, f64::INFINITY).is_err());
    assert_eq!(inst.param_get(0).unwrap(), 440.0);
}
