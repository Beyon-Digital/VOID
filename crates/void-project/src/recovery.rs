//! Startup recovery (CONTRACTS.md §4):
//!
//! - Verify CURRENT → manifest → every referenced byte.
//! - If the pointer is missing/corrupt, COMPLETE+VERIFIED checkpoints are
//!   offered as explicitly-labelled candidates — never silently promoted
//!   by mtime, and staging is never authoritative.
//! - Incomplete staging dirs are quarantined under `staging/_quarantine/`.
//! - Recording chunks are validated against their journal and reported for
//!   explicit salvage.
//! - Live receipts newer than the verified checkpoint become
//!   OUTCOME_UNKNOWN candidates — never silently replayed.

use crate::current::{self, CurrentPointer};
use crate::error::Result;
use crate::layout;
use crate::verify::{self, VerifiedCheckpoint};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointerStatus {
    /// CURRENT read, parsed, and points at a fully verified checkpoint.
    Valid { checkpoint_id: String },
    /// No CURRENT file — brand-new or damaged container.
    Missing,
    /// CURRENT exists but is unparseable, malformed, or points at a
    /// checkpoint that fails verification.
    Corrupt { detail: String },
}

#[derive(Debug, Clone)]
pub struct RecoveryCandidate {
    pub checkpoint: VerifiedCheckpoint,
    /// Why it is a candidate: always explicit, never auto-promoted.
    pub reason: &'static str,
}

/// Recording journal sidecar written next to growing take chunks.
/// `finalized=false` means the take was interrupted; chunks may still be
/// valid media up to their own verified sizes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingJournal {
    pub take_id: String,
    pub finalized: bool,
    #[serde(default)]
    pub chunks: Vec<RecordingChunk>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingChunk {
    pub path: String,
    pub sha256: String,
    pub bytes: String,
}

#[derive(Debug, Clone)]
pub struct RecordingSalvage {
    pub take_id: String,
    pub dir: PathBuf,
    pub finalized: bool,
    /// Chunks that verified against the journal.
    pub valid_chunks: Vec<String>,
    /// Chunks that failed verification (truncated tail, etc).
    pub corrupt_chunks: Vec<(String, String)>,
}

#[derive(Debug, Default)]
pub struct RecoveryReport {
    pub pointer: Option<PointerStatus>,
    /// The verified checkpoint CURRENT authoritatively points at.
    pub current: Option<VerifiedCheckpoint>,
    /// Earlier verified checkpoints retained by policy.
    pub retained: Vec<VerifiedCheckpoint>,
    /// Complete+verified checkpoints not referenced by CURRENT — offered
    /// to the user explicitly; never promoted automatically.
    pub orphan_candidates: Vec<RecoveryCandidate>,
    /// Checkpoint dirs that failed verification (id, reason).
    pub corrupt_checkpoints: Vec<(String, String)>,
    /// Staging dirs moved to quarantine this pass.
    pub quarantined_staging: Vec<PathBuf>,
    pub salvaged_recordings: Vec<RecordingSalvage>,
}

/// Run the full startup reconciliation over a container.
pub fn recover(root: &Path) -> Result<RecoveryReport> {
    let mut report = RecoveryReport {
        quarantined_staging: quarantine_staging(root)?,
        salvaged_recordings: salvage_recordings(root),
        ..Default::default()
    };

    let pointer = current::read_current(root);
    let (status, current) = match pointer {
        Ok(Some(ptr)) => match verify::verify_checkpoint(root, &ptr.checkpoint_id) {
            Ok(v) if v.manifest_sha256 == ptr.manifest_sha256 => (
                PointerStatus::Valid {
                    checkpoint_id: ptr.checkpoint_id.clone(),
                },
                Some(v),
            ),
            Ok(_) => (
                PointerStatus::Corrupt {
                    detail: "manifest hash does not match CURRENT".into(),
                },
                None,
            ),
            Err(e) => (
                PointerStatus::Corrupt {
                    detail: format!("pointed checkpoint failed verification: {e}"),
                },
                None,
            ),
        },
        Ok(None) => (PointerStatus::Missing, None),
        Err(e) => (
            PointerStatus::Corrupt {
                detail: e.to_string(),
            },
            None,
        ),
    };
    report.current = current;
    report.pointer = Some(status);

    // Scan every checkpoint dir; classify without promoting anything.
    let cdir = layout::checkpoints_dir(root);
    if cdir.is_dir() {
        for e in fs::read_dir(&cdir)?.flatten() {
            if !e.path().is_dir() {
                continue;
            }
            let id = e.file_name().to_string_lossy().to_string();
            if report
                .current
                .as_ref()
                .is_some_and(|c| c.checkpoint_id == id)
            {
                continue;
            }
            match verify::verify_checkpoint(root, &id) {
                Ok(v) => {
                    let current_rev = report.current.as_ref().map(|c| c.revision);
                    if report.current.is_none() {
                        report.orphan_candidates.push(RecoveryCandidate {
                            checkpoint: v,
                            reason: "orphan: complete verified checkpoint, no valid CURRENT",
                        });
                    } else if current_rev.is_some_and(|cr| v.revision > cr) {
                        // Complete save that never published: ahead of
                        // CURRENT, so it is an explicit recovery candidate —
                        // NOT a superseded older checkpoint.
                        report.orphan_candidates.push(RecoveryCandidate {
                            checkpoint: v,
                            reason: "orphan: complete verified checkpoint never published (newer than CURRENT)",
                        });
                    } else {
                        report.retained.push(v);
                    }
                }
                Err(err) => report.corrupt_checkpoints.push((id, err.to_string())),
            }
        }
    }
    // Intentionally: no auto-promotion, no deletion of corrupt dirs —
    // recovery *reports*, the caller/user decides.
    Ok(report)
}

/// Move every incomplete `staging/<id>` into `staging/_quarantine/<id>` so
/// a later import or recovery pass can inspect them; returns moved paths.
pub fn quarantine_staging(root: &Path) -> Result<Vec<PathBuf>> {
    let staging = root.join(layout::STAGING_DIR);
    let quarantine = layout::quarantine_dir(root);
    let mut moved = Vec::new();
    if !staging.is_dir() {
        return Ok(moved);
    }
    for e in fs::read_dir(&staging)?.flatten() {
        let path = e.path();
        if !path.is_dir() {
            continue;
        }
        let name = e.file_name().to_string_lossy().to_string();
        if name == "_quarantine" {
            continue;
        }
        fs::create_dir_all(&quarantine)?;
        let mut dest = quarantine.join(&name);
        let mut i = 0;
        while dest.exists() {
            i += 1;
            dest = quarantine.join(format!("{name}.{i}"));
        }
        fs::rename(&path, &dest)?;
        moved.push(dest);
    }
    void_assets::sync_dir(&staging)?;
    Ok(moved)
}

/// Inspect `recordings/<takeId>` dirs: chunks are validated against their
/// journal hashes/sizes. Returns one salvage record per take — explicit
/// listing, never automatic deletion or promotion.
pub fn salvage_recordings(root: &Path) -> Vec<RecordingSalvage> {
    let rec = layout::recordings_dir(root);
    let mut out = Vec::new();
    let Ok(rd) = fs::read_dir(&rec) else {
        return out;
    };
    for e in rd.flatten() {
        let dir = e.path();
        if !dir.is_dir() {
            continue;
        }
        let take_id = e.file_name().to_string_lossy().to_string();
        let journal_path = dir.join("journal.json");
        let journal: Option<RecordingJournal> = fs::read(&journal_path)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        let mut valid = Vec::new();
        let mut corrupt = Vec::new();
        if let Some(j) = &journal {
            for chunk in &j.chunks {
                let rel = match crate::fsutil::safe_rel_path(&chunk.path) {
                    Ok(r) => r,
                    Err(e) => {
                        corrupt.push((chunk.path.clone(), e.to_string()));
                        continue;
                    }
                };
                let p = dir.join(rel);
                match std::fs::read(&p) {
                    Ok(bytes) => {
                        let sha = void_assets::hex_sha256(&bytes);
                        if sha == chunk.sha256 && bytes.len().to_string() == chunk.bytes {
                            valid.push(chunk.path.clone());
                        } else {
                            corrupt.push((chunk.path.clone(), "hash/size mismatch".into()));
                        }
                    }
                    Err(e) => corrupt.push((chunk.path.clone(), e.to_string())),
                }
            }
        }
        out.push(RecordingSalvage {
            take_id,
            dir,
            finalized: journal.map(|j| j.finalized).unwrap_or(false),
            valid_chunks: valid,
            corrupt_chunks: corrupt,
        });
    }
    out
}

/// Commands the live session recorded but the verified checkpoint did not
/// commit — every one is an OUTCOME_UNKNOWN candidate (CONTRACTS §3):
/// report them; never silently replay against new state.
pub fn uncertain_commands<'a>(
    live_command_ids: impl IntoIterator<Item = &'a str>,
    committed: &std::collections::HashSet<String>,
) -> Vec<String> {
    live_command_ids
        .into_iter()
        .filter(|id| !committed.contains(*id))
        .map(|s| s.to_string())
        .collect()
}
/// Resolve the pointer a recovery UI should offer, or None if the caller
/// must present `orphan_candidates` instead.
pub fn authoritative_pointer(root: &Path) -> Result<Option<CurrentPointer>> {
    current::read_current(root)
}
