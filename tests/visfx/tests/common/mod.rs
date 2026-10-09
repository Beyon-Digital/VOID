//! Shared fixtures for the visfx detached suite.
#![allow(dead_code)]

use void_assets::AssetStore;
use void_jobs::{JobBudget, JobDb, JobRunner, RunContext, WorkerRuntime};
use void_visfx::VisualGenService;
use std::path::PathBuf;

pub struct Fixture {
    pub root: tempfile::TempDir,
    pub db: JobDb,
    pub store: AssetStore,
    pub svc: VisualGenService,
}

pub fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let db = JobDb::open(root.path().join("index.db")).unwrap();
    let store = AssetStore::new(root.path().join("assets"), 4 * 1024 * 1024 * 1024).unwrap();
    let svc = VisualGenService::new(store.clone(), root.path()).unwrap();
    Fixture {
        root,
        db,
        store,
        svc,
    }
}

pub fn runner(f: &Fixture) -> JobRunner {
    let db = JobDb::open(f.root.path().join("index.db")).unwrap();
    JobRunner::new(db, f.store.clone(), f.root.path().to_path_buf())
}

pub fn worker() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_visfx_fake_worker"))
}

pub fn runtime() -> WorkerRuntime {
    WorkerRuntime::new(worker())
}

pub fn ctx<'a>(rt: &'a WorkerRuntime, b: JobBudget) -> RunContext<'a> {
    RunContext {
        runtime: rt,
        budget: b,
        model_version: Some("1.0.0".into()),
    }
}

pub fn budget(wall_ns: u64) -> JobBudget {
    JobBudget {
        cpu_seconds: 30,
        memory_bytes: 4 * 1024 * 1024 * 1024,
        vram_bytes: 0,
        deadline_monotonic_ns: 0,
        wall_ns,
    }
}

/// Deterministic mono PCM fixture: 440Hz sine at `amp` for `n` samples
/// at 48k plus a hard impulse (onset) at `impulse_at`.
pub fn pcm_fixture(n: usize, impulse_at: usize, amp: f32) -> Vec<f32> {
    let mut v = Vec::with_capacity(n);
    for i in 0..n {
        let t = i as f32 / 48_000.0;
        v.push(amp * (2.0 * std::f32::consts::PI * 440.0 * t).sin());
    }
    if impulse_at < n {
        v[impulse_at] = 0.99;
    }
    v
}

/// Valid WGSL generator body (template contract: fn gen(uv,u)->vec4).
pub const GOOD_BODY: &str = r#"
fn gen(uv: vec2<f32>, u: GenU) -> vec4<f32> {
    let t = u.tick * 0.001 + u.seed * 0.01;
    let r = 0.5 + 0.5 * sin(uv.x * 6.0 + t);
    let g = 0.5 + 0.5 * sin(uv.y * 6.0 + t * 1.3);
    return vec4<f32>(r, g, u.rms, 1.0);
}
"#;

/// Malformed WGSL — parse must fail.
pub const BAD_BODY: &str = "fn gen(uv: vec2<f32>, u: GenU) -> vec4<f32> { let x = ; }";

/// Forbidden builtin — the isolated-renderer contract bans storage
/// writes; compiles fine syntactically but must fail forbidden_check.
pub const FORBIDDEN_BODY: &str = r#"
fn gen(uv: vec2<f32>, u: GenU) -> vec4<f32> {
    workgroupBarrier();
    return vec4<f32>(uv.x, uv.y, 0.0, 1.0);
}
"#;

/// Over-budget: an unbounded loop{} — literally no reachable break.
pub const UNBOUNDED_BODY: &str = r#"
fn gen(uv: vec2<f32>, u: GenU) -> vec4<f32> {
    var c = 0.0;
    loop {
        c += sin(uv.x * 10.0);
    }
    return vec4<f32>(uv.x, uv.y, c, 1.0);
}
"#;

/// Over-budget: bounded loop with a provable trip count far over the
/// sample-iteration budget (texture-free but instruction-heavy —
/// the cost model still sees the flattened instruction count).
pub const EXPENSIVE_BODY: &str = r#"
fn gen(uv: vec2<f32>, u: GenU) -> vec4<f32> {
    var acc = vec3<f32>(0.0);
    for (var i = 0; i < 100000; i = i + 1) {
        acc += vec3<f32>(sin(uv.x + f32(i)), sin(uv.y + f32(i)), sin(f32(i)));
    }
    return vec4<f32>(acc, 1.0);
}
"#;
