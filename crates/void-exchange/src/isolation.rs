//! Plugin processing-isolation POLICY model (W20 / T75 spec side).
//!
//! This is the descriptor set a native plugin-worker host enforces:
//! bounded preallocated buffers, a processing deadline with a declared
//! missed-deadline action, a bounded restart budget, declared latency, and
//! the fallback applied when a worker dies. It is *not* the isolation
//! itself — CONTRACTS.md §7 is explicit that real fault containment needs
//! native evidence (docs/exchange/NEEDS.md records that gap). What this
//! crate guarantees: policies are validated, platform/format availability
//! is honest, and no policy can claim containment it cannot describe.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IsolationMode {
    /// Plugin runs inside the engine process — its crash stops all audio
    /// (the honest pre-isolation default; W20 §Done-only-when).
    InProcess,
    /// Plugin runs in a dedicated worker process behind the declared
    /// bridge/deadline/restart contract.
    IsolatedProcess,
}

/// Bounded bridge budgets — preallocated native buffers, never grown
/// inside the audio path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeBudget {
    /// Max pending control/event messages across the bridge.
    pub event_queue_capacity: u32,
    /// Audio buffer frames per bridge hop (power of two preferred,
    /// enforced ≤ MAX).
    pub audio_buffer_frames: u32,
    /// Max bridge call depth (nested remote calls) before rejection.
    pub max_bridge_depth: u32,
}

/// What the host does when a worker misses its processing deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MissedDeadlineAction {
    /// Emit silence through the bypass path — track stays alive.
    MuteAndBypass,
    /// Replay the last good buffer once, then bypass on further misses.
    KeepLastBuffer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadlinePolicy {
    /// Per-block processing budget in nanoseconds (decimal string on the
    /// wire side; u64 here — serde DTOs stringify at the boundary).
    pub budget_ns: u64,
    /// Consecutive missed budgets before the action fires (1 = immediate).
    pub consecutive_misses_before_action: u32,
    pub on_missed: MissedDeadlineAction,
}

/// Bounded restart attempts — a crash-looping plugin must be stopped, not
/// restarted forever.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestartBudget {
    pub max_restarts: u32,
    /// Window the restart counter applies over (seconds).
    pub window_seconds: u32,
    /// Fixed backoff between restarts (ms).
    pub backoff_ms: u32,
}

/// When the worker is unrecoverable (restart budget exhausted).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WorkerDeathAction {
    /// Bypass the plugin; the rest of the graph keeps running. This is the
    /// T75-required fallback: other tested tracks continue.
    MuteAndBypass,
    /// Drop the track's output to silence (harsher, still bounded).
    Silence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LatencyPolicy {
    /// Latency the isolated path adds (frames). Declared, never hidden —
    /// the host compensates other tracks by this amount when `compensate`.
    pub declared_latency_frames: u64,
    pub compensate: bool,
}

/// The complete policy a native host would enforce. `validate()` is the
/// contract: invalid configurations are rejected, not clamped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IsolationPolicy {
    pub mode: IsolationMode,
    pub bridge: BridgeBudget,
    pub deadline: DeadlinePolicy,
    pub restart: RestartBudget,
    pub latency: LatencyPolicy,
    pub on_worker_death: WorkerDeathAction,
}

/// Typed reason isolation is unavailable — the honest T75 answer instead
/// of a fake "isolated" flag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "detail")]
pub enum IsolationUnavailable {
    /// VOID cannot host this format at all (AAX — Avid-only).
    FormatNotHostable { format: String },
    /// Format exists but not on this platform (AU off macOS).
    PlatformNotSupported { format: String, platform: String },
    /// Engine build lacks a plugin worker — the real T75 blocker on this
    /// Linux lane: `native/void-plugin-worker/` is unbuilt native scope.
    NoIsolatedHost { build: String },
    /// The plugin build itself cannot leave the process (e.g. requires
    /// in-process device/GPU access).
    DeviceBound { detail: String },
}

// Bounds — CONTRACTS §2 "bounded native buffers/deadlines/restarts".
pub const MAX_EVENT_QUEUE_CAPACITY: u32 = 4096;
pub const MAX_AUDIO_BUFFER_FRAMES: u32 = 8192;
pub const MIN_AUDIO_BUFFER_FRAMES: u32 = 32;
pub const MAX_BRIDGE_DEPTH: u32 = 256;
pub const MAX_DEADLINE_BUDGET_NS: u64 = 1_000_000_000; // 1s ceiling
pub const MAX_RESTARTS: u32 = 64;
pub const MAX_RESTART_WINDOW_SECONDS: u32 = 86_400;
pub const MAX_RESTART_BACKOFF_MS: u32 = 60_000;
pub const MAX_LATENCY_FRAMES: u64 = 1 << 22;

impl IsolationPolicy {
    /// The honest default for an engine-hosted plugin today: explicitly
    /// in-process, no claimed containment. Callers must not re-label this
    /// "isolated".
    pub fn in_process() -> Self {
        Self {
            mode: IsolationMode::InProcess,
            bridge: BridgeBudget {
                event_queue_capacity: 0,
                audio_buffer_frames: 0,
                max_bridge_depth: 0,
            },
            deadline: DeadlinePolicy {
                budget_ns: 0,
                consecutive_misses_before_action: 0,
                on_missed: MissedDeadlineAction::MuteAndBypass,
            },
            restart: RestartBudget {
                max_restarts: 0,
                window_seconds: 0,
                backoff_ms: 0,
            },
            latency: LatencyPolicy {
                declared_latency_frames: 0,
                compensate: false,
            },
            on_worker_death: WorkerDeathAction::MuteAndBypass,
        }
    }

    /// The policy the coordinator *requests* when a native isolated host
    /// exists. Conservative defaults; the host may tighten, never loosen.
    pub fn isolated_default() -> Self {
        Self {
            mode: IsolationMode::IsolatedProcess,
            bridge: BridgeBudget {
                event_queue_capacity: 1024,
                audio_buffer_frames: 512,
                max_bridge_depth: 16,
            },
            deadline: DeadlinePolicy {
                // 20ms — comfortably above a 512-frame block at 44.1kHz
                // (~11.6ms); a miss is a defect the fallback absorbs.
                budget_ns: 20_000_000,
                consecutive_misses_before_action: 2,
                on_missed: MissedDeadlineAction::MuteAndBypass,
            },
            restart: RestartBudget {
                max_restarts: 3,
                window_seconds: 60,
                backoff_ms: 250,
            },
            latency: LatencyPolicy {
                declared_latency_frames: 512,
                compensate: true,
            },
            on_worker_death: WorkerDeathAction::MuteAndBypass,
        }
    }

    /// Reject-invalid policy: returns every violation. InProcess policies
    /// carry no bridge semantics; their budgets must be the zero values
    /// of `in_process()` (a nonzero field on an in-process policy is a
    /// lie — it claims limits nothing enforces).
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();
        let mut bad = |m: String| errors.push(m);

        match self.mode {
            IsolationMode::InProcess => {
                if self.bridge.event_queue_capacity != 0
                    || self.bridge.audio_buffer_frames != 0
                    || self.bridge.max_bridge_depth != 0
                    || self.deadline.budget_ns != 0
                    || self.deadline.consecutive_misses_before_action != 0
                    || self.restart.max_restarts != 0
                    || self.restart.window_seconds != 0
                    || self.restart.backoff_ms != 0
                    || self.latency.declared_latency_frames != 0
                    || self.latency.compensate
                {
                    bad("inProcess policy must carry zeroed bridge/deadline/restart/latency budgets — nonzero values claim enforcement nothing provides".into());
                }
            }
            IsolationMode::IsolatedProcess => {
                let b = &self.bridge;
                if b.event_queue_capacity == 0 || b.event_queue_capacity > MAX_EVENT_QUEUE_CAPACITY
                {
                    bad(format!(
                        "bridge.eventQueueCapacity {} out of 1..={MAX_EVENT_QUEUE_CAPACITY}",
                        b.event_queue_capacity
                    ));
                }
                if b.audio_buffer_frames < MIN_AUDIO_BUFFER_FRAMES
                    || b.audio_buffer_frames > MAX_AUDIO_BUFFER_FRAMES
                {
                    bad(format!("bridge.audioBufferFrames {} out of {MIN_AUDIO_BUFFER_FRAMES}..={MAX_AUDIO_BUFFER_FRAMES}", b.audio_buffer_frames));
                }
                if b.max_bridge_depth == 0 || b.max_bridge_depth > MAX_BRIDGE_DEPTH {
                    bad(format!(
                        "bridge.maxBridgeDepth {} out of 1..={MAX_BRIDGE_DEPTH}",
                        b.max_bridge_depth
                    ));
                }
                let d = &self.deadline;
                if d.budget_ns == 0 || d.budget_ns > MAX_DEADLINE_BUDGET_NS {
                    bad(format!(
                        "deadline.budgetNs {} out of 1..={MAX_DEADLINE_BUDGET_NS}",
                        d.budget_ns
                    ));
                }
                if d.consecutive_misses_before_action == 0 {
                    bad("deadline.consecutiveMissesBeforeAction must be >= 1".into());
                }
                let r = &self.restart;
                if r.max_restarts == 0 || r.max_restarts > MAX_RESTARTS {
                    bad(format!(
                        "restart.maxRestarts {} out of 1..={MAX_RESTARTS}",
                        r.max_restarts
                    ));
                }
                if r.window_seconds == 0 || r.window_seconds > MAX_RESTART_WINDOW_SECONDS {
                    bad(format!(
                        "restart.windowSeconds {} out of 1..={MAX_RESTART_WINDOW_SECONDS}",
                        r.window_seconds
                    ));
                }
                if r.backoff_ms > MAX_RESTART_BACKOFF_MS {
                    bad(format!(
                        "restart.backoffMs {} > {MAX_RESTART_BACKOFF_MS}",
                        r.backoff_ms
                    ));
                }
                if self.latency.declared_latency_frames > MAX_LATENCY_FRAMES {
                    bad(format!(
                        "latency.declaredLatencyFrames {} > {MAX_LATENCY_FRAMES}",
                        self.latency.declared_latency_frames
                    ));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}

/// What isolation mode the coordinator requests for a plugin build.
///
/// `format` is the registry `PluginFormat` rendered (e.g. "vst3"); passing
/// a `PluginDescriptor` keeps formats typed. `platform` ids match the
/// registry ("macos-arm64", "linux-x86_64", "windows-x86_64").
///
/// Honest answers only: AAX → FormatNotHostable always; AU → only macOS;
/// every other format on a qualified platform returns the policy the host
/// would enforce — `IsolationUnavailable::NoIsolatedHost` is what a
/// runtime without `native/void-plugin-worker` must surface, and that is
/// the caller's responsibility to report, not this function's to fake.
pub fn requested_policy(
    descriptor: &crate::registry::PluginDescriptor,
    platform: &str,
) -> Result<IsolationPolicy, IsolationUnavailable> {
    use crate::registry::PluginFormat;
    match &descriptor.format {
        PluginFormat::Aax => Err(IsolationUnavailable::FormatNotHostable {
            format: "aax".into(),
        }),
        PluginFormat::Au if !platform.starts_with("macos") => {
            Err(IsolationUnavailable::PlatformNotSupported {
                format: "au".into(),
                platform: platform.to_string(),
            })
        }
        // Builtins are engine code — in-process by definition.
        PluginFormat::Builtin => Ok(IsolationPolicy::in_process()),
        _ => Ok(IsolationPolicy::isolated_default()),
    }
}
