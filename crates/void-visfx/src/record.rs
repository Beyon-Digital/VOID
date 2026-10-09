//! VisGenRecord — per-generation lifecycle record, mirroring the
//! void-proposals state machine (W13) for generated visual scenes:
//! pending → ready → accepted|rejected|stale (T85 accept/reject).

use crate::spec::SceneDoc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VisGenStatus {
    Pending,
    Ready,
    Failed,
    Accepted,
    Rejected,
    Stale,
}

impl VisGenStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Stale => "stale",
        }
    }
    pub fn can_transition_to(self, next: Self) -> bool {
        use VisGenStatus::*;
        matches!(
            (self, next),
            (Pending, Ready)
                | (Pending, Failed)
                | (Pending, Stale)
                | (Pending, Rejected)
                | (Ready, Accepted)
                | (Ready, Rejected)
                | (Ready, Stale)
        )
    }
}

/// Brief fact set per produced artifact (full bytes live in assets/).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactBrief {
    pub filename: String,
    pub sha256: String,
    pub bytes: u64,
    pub asset_rel: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PendingFacts {
    pub submitted_at: String,
}

/// Provenance recorded on Ready — mirrors ProposalProvenance (W13):
/// job identity, generator facts, model facts, the document hash +
/// asset link, and the worker's measured analysis verbatim (T85).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisGenProvenance {
    pub job_id: String,
    pub generator_id: String,
    pub generator_version: String,
    #[serde(default)]
    pub model_id: Option<String>,
    pub runtime_id: String,
    pub runtime_sha256: String,
    /// Decimal u64 seed used.
    pub seed: String,
    /// sha256 of scene.json as imported.
    pub document_sha256: String,
    /// The worker's declared analysis block, echoed verbatim.
    #[serde(default)]
    pub analysis: serde_json::Value,
    /// container-relative asset path of scene.json.
    pub document_asset: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadyFacts {
    pub finished_at: String,
    pub scene_doc_sha256: String,
    /// Parsed + validated scene document.
    pub scene: SceneDoc,
    pub artifacts: Vec<ArtifactBrief>,
    /// Full provenance — runtime id/sha, generator+model facts, doc
    /// hash + asset link, measured analysis (T85 provenance panel).
    pub provenance: VisGenProvenance,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FailedFacts {
    pub at: String,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RejectedFacts {
    pub at: String,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StaleFacts {
    pub at: String,
    /// context_changed | project_changed | job_cancelled | superseded
    pub cause: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisGenRecord {
    pub schema_version: u32,
    pub record_id: String,
    pub project_id: String,
    /// Decimal u64 source revision at request time.
    pub revision: String,
    pub context_sha256: String,
    /// Canonical hash of the SceneGenSpec the job ran with.
    pub spec_sha256: String,
    pub spec: crate::spec::SceneGenSpec,
    pub job_id: String,
    pub status: VisGenStatus,
    #[serde(default)]
    pub pending: Option<PendingFacts>,
    #[serde(default)]
    pub ready: Option<ReadyFacts>,
    #[serde(default)]
    pub failed: Option<FailedFacts>,
    #[serde(default)]
    pub rejected: Option<RejectedFacts>,
    #[serde(default)]
    pub stale: Option<StaleFacts>,
    #[serde(default)]
    pub transaction_id: Option<String>,
    #[serde(default)]
    pub supersedes: Option<String>,
}

impl VisGenRecord {
    pub fn transition(&mut self, next: VisGenStatus) -> crate::error::Result<()> {
        if !self.status.can_transition_to(next) {
            return Err(crate::error::VisFxError::InvalidTransition {
                from: self.status.label(),
                to: next.label(),
            });
        }
        self.status = next;
        Ok(())
    }
}
