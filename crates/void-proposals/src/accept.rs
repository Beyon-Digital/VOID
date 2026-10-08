//! Accept / reject — the only path from proposal to project edit.
//!
//! `accept()` revalidates freshness *inside* the accept call (contract
//! §6: revalidate on application, not just on display) and produces an
//! `AcceptPlan`: a set of `InsertNoteOp`-shaped placements sharing ONE
//! transaction id. The crate never talks to the engine — the studio
//! bridges the plan onto the existing PersistentCommand wire (W13: no
//! new ops).

use crate::error::{ProposalError, Result};
use crate::record::{transition, AcceptedRecord, ProposalRecord, ProposalStatus, StaleCause};
use crate::store::{utc_now, ProposalStore};

/// Freshness facts the caller must answer live from the workspace —
/// the crate can't see the engine; it *requires* honest answers.
pub struct Revalidation {
    /// Project the workspace currently has open.
    pub project_id: String,
    /// Recomputed sha256 of the region context RIGHT NOW (same
    /// canonicalization as RegionContext::sha256 over the *current*
    /// region contents).
    pub current_context_sha256: String,
    /// Target clip still exists (not deleted/moved away).
    pub target_clip_exists: bool,
}

/// One planned InsertNoteOp payload — studio maps 1:1 onto the wire
/// (`{op:"insert_note",clipId,noteId,pitch,velocity,startTicks,
/// lengthTicks}` inside the accepted gesture's transaction).
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedInsert {
    pub clip_id: String,
    pub note_id: String,
    pub pitch: i32,
    pub velocity: i32,
    pub start_ticks: String,
    pub length_ticks: String,
}

/// What the UI applies through the normal PersistentCommand path.
#[derive(Debug, Clone, PartialEq)]
pub struct AcceptPlan {
    /// One transaction per accepted proposal — undo rolls back all of
    /// it in one UndoOp (CONTRACTS.md §5).
    pub transaction_id: String,
    pub proposal_id: String,
    pub candidate_rank: u32,
    pub inserts: Vec<PlannedInsert>,
    /// Candidate indices skipped for overlapping locked ranges.
    pub dropped_indices: Vec<u32>,
}

/// Reject: discard for this session. Terminal — the record stays
/// auditable but can never be applied.
pub fn reject(store: &ProposalStore, proposal_id: &str) -> Result<ProposalRecord> {
    let mut rec = store.load(proposal_id)?;
    transition(rec.status, ProposalStatus::Rejected)?;
    rec.status = ProposalStatus::Rejected;
    rec.updated_at = utc_now();
    store.save(&rec)?;
    Ok(rec)
}

/// Accept a candidate (optionally a subset of its notes). Returns the
/// plan; the caller applies it via InsertNoteOps in `transaction_id`,
/// then persists the record's `accepted` block.
///
/// `indices` = candidate note indices to take (None = all). Notes
/// colliding with a recorded locked range are dropped from the plan —
/// never inserted — and reported in `dropped_indices` so the UI can say
/// what happened instead of silently skipping.
pub fn plan_accept(
    rec: &ProposalRecord,
    candidate_rank: u32,
    indices: Option<&[usize]>,
    reval: &Revalidation,
    drop_locked: bool,
) -> Result<AcceptPlan> {
    if rec.status != ProposalStatus::Ready {
        return Err(ProposalError::Stale(format!(
            "status {} — only ready proposals accept",
            rec.status.as_str()
        )));
    }
    // Revalidation inside accept (T55):
    if reval.project_id != rec.project_id {
        return Err(ProposalError::Revalidation("different project open".into()));
    }
    if !reval.target_clip_exists {
        return Err(ProposalError::Revalidation("target clip gone".into()));
    }
    if reval.current_context_sha256 != rec.context_sha256 {
        return Err(ProposalError::Revalidation(
            "region changed since generation".into(),
        ));
    }
    let cand = rec
        .candidates
        .iter()
        .find(|c| c.rank == candidate_rank)
        .ok_or_else(|| ProposalError::InvalidRequest("candidate rank missing".into()))?;
    let want: Vec<usize> = match indices {
        Some(ix) => {
            let mut v = ix.to_vec();
            v.sort_unstable();
            v.dedup();
            v
        }
        None => (0..cand.notes.len()).collect(),
    };
    if want.iter().any(|&i| i >= cand.notes.len()) {
        return Err(ProposalError::InvalidRequest(
            "note index out of range".into(),
        ));
    }
    let tx = uuid::Uuid::new_v4().to_string();
    let mut inserts = Vec::with_capacity(want.len());
    let mut dropped = Vec::new();
    let mut locked_hit = Vec::new();
    for i in want {
        let n = &cand.notes[i];
        let onset: i64 = n
            .onset_ticks
            .parse()
            .map_err(|_| ProposalError::MalformedDocument("accepted note onset".into()))?;
        let len: i64 = n.length_ticks.parse().unwrap_or(0);
        let locked = rec
            .context
            .locked_ranges
            .iter()
            .any(|r| r.overlaps(onset, onset + len));
        if locked && !drop_locked {
            locked_hit.push(i);
            continue;
        }
        if locked && drop_locked {
            dropped.push(i as u32);
            continue;
        }
        inserts.push(PlannedInsert {
            clip_id: rec.context.clip_id.clone(),
            note_id: uuid::Uuid::new_v4().to_string(), // ids minted HERE
            pitch: n.pitch,
            velocity: n.velocity,
            start_ticks: n.onset_ticks.clone(),
            length_ticks: n.length_ticks.clone(),
        });
    }
    if !locked_hit.is_empty() {
        return Err(ProposalError::LockedCollision(locked_hit));
    }
    if inserts.is_empty() {
        return Err(ProposalError::InvalidRequest(
            "selection produced no insertable notes".into(),
        ));
    }
    Ok(AcceptPlan {
        transaction_id: tx,
        proposal_id: rec.proposal_id.clone(),
        candidate_rank,
        inserts,
        dropped_indices: dropped,
    })
}

/// Persist the accepted terminal state after the engine confirmed the
/// inserts (call this once the command round-trips — accepted notes are
/// then normal editable MIDI).
pub fn commit_accepted(
    store: &ProposalStore,
    proposal_id: &str,
    plan: &AcceptPlan,
) -> Result<ProposalRecord> {
    let mut rec = store.load(proposal_id)?;
    transition(rec.status, ProposalStatus::Accepted)?;
    rec.status = ProposalStatus::Accepted;
    rec.accepted = Some(AcceptedRecord {
        transaction_id: plan.transaction_id.clone(),
        candidate_rank: plan.candidate_rank,
        note_ids: plan.inserts.iter().map(|i| i.note_id.clone()).collect(),
        dropped_indices: plan.dropped_indices.clone(),
        accepted_at: utc_now(),
    });
    rec.updated_at = utc_now();
    store.save(&rec)?;
    Ok(rec)
}

/// Mark a single proposal stale (T55 — region edit detected for one).
pub fn mark_stale_one(
    store: &ProposalStore,
    proposal_id: &str,
    cause: StaleCause,
) -> Result<ProposalRecord> {
    let mut rec = store.load(proposal_id)?;
    transition(rec.status, ProposalStatus::Stale)?;
    rec.status = ProposalStatus::Stale;
    rec.stale_cause = Some(cause);
    rec.updated_at = utc_now();
    store.save(&rec)?;
    Ok(rec)
}
