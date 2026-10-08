//! Job provenance record (W12, CONTRACTS.md §6).
//!
//! Written inside the published `jobs/<jobId>/` dir alongside
//! `result.json`: permitted inputs, model/runtime revision, settings
//! echo and verified output artifact SHA-256s. Success means verified
//! output exists — never "accepted into the song".

use crate::budget::JobBudget;
use crate::job::JobSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OutputArtifact {
    /// Declared name relative to the job staging dir.
    pub name: String,
    pub sha256: String,
    /// Decimal string per §1.
    pub bytes: String,
    /// Container-relative asset path (`assets/sha256/<hash>.<ext>`).
    pub asset: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProvenance {
    /// JobSpec's declared runtime id + artifact hash.
    pub runtime_id: String,
    pub runtime_sha256: String,
    /// Executable basename (never a path).
    pub executable: String,
    /// SHA-256 of the exact argv vector — invocation evidence.
    pub argv_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProvenance {
    pub format_version: u32,
    pub job_id: String,
    pub project_id: String,
    /// Source revision + context hash the job was bound to (§6).
    pub source_revision: String,
    pub context_sha256: String,
    pub kind: String,
    /// Approved model identity + hash when the job uses one (§6).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_sha256: Option<String>,
    pub runtime: RuntimeProvenance,
    /// Scoped input asset hashes (§6 permitted inputs).
    #[serde(default)]
    pub inputs: Vec<String>,
    /// Validated parameters, echoed verbatim.
    pub parameters: serde_json::Value,
    /// Reservations + the effective enforced budget.
    pub budget: JobBudgetRecord,
    /// Measured run facts (wall time; undeclared stray files).
    pub measured: Measured,
    #[serde(default)]
    pub artifacts: Vec<OutputArtifact>,
    #[serde(default)]
    pub warnings: Vec<String>,
    /// RFC3339 UTC.
    pub created_at: String,
    /// "succeeded" — only successful outcomes publish provenance.
    pub result: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobBudgetRecord {
    pub ram_bytes: String,
    pub vram_bytes: String,
    pub cpu_threads: u32,
    pub cpu_seconds: String,
    pub memory_bytes: String,
    pub deadline_monotonic_ns: String,
    pub wall_ns: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Measured {
    pub wall_ns: String,
    /// Files the worker wrote but did not declare — dropped, counted.
    #[serde(default)]
    pub stray_files: u32,
}

impl JobProvenance {
    pub fn new(
        spec: &JobSpec,
        budget: &JobBudget,
        runtime: RuntimeProvenance,
        model_version: Option<String>,
        artifacts: Vec<OutputArtifact>,
        warnings: Vec<String>,
        measured: Measured,
        created_at: String,
    ) -> Self {
        Self {
            format_version: 1,
            job_id: spec.job_id.clone(),
            project_id: spec.project_id.clone(),
            source_revision: spec.source_revision.clone(),
            context_sha256: spec.context_sha256.clone(),
            kind: kind_str(spec.kind).to_string(),
            model_id: spec.model_id.clone(),
            model_version,
            model_sha256: spec.model_sha256.clone(),
            runtime,
            inputs: spec.inputs.clone(),
            parameters: spec.parameters.clone(),
            budget: JobBudgetRecord {
                ram_bytes: spec.reservations.ram_bytes.clone(),
                vram_bytes: spec.reservations.vram_bytes.clone(),
                cpu_threads: spec.reservations.cpu_threads,
                cpu_seconds: budget.cpu_seconds.to_string(),
                memory_bytes: budget.memory_bytes.to_string(),
                deadline_monotonic_ns: budget.deadline_monotonic_ns.to_string(),
                wall_ns: budget.wall_ns.to_string(),
            },
            measured,
            artifacts,
            warnings,
            created_at,
            result: "succeeded".into(),
        }
    }
}

fn kind_str(k: crate::job::JobKind) -> &'static str {
    match k {
        crate::job::JobKind::Symbolic => "symbolic",
        crate::job::JobKind::Transcription => "transcription",
        crate::job::JobKind::Separation => "separation",
        crate::job::JobKind::AudioGeneration => "audio_generation",
        crate::job::JobKind::VisualGeneration => "visual_generation",
        crate::job::JobKind::Analysis => "analysis",
        crate::job::JobKind::AvExport => "av_export",
    }
}
