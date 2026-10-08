//! Generation service: request → void-jobs submit/run → collect into
//! proposal records. The JobRunner stays the authority on worker
//! boundaries; this module owns the proposal-side lifecycle.

use crate::context::RegionContext;
use crate::document::parse_document;
use crate::error::{ProposalError, Result};
use crate::record::{
    Candidate as RecCandidate, ProposalProvenance, ProposalRecord, ProposalStatus, StaleCause,
};
use crate::store::{utc_now, ProposalStore};
use sha2::{Digest, Sha256};
use std::path::Path;
use void_jobs::{decide, Admit, JobDb, JobKind, JobSpec, JobStatus, Reservations, RunOutcome};

/// The bundled symbolic worker's runtime identity — pinned by the
/// coordinator's allowlist; callers pass the resolved executable path +
/// its SHA-256 (never derived from job data, contract §2).
pub const SYMBOLIC_RUNTIME_ID: &str = "void-symbolic-worker";

pub const MAX_PROPOSALS: u32 = 8;

pub struct GenerateRequest {
    pub project_id: String,
    pub source_revision: String,
    pub context: RegionContext,
    /// Max candidates (bounded 1..=8).
    pub max_proposals: u32,
    /// None → derived deterministically from the context hash.
    pub seed: Option<u64>,
    /// Resolved worker binary + pinned hash.
    pub runtime_sha256: String,
    pub reservations: Reservations,
    pub deadline_monotonic_ns: String,
}

pub struct ProposalService {
    pub db: JobDb,
    pub store: ProposalStore,
    pub project_root: std::path::PathBuf,
}

/// What `generate` leaves behind for the caller to run.
pub struct PendingProposal {
    pub record: ProposalRecord,
    pub spec: JobSpec,
}

impl ProposalService {
    pub fn new(db: JobDb, store: ProposalStore, project_root: std::path::PathBuf) -> Self {
        Self {
            db,
            store,
            project_root,
        }
    }

    /// Validate the context, admit a symbolic job, persist a pending
    /// record. The caller runs `JobRunner::run` on `pending.spec`.
    pub fn request(&self, req: &GenerateRequest) -> Result<PendingProposal> {
        req.context.validate()?;
        if req.source_revision.parse::<u64>().is_err() {
            return Err(ProposalError::InvalidRequest("sourceRevision".into()));
        }
        if !(1..=MAX_PROPOSALS).contains(&req.max_proposals) {
            return Err(ProposalError::InvalidRequest("maxProposals 1..=8".into()));
        }
        let seed = req.seed.unwrap_or_else(|| req.context.default_seed());
        let proposal_id = uuid::Uuid::new_v4().to_string();
        let job_id = uuid::Uuid::new_v4().to_string();
        let context_sha256 = req.context.sha256();

        let spec = JobSpec {
            job_id: job_id.clone(),
            project_id: req.project_id.clone(),
            source_revision: req.source_revision.clone(),
            context_sha256: context_sha256.clone(),
            kind: JobKind::Symbolic,
            runtime_id: SYMBOLIC_RUNTIME_ID.into(),
            runtime_sha256: req.runtime_sha256.clone(),
            model_id: Some("interval-markov-1".into()),
            model_sha256: None, // algorithmic generator — no artifact hash
            inputs: vec![context_sha256.clone()],
            parameters: req.context.to_worker_params(seed, req.max_proposals),
            reservations: req.reservations.clone(),
            deadline_monotonic_ns: req.deadline_monotonic_ns.clone(),
            output_scope_token: format!("prop-{proposal_id}"),
            cloud_consent_id: None,
        };
        spec.validate()
            .map_err(|e| ProposalError::InvalidRequest(e.to_string()))?;

        match decide(&self.db, &spec)? {
            Admit::StartNow | Admit::Queue => {}
            Admit::Full => return Err(ProposalError::InvalidRequest("job queue full".into())),
        }
        self.db.submit(&spec)?;

        let now = utc_now();
        let record = ProposalRecord {
            format_version: 1,
            proposal_id,
            project_id: req.project_id.clone(),
            source_revision: req.source_revision.clone(),
            context_sha256,
            context: req.context.clone(),
            status: ProposalStatus::Pending,
            stale_cause: None,
            candidates: Vec::new(),
            provenance: None,
            accepted: None,
            supersedes: None,
            error: None,
            created_at: now.clone(),
            updated_at: now,
        };
        self.store.save(&record)?;
        Ok(PendingProposal { record, spec })
    }

    /// After `runner.run(spec)` finishes: fold the outcome into the
    /// proposal record. Succeeded → parse+validate the proposals.json
    /// artifact → `ready`. Failed/cancelled → `failed`/`stale(cancelled)`.
    /// Late or cancelled results never reach `ready` — the runner's own
    /// quarantine already decided (T49/T55).
    pub fn collect(
        &self,
        proposal_id: &str,
        job: &void_jobs::JobRecord,
        outcome: &RunOutcome,
    ) -> Result<ProposalRecord> {
        let mut rec = self.store.load(proposal_id)?;
        match outcome {
            RunOutcome::Succeeded {
                artifacts,
                warnings,
                ..
            } if job.status == JobStatus::Succeeded => {
                let doc_art = artifacts
                    .iter()
                    .find(|a| a.name == "proposals.json")
                    .ok_or_else(|| ProposalError::MalformedDocument("no proposals.json".into()))?;
                let doc = parse_document(&std::fs::read(&doc_art.asset_abs)?)?;
                rec.candidates = doc
                    .candidates
                    .iter()
                    .enumerate()
                    .map(|(i, c)| RecCandidate::from_doc(i as u32 + 1, c))
                    .collect();
                rec.provenance = Some(ProposalProvenance {
                    job_id: job.spec.job_id.clone(),
                    generator_id: doc.generator.id,
                    generator_version: doc.generator.version,
                    model_id: doc.generator.model,
                    runtime_id: job.spec.runtime_id.clone(),
                    runtime_sha256: job.spec.runtime_sha256.clone(),
                    seed: doc.seed.to_string(),
                    document_sha256: doc_art.sha256.clone(),
                    analysis: doc.analysis,
                    document_asset: doc_art.asset_rel.clone(),
                });
                transition_pending(&mut rec, ProposalStatus::Ready)?;
                if !warnings.is_empty() {
                    rec.error = Some(warnings.join("; "));
                }
            }
            RunOutcome::Cancelled { .. } => {
                transition_pending(&mut rec, ProposalStatus::Stale)?;
                rec.stale_cause = Some(StaleCause::Cancelled);
            }
            RunOutcome::Failed { error, .. } => {
                transition_pending(&mut rec, ProposalStatus::Failed)?;
                rec.error = Some(error.clone());
            }
            _ => {
                transition_pending(&mut rec, ProposalStatus::Failed)?;
                rec.error = Some("job did not reach succeeded".into());
            }
        }
        rec.updated_at = utc_now();
        self.store.save(&rec)?;
        Ok(rec)
    }

    /// Sweep live proposals for a project to `stale` — used on project
    /// close / engine epoch change / region mutation (T55).
    pub fn mark_stale(&self, project_id: &str, cause: StaleCause) -> Result<usize> {
        let mut n = 0;
        for mut rec in self.store.list_for_project(project_id) {
            if matches!(rec.status, ProposalStatus::Ready | ProposalStatus::Pending) {
                if transition_pending(&mut rec, ProposalStatus::Stale).is_ok() {
                    rec.stale_cause = Some(cause.clone());
                    rec.updated_at = utc_now();
                    self.store.save(&rec)?;
                    n += 1;
                }
            }
        }
        Ok(n)
    }

    /// Revalidation never resurrects a stale record: it mints a new
    /// pending proposal bound to the fresh context, linked by
    /// `supersedes` (CONTRACTS.md §6).
    pub fn revalidate(&self, stale_id: &str, req: &GenerateRequest) -> Result<PendingProposal> {
        let old = self.store.load(stale_id)?;
        if old.status != ProposalStatus::Stale {
            return Err(ProposalError::Revalidation("source not stale".into()));
        }
        let mut pending = self.request(req)?;
        pending.record.supersedes = Some(stale_id.into());
        self.store.save(&pending.record)?;
        Ok(pending)
    }
}

fn transition_pending(rec: &mut ProposalRecord, to: ProposalStatus) -> Result<()> {
    crate::record::transition(rec.status, to).map(|_| {
        rec.status = to;
    })
}

/// SHA-256 of a file for runtime pinning (the coordinator computes the
/// worker binary's hash at registration time and stores it in specs).
pub fn file_sha256(path: &Path) -> Result<String> {
    let d: [u8; 32] = Sha256::digest(&std::fs::read(path)?).into();
    Ok(d.iter().map(|b| format!("{b:02x}")).collect())
}
