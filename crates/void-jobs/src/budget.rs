//! Per-job resource budgets (W12, T50; CONTRACTS.md §2/§6).
//!
//! Three enforcement axes, honestly separated:
//! - `cpu_seconds` — kernel-enforced CPU cap (RLIMIT_CPU, Linux).
//! - `memory_bytes` — kernel-enforced address-space cap (RLIMIT_AS).
//! - `deadline_monotonic_ns` — absolute monotonic clock bound; the
//!   runner kills the worker when it trips (spawn-independent).
//! - `wall_ns` — relative-to-spawn bound (model `wallNs` default).
//!
//! Rlimits are applied via `pre_exec` before the worker image loads —
//! a child can never run above its budget. Linux-only for now;
//! elsewhere the runner still enforces deadline/cancel by kill.

use crate::job::JobSpec;

/// Effective budget for one run. `0` = unenforced axis (never emitted
/// for cpu/memory by `from_spec` + model defaults — a qualified model
/// always carries real caps; an empty reservation leaves the model's).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JobBudget {
    pub cpu_seconds: u64,
    pub memory_bytes: u64,
    pub vram_bytes: u64,
    /// Absolute monotonic deadline (spec field; 0 = none).
    pub deadline_monotonic_ns: u64,
    /// Relative wall budget from spawn (0 = none).
    pub wall_ns: u64,
}

fn u64_str(s: &str) -> u64 {
    s.trim().parse().unwrap_or(0)
}

impl JobBudget {
    /// Budget carried by the job spec itself: memory = the reservation
    /// (the cap is what was reserved), deadline = the absolute bound.
    /// cpu/wall come from the qualified model defaults — jobs cannot
    /// raise them via parameters (T50 admission is manifest-driven).
    pub fn from_spec(spec: &JobSpec) -> Self {
        Self {
            cpu_seconds: 0,
            memory_bytes: u64_str(&spec.reservations.ram_bytes),
            vram_bytes: u64_str(&spec.reservations.vram_bytes),
            deadline_monotonic_ns: u64_str(&spec.deadline_monotonic_ns),
            wall_ns: 0,
        }
    }

    /// Clamp into a model's qualified ceilings: the tighter bound wins.
    /// `memory_bytes` takes the reservation when present, else the
    /// model cap — then clamps to the cap. cpu/wall always take the
    /// model's qualified values.
    pub fn clamped_to(mut self, cpu_seconds: u64, memory_cap: u64, wall_ns: u64) -> Self {
        self.cpu_seconds = cpu_seconds;
        self.wall_ns = wall_ns;
        self.memory_bytes = match (self.memory_bytes, memory_cap) {
            (0, cap) => cap,
            (res, 0) => res,
            (res, cap) => res.min(cap),
        };
        self
    }

    /// Absolute monotonic deadline for this run — earliest of the
    /// spec's absolute deadline and spawn + wall_ns. `None` when
    /// neither applies.
    pub fn effective_deadline(&self, spawn_monotonic_ns: u64) -> Option<u64> {
        let abs = (self.deadline_monotonic_ns > 0).then_some(self.deadline_monotonic_ns);
        let rel = (self.wall_ns > 0).then(|| spawn_monotonic_ns.saturating_add(self.wall_ns));
        match (abs, rel) {
            (Some(a), Some(r)) => Some(a.min(r)),
            (Some(a), None) => Some(a),
            (None, Some(r)) => Some(r),
            (None, None) => None,
        }
    }
}

/// CLOCK_MONOTONIC nanoseconds — same clock the protocol's
/// `deadlineMonotonicNs` refers to. `0` off unix (tests run Linux).
#[cfg(unix)]
pub fn monotonic_ns() -> u64 {
    #[repr(C)]
    struct Ts {
        sec: i64,
        nsec: i64,
    }
    extern "C" {
        fn clock_gettime(clk: i32, ts: *mut Ts) -> i32;
    }
    let mut t = Ts { sec: 0, nsec: 0 };
    // 1 = CLOCK_MONOTONIC on Linux.
    let rc = unsafe { clock_gettime(1, &mut t) };
    if rc != 0 || t.sec < 0 {
        return 0;
    }
    t.sec as u64 * 1_000_000_000 + t.nsec.max(0) as u64
}

#[cfg(not(unix))]
pub fn monotonic_ns() -> u64 {
    0
}

/// Apply cpu/memory rlimits to a Command pre-spawn (Linux).
/// `pre_exec` runs in the child between fork and exec — the worker
/// never executes a single instruction above budget.
#[cfg(target_os = "linux")]
pub fn apply_rlimits(cmd: &mut std::process::Command, budget: &JobBudget) {
    use std::os::unix::process::CommandExt;
    #[repr(C)]
    struct Rlim {
        cur: u64,
        max: u64,
    }
    extern "C" {
        fn setrlimit(resource: i32, rlim: *const Rlim) -> i32;
    }
    const RLIMIT_CPU: i32 = 0;
    const RLIMIT_AS: i32 = 9;
    let cpu = budget.cpu_seconds;
    let mem = budget.memory_bytes;
    unsafe {
        cmd.pre_exec(move || {
            if cpu > 0 {
                let r = Rlim { cur: cpu, max: cpu };
                setrlimit(RLIMIT_CPU, &r);
            }
            if mem > 0 {
                let r = Rlim { cur: mem, max: mem };
                setrlimit(RLIMIT_AS, &r);
            }
            Ok(())
        });
    }
}

#[cfg(not(target_os = "linux"))]
pub fn apply_rlimits(_cmd: &mut std::process::Command, _budget: &JobBudget) {
    // Deadline/cancel still enforced by the runner's kill path.
}
