//! Job model + state machine (CONTRACTS.md §6):
//! `queued → running → succeeded | failed | cancelling → cancelled`.
//!
//! - Queued jobs cancel directly.
//! - Terminal result is singular.
//! - Results arriving after cancellation, for the wrong epoch/project or
//!   with an expired context are QUARANTINED — never applied.

use crate::error::{JobError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelling,
    Cancelled,
}

impl JobStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Cancelling => "cancelling",
            Self::Cancelled => "cancelled",
        }
    }
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "queued" => Self::Queued,
            "running" => Self::Running,
            "succeeded" => Self::Succeeded,
            "failed" => Self::Failed,
            "cancelling" => Self::Cancelling,
            "cancelled" => Self::Cancelled,
            other => return Err(JobError::InvalidSpec(format!("status {other:?}"))),
        })
    }
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobKind {
    Symbolic,
    Transcription,
    Separation,
    AudioGeneration,
    VisualGeneration,
    Analysis,
    AvExport,
}

/// The contract-side job description (schemas/job/1.0.0). Resource fields
/// are decimal strings per §1; `parameters` must additionally pass the
/// runtime/task-specific schema — this type does not carry argv.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSpec {
    pub job_id: String,
    pub project_id: String,
    pub source_revision: String,
    pub context_sha256: String,
    pub kind: JobKind,
    pub runtime_id: String,
    pub runtime_sha256: String,
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub model_sha256: Option<String>,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub parameters: serde_json::Value,
    pub reservations: Reservations,
    pub deadline_monotonic_ns: String,
    pub output_scope_token: String,
    #[serde(default)]
    pub cloud_consent_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reservations {
    pub ram_bytes: String,
    pub vram_bytes: String,
    pub cpu_threads: u32,
}

fn is_hex64(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

impl JobSpec {
    /// Shape validation from the job schema (IDs, hashes, decimal fields,
    /// bounded inputs). Runtime-specific parameter validation is a second
    /// step owned by the qualified runtime manifest.
    pub fn validate(&self) -> Result<()> {
        let bad = |m: &str| JobError::InvalidSpec(m.into());
        if !void_protocol::ids::is_valid_id(&self.job_id) {
            return Err(bad("jobId not a UUID"));
        }
        if !void_protocol::ids::is_valid_id(&self.project_id) {
            return Err(bad("projectId not a UUID"));
        }
        if self.source_revision.parse::<u64>().is_err() {
            return Err(bad("sourceRevision not a decimal u64"));
        }
        if !is_hex64(&self.context_sha256) {
            return Err(bad("contextSha256 not sha256 hex"));
        }
        if self.runtime_id.is_empty() || self.runtime_id.len() > 200 {
            return Err(bad("runtimeId missing/oversized"));
        }
        if !is_hex64(&self.runtime_sha256) {
            return Err(bad("runtimeSha256 not sha256 hex"));
        }
        if let Some(m) = &self.model_sha256 {
            if !is_hex64(m) {
                return Err(bad("modelSha256 not sha256 hex"));
            }
        }
        if self.inputs.len() > 128 {
            return Err(bad("inputs exceeds 128 entries"));
        }
        for i in &self.inputs {
            if !is_hex64(i) {
                return Err(bad("inputs[] not sha256 hex"));
            }
        }
        for f in [
            &self.reservations.ram_bytes,
            &self.reservations.vram_bytes,
            &self.deadline_monotonic_ns,
        ] {
            if f.parse::<u64>().is_err() {
                return Err(bad("numeric field not a decimal u64"));
            }
        }
        if self.output_scope_token.is_empty() {
            return Err(bad("outputScopeToken missing"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobRecord {
    pub spec: JobSpec,
    pub status: JobStatus,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub result: Option<serde_json::Value>,
    /// Late/foreign results are quarantined — stored for inspection but
    /// flagged so no caller can apply them by accident.
    #[serde(default)]
    pub quarantined: bool,
}

/// Legal transitions of the §6 state machine.
pub fn transition(from: JobStatus, to: JobStatus) -> Result<()> {
    let ok = matches!(
        (from, to),
        (JobStatus::Queued, JobStatus::Running)
            | (JobStatus::Queued, JobStatus::Cancelled)
            | (JobStatus::Running, JobStatus::Succeeded)
            | (JobStatus::Running, JobStatus::Failed)
            | (JobStatus::Running, JobStatus::Cancelling)
            | (JobStatus::Cancelling, JobStatus::Cancelled)
    );
    if ok {
        Ok(())
    } else {
        Err(JobError::InvalidTransition {
            from: from.as_str(),
            to: to.as_str(),
        })
    }
}
