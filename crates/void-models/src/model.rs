//! Model descriptor records (W12, CONTRACTS.md §6 worker manifests).
//!
//! A descriptor is what the registry persists in the app SQLite index —
//! a *reconstructable* record, never the authority over the model files
//! themselves. Artifact references are SHA-256 addresses into the same
//! content-addressed store every other immutable blob uses (§4).

use serde::{Deserialize, Serialize};

/// Broad model family — the W12 triage axis (symbolic | audio | visual).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelKind {
    /// Symbolic/music-structure models (notes, chords, rhythm).
    Symbolic,
    /// Audio-domain models (generation, separation, transcription).
    Audio,
    /// Visual-domain models (generated/reactive visuals).
    Visual,
}

/// How the model executes. `argv` = a bundled argv-only worker process
/// (the workers/ contract); `internal` = code linked into the app —
/// both resolve through an approved allowlist, never from model data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeKind {
    Argv,
    Internal,
}

/// Runtime reference. The executable is a *basename only* — resolution
/// into the bundled workers directory is the coordinator's job, so a
/// manifest can never smuggle a path or remote code reference (T51).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeRef {
    pub kind: RuntimeKind,
    /// Basename of the approved worker executable (`[A-Za-z0-9._-]+`).
    pub executable: String,
    /// Optional SHA-256 of the expected executable bytes — when present,
    /// the coordinator verifies the resolved binary before spawning.
    #[serde(default)]
    pub executable_sha256: Option<String>,
    /// Version of the argv-worker protocol the model was qualified for.
    pub worker_protocol: u32,
}

/// One immutable model artifact, addressed by content hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactRef {
    pub sha256: String,
    /// Short type tag mirroring the asset-store extension (`wav`, `onnx`,
    /// `safetensors`, `json`, ...). Sanitized to `[a-z0-9]`.
    pub ext: String,
    /// Declared size, decimal string per CONTRACTS §1.
    pub bytes: String,
    /// Required artifacts make the model unusable when absent; optional
    /// ones degrade it (T51: missing models expose an unavailable state).
    #[serde(default = "default_required")]
    pub required: bool,
}

fn default_required() -> bool {
    true
}

/// What the model may do — stated, not implied (§6 worker manifests).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityFlags {
    /// Task kinds the manifest was qualified for (`symbolic`,
    /// `transcription`, `separation`, `audio_generation`, ...).
    #[serde(default)]
    pub task_kinds: Vec<String>,
    /// Declared hardware support (`cpu`, `coreml`, `cuda`, `metal`, ...).
    #[serde(default)]
    pub hardware: Vec<String>,
    /// Requires a GPU reservation (vram_bytes > 0 in budgets).
    #[serde(default)]
    pub requires_gpu: bool,
    /// Maximum scoped input bytes the runtime may be handed.
    #[serde(default)]
    pub max_input_bytes: String,
}

impl Default for CapabilityFlags {
    fn default() -> Self {
        Self {
            task_kinds: Vec::new(),
            hardware: vec!["cpu".into()],
            requires_gpu: false,
            max_input_bytes: "0".into(),
        }
    }
}

/// Per-job budget ceilings the model was qualified with. Jobs may lower
/// these per request but never exceed them — the runner clamps to these
/// values (T50). Decimal strings per CONTRACTS §1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BudgetDefaults {
    /// CPU seconds the process may burn (RLIMIT_CPU). 0 = unbounded is
    /// NOT allowed — every qualified model carries a real cap.
    pub cpu_seconds: String,
    /// Address-space cap in bytes (RLIMIT_AS).
    pub memory_bytes: String,
    /// VRAM reservation in bytes.
    pub vram_bytes: String,
    /// Wall-clock budget in ns relative to spawn. The job-level
    /// `deadlineMonotonicNs` additionally applies as an absolute bound.
    pub wall_ns: String,
    /// Threads the runtime may use (reservation axis).
    #[serde(default = "default_threads")]
    pub cpu_threads: u32,
}

fn default_threads() -> u32 {
    1
}

/// Lifecycle state the registry reports (T51 isolation states).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelStatus {
    /// All required artifacts verified — usable.
    Available,
    /// Optional artifacts missing — usable with reduced coverage.
    Degraded,
    /// Required artifacts missing — cannot run; install optional.
    Missing,
    /// Integrity/verification failure — refused.
    Rejected,
}

impl ModelStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Degraded => "degraded",
            Self::Missing => "missing",
            Self::Rejected => "rejected",
        }
    }
    pub fn parse(s: &str) -> Self {
        match s {
            "degraded" => Self::Degraded,
            "missing" => Self::Missing,
            "rejected" => Self::Rejected,
            _ => Self::Available,
        }
    }
    /// Can a job actually launch on this model?
    pub fn runnable(self) -> bool {
        matches!(self, Self::Available | Self::Degraded)
    }
}

/// The persisted registry record — JSON-serializable so the index row is
/// self-describing and rebuildable into any future schema.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDescriptor {
    /// Registry identifier — namespaced string, not a UUID (`org.name`).
    pub model_id: String,
    pub version: String,
    pub name: String,
    pub kind: ModelKind,
    pub runtime: RuntimeRef,
    #[serde(default)]
    pub artifacts: Vec<ArtifactRef>,
    #[serde(default)]
    pub capabilities: CapabilityFlags,
    pub budgets: BudgetDefaults,
    /// Licence/source records (§8: models need their own licence trail).
    pub license: String,
    pub source: String,
    pub status: ModelStatus,
    /// Integrity hash of the manifest this descriptor was installed
    /// from — covers the whole manifest content.
    pub manifest_sha256: String,
    /// RFC3339 UTC install time ("" for bundled content).
    #[serde(default)]
    pub installed_at: String,
}
