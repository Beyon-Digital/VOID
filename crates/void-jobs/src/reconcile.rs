//! DB ↔ CURRENT reconciliation (CONTRACTS.md §4, T17).
//!
//! The checkpoint manifest is the authority; the DB is a reconstructable
//! index. Reconciliation always flows container → DB. Deliberately a pure
//! function over already-verified data: this crate never reads the
//! container itself — the caller feeds it a `VerifiedCheckpoint` so the
//! index can only be rebuilt from verified manifests.

use crate::db::JobDb;
use crate::error::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconcileOutcome {
    /// Index already matched CURRENT.
    Consistent,
    /// Index rebuilt/updated to match the verified pointer.
    Repaired { previous_indexed: Option<String> },
    /// No valid pointer — index left untouched; caller offers recovery
    /// candidates instead of fabricating state.
    NoAuthority,
}

/// Reconcile the index to a verified checkpoint pointer. Pass `None` when
/// recovery found no valid CURRENT (orphan path) — the index stays as-is.
pub fn reconcile(
    db: &JobDb,
    project_id: &str,
    verified: Option<(&str, u64, &str, &[String])>, // (checkpoint_id, revision, manifest_sha, asset_hashes)
) -> Result<ReconcileOutcome> {
    let Some((checkpoint_id, revision, manifest_sha, asset_hashes)) = verified else {
        return Ok(ReconcileOutcome::NoAuthority);
    };
    let previous = db.indexed_current(project_id)?;
    if previous.as_deref() == Some(checkpoint_id) {
        return Ok(ReconcileOutcome::Consistent);
    }
    db.upsert_current_checkpoint(
        project_id,
        checkpoint_id,
        revision,
        manifest_sha,
        asset_hashes,
    )?;
    Ok(ReconcileOutcome::Repaired {
        previous_indexed: previous,
    })
}
