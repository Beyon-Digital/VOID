//! Proposal record + lifecycle (CONTRACTS.md §6, W13).
//!
//! `pending → ready | failed | stale` then `ready → accepted | rejected
//! | stale`. Terminal: accepted/rejected/failed/stale. A stale proposal
//! is never mutated back to life — revalidation creates a NEW record
//! linked by `supersedes`.

use crate::context::RegionContext;
use crate::document::CandidateDoc;
use crate::error::{ProposalError, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalStatus {
    Pending,
    Ready,
    Stale,
    Accepted,
    Rejected,
    Failed,
}

impl ProposalStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Stale => "stale",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Failed => "failed",
        }
    }
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Accepted | Self::Rejected | Self::Failed | Self::Stale
        )
    }
}

/// Why a proposal went stale — shown to the user, never silent (T55).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StaleCause {
    /// Region contents changed (context hash no longer matches).
    ContextChanged,
    /// Target clip/track deleted or missing.
    TargetGone,
    /// Engine epoch restarted / project closed / superseded.
    SessionEnded,
    Superseded,
    /// Generation job was cancelled before results landed.
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposedNote {
    pub pitch: i32,
    pub velocity: i32,
    pub onset_ticks: String,
    pub length_ticks: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    /// Rank 1..N, assigned by score at ingest — stable.
    pub rank: u32,
    /// Model's own score (0..1 likelihood of its sample). Not a
    /// confidence claim.
    pub score: f64,
    pub rationale: String,
    pub notes: Vec<ProposedNote>,
}

/// Provenance carried on every proposal (contract §6: model id/version,
/// input region hash, generation params incl. seed — all recorded).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalProvenance {
    pub job_id: String,
    pub generator_id: String,
    pub generator_version: String,
    pub model_id: String,
    pub runtime_id: String,
    pub runtime_sha256: String,
    /// Deterministic seed — same context + seed ⇒ same proposals.
    pub seed: String,
    /// Document artifact hash in the content store.
    pub document_sha256: String,
    /// Worker analysis block echoed verbatim (measured facts only).
    pub analysis: serde_json::Value,
    pub document_asset: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedRecord {
    /// One transaction per accepted proposal (CONTRACTS.md §5).
    pub transaction_id: String,
    pub candidate_rank: u32,
    /// Note ids minted at accept time — the model never names objects.
    pub note_ids: Vec<String>,
    /// Candidate indices the accept dropped (locked-range collisions).
    pub dropped_indices: Vec<u32>,
    pub accepted_at: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProposalRecord {
    pub format_version: u32,
    pub proposal_id: String,
    pub project_id: String,
    /// Coordinator revision the context was captured at.
    pub source_revision: String,
    /// sha256 of the canonical RegionContext.
    pub context_sha256: String,
    /// The scoped context — retained verbatim for audit + revalidation.
    pub context: RegionContext,
    pub status: ProposalStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_cause: Option<StaleCause>,
    #[serde(default)]
    pub candidates: Vec<Candidate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<ProposalProvenance>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accepted: Option<AcceptedRecord>,
    /// Set when this record re-validates an older stale one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl Candidate {
    pub fn from_doc(rank: u32, d: &CandidateDoc) -> Self {
        Self {
            rank,
            score: d.score,
            rationale: d.rationale.clone(),
            notes: d
                .notes
                .iter()
                .map(|n| ProposedNote {
                    pitch: n.pitch,
                    velocity: n.velocity,
                    onset_ticks: n.onset_ticks.to_string(),
                    length_ticks: n.length_ticks.to_string(),
                })
                .collect(),
        }
    }
}

/// Legal edges — the state machine is the only authority.
pub fn transition(from: ProposalStatus, to: ProposalStatus) -> Result<()> {
    let ok = matches!(
        (from, to),
        (ProposalStatus::Pending, ProposalStatus::Ready)
            | (ProposalStatus::Pending, ProposalStatus::Failed)
            | (ProposalStatus::Pending, ProposalStatus::Stale)
            | (ProposalStatus::Pending, ProposalStatus::Rejected)
            | (ProposalStatus::Ready, ProposalStatus::Accepted)
            | (ProposalStatus::Ready, ProposalStatus::Rejected)
            | (ProposalStatus::Ready, ProposalStatus::Stale)
    );
    if ok {
        Ok(())
    } else {
        Err(ProposalError::InvalidTransition {
            from: from.as_str(),
            to: to.as_str(),
        })
    }
}
