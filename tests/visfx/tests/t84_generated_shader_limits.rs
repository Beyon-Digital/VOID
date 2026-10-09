//! T84 — Generated shader limits: naga accepts a good shader, rejects
//! malformed, forbidden-builtin and over-budget bodies; watchdog/
//! cancel policy model; known-good fallback on compile failure;
//! compile cache; audio path never blocked (failure returns reports,
//! never panics).

mod common;
use common::*;
use void_visfx::{
    KernelOutcome, PresetRef, PresetStatus, RejectReason, ShaderRegistry, ValidationStage,
    WatchdogPolicy,
};

fn registry() -> (tempfile::TempDir, ShaderRegistry) {
    let dir = tempfile::tempdir().unwrap();
    let reg = ShaderRegistry::open(dir.path()).unwrap();
    (dir, reg)
}

#[test]
fn naga_accepts_good_shader() {
    let (_dir, mut reg) = registry();
    let rep = reg.compile_body("t-good", GOOD_BODY);
    assert!(rep.ok(), "good shader rejected: {rep:?}");
    assert_eq!(rep.stage, ValidationStage::Budget);
    assert!(rep.passed_all_previous);
    let cost = rep.cost.expect("cost measured");
    assert!(cost.instructions > 0);
    assert!(!cost.has_unbounded_loop);
    // builtins themselves validate through the same pipeline
    for name in void_visfx::shader::BUILTIN_PRESETS {
        let r = reg.resolve(&PresetRef::Builtin {
            name: (*name).into(),
        });
        assert!(r.is_ok(), "builtin {name} failed: {:?}", r.err());
        assert!(!r.unwrap().fell_back);
    }
}

#[test]
fn naga_rejects_malformed() {
    let (_dir, mut reg) = registry();
    let rep = reg.compile_body("t-bad", BAD_BODY);
    assert!(!rep.ok());
    assert_eq!(rep.stage, ValidationStage::Parse);
    assert_eq!(rep.rejection, Some(RejectReason::InvalidSyntax));
}

#[test]
fn registry_rejects_forbidden_builtins() {
    let (_dir, mut reg) = registry();
    let rep = reg.compile_body("t-forbidden", FORBIDDEN_BODY);
    assert!(!rep.ok());
    assert_eq!(rep.stage, ValidationStage::ForbiddenCheck);
    assert_eq!(rep.rejection, Some(RejectReason::Forbidden));
}

#[test]
fn registry_rejects_over_budget() {
    let (_dir, mut reg) = registry();

    // unbounded loop{} — no break_if
    let rep = reg.compile_body("t-loop", UNBOUNDED_BODY);
    assert!(!rep.ok());
    assert_eq!(rep.stage, ValidationStage::Budget);
    assert_eq!(rep.rejection, Some(RejectReason::OverBudget));
    assert!(rep.cost.unwrap().has_unbounded_loop);

    // provable trip count far over instruction budget
    let rep = reg.compile_body("t-expensive", EXPENSIVE_BODY);
    assert!(!rep.ok());
    assert_eq!(rep.stage, ValidationStage::Budget);
    assert_eq!(rep.rejection, Some(RejectReason::OverBudget));
    assert!(rep.cost.unwrap().instructions > reg.budget.max_instructions);
}

#[test]
fn fallback_resolves_known_good_on_compile_failure() {
    let (_dir, mut reg) = registry();
    // requested preset fails → policy fallback to a builtin
    let mut r = reg.resolve_or_fallback(&PresetRef::Generated {
        id: uuid::Uuid::new_v4().to_string(),
    });
    assert!(r.fell_back);
    assert_eq!(r.id, "black"); // DEFAULT_FALLBACK
    assert!(!r.wgsl.is_empty());
    assert!(!r.chain.is_empty(), "failure chain must carry the report");

    // monadic chain: bad body → builtin plasma
    let bad_id = uuid::Uuid::new_v4().to_string();
    let chain = reg
        .chain(PresetRef::Generated { id: bad_id })
        .or(PresetRef::Builtin {
            name: "plasma".into(),
        })
        .resolve();
    r = chain.expect("chain must resolve via fallback");
    assert_eq!(r.id, "plasma");
    assert!(r.fell_back);
    assert_eq!(r.chain.len(), 1);
    assert_eq!(r.chain[0].rejection, Some(RejectReason::InvalidLayout));

    // a good generated body resolves directly, no fallback
    let good_id = uuid::Uuid::new_v4().to_string();
    let rec = reg
        .register_generated(&good_id, GOOD_BODY, None, Some(42))
        .expect("good body registers");
    assert_eq!(rec.status, PresetStatus::Active);
    let r = reg.resolve_or_fallback(&PresetRef::Generated { id: good_id });
    assert!(!r.fell_back);
    assert!(r.chain.is_empty());
    assert!(r.wgsl.contains("fn gen("));
}

#[test]
fn rejected_body_cannot_register() {
    let (_dir, mut reg) = registry();
    let id = uuid::Uuid::new_v4().to_string();
    assert!(reg.register_generated(&id, BAD_BODY, None, None).is_err());
    assert!(reg.get(&id).is_none());
}

#[test]
fn compile_cache_reuses_results() {
    let (_dir, mut reg) = registry();
    let r1 = reg.compile_body("same", GOOD_BODY);
    let r2 = reg.compile_body("other-alias", GOOD_BODY);
    assert_eq!(r1.composed_sha256, r2.composed_sha256);
    assert_eq!(r1.rejection, r2.rejection);
    // invalidating a preset clears its cache entry but builtins
    // recompile transparently on next resolve.
    reg.invalidate("plasma").unwrap();
    let r = reg.resolve(&PresetRef::Builtin {
        name: "plasma".into(),
    });
    assert!(r.is_ok());
}

#[test]
fn watchdog_policy_model_and_kernel_outcomes() {
    let w = WatchdogPolicy::default();
    w.validate().unwrap();
    let res = w.reservations();
    assert!(res.ram_bytes.parse::<u64>().unwrap() >= 8 * 1024 * 1024);
    // honest: no fake VRAM cap
    assert_eq!(res.vram_bytes, "0");

    // invalid policy is rejected
    let bad = WatchdogPolicy {
        kill_after_ns: 100,
        heartbeat_timeout_ns: 500,
        ..Default::default()
    };
    assert!(bad.validate().is_err());

    // a timeout kills the preset → quarantined + watchdog report
    let (_dir, mut reg) = registry();
    let id = uuid::Uuid::new_v4().to_string();
    reg.register_generated(&id, GOOD_BODY, None, None).unwrap();
    let rep = reg
        .record_kernel_outcome(&id, KernelOutcome::Timeout { wall_ns: 999 })
        .unwrap();
    assert_eq!(rep.stage, ValidationStage::Watchdog);
    assert_eq!(rep.rejection, Some(RejectReason::WatchdogTimeout));
    assert_eq!(reg.get(&id).unwrap().status, PresetStatus::Quarantined);
    // quarantined resolves only via fallback now
    let r = reg.resolve_or_fallback(&PresetRef::Generated { id: id.clone() });
    assert!(r.fell_back);

    // cancel is recorded as its own rejection kind
    let rep = reg
        .record_kernel_outcome(&id, KernelOutcome::Cancelled)
        .unwrap();
    assert_eq!(rep.rejection, Some(RejectReason::WatchdogCancelled));
}

#[test]
fn shader_pipeline_never_blocks_on_failure() {
    // T84's "never blocks audio": a failed compile returns a report
    // synchronously — no panic, no unbounded work. Malformed +
    // over-budget both complete in bounded wall time.
    let (_dir, mut reg) = registry();
    for body in [BAD_BODY, UNBOUNDED_BODY, EXPENSIVE_BODY, FORBIDDEN_BODY] {
        let rep = reg.compile_body("x", body);
        assert!(!rep.ok());
        assert!(rep.wall_ns < 5_000_000_000, "compile took >5s");
    }
}
