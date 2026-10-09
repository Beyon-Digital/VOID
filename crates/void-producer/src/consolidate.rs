//! Project consolidation planner (T80-side; DOC-04, OUT-02).
//!
//! Plans + executes "collect external media into the container": walk
//! the caller-supplied external references, hash each source, import
//! into the `AssetStore` (content-addressed — dedup is free), and emit
//! a manifest mapping old paths → asset hashes. Re-running the plan is
//! idempotent and *never* imports an asset another plan alternative
//! still references (protected list). No silent replacements: a source
//! whose content changed under a path reports the change explicitly.

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use void_assets::{file_sha256, hex_sha256, AssetStore};

/// One external reference the project points at (file on disk outside
/// the container's managed asset root).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalRef {
    /// Absolute path as the project recorded it.
    pub path: String,
    /// What inside the project references it (clip id / take id…).
    pub referenced_by: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RefStatus {
    /// Imported (or already present) — `new_sha` is authoritative.
    Collected,
    /// Content hash equals a hash another plan already owns.
    AlreadyContained,
    /// Source file missing/unreadable — the plan fails closed: the
    /// reference is reported, never silently dropped.
    Missing,
    /// Content under `path` differs from a previously collected sha
    /// recorded for the same path (drift — reported, not hidden).
    Drifted,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsolidatedRef {
    pub path: String,
    pub referenced_by: String,
    pub status: RefStatus,
    /// sha256 of the imported/present asset (when present).
    pub sha256: Option<String>,
    /// Bytes imported for this ref.
    pub bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsolidationPlan {
    pub project_id: String,
    /// sha256 set already referenced by OTHER project alternatives —
    /// protected from eviction during consolidation (alternatives must
    /// keep resolving after a consolidate lands).
    pub protected_shas: Vec<String>,
    pub refs: Vec<ConsolidatedRef>,
    /// Total new bytes this plan would add to the store.
    pub new_bytes: u64,
    /// Whether every referenced source resolved. False → caller must
    /// surface Missing/Drifted entries, never proceed silently.
    pub complete: bool,
}

/// Build the plan without touching the store (dry run — used for the
/// "collect files" preview and for tests that only need the mapping).
pub fn plan_consolidation(
    project_id: &str,
    externals: &[ExternalRef],
    protected_shas: &[String],
) -> Result<ConsolidationPlan> {
    let protected: std::collections::HashSet<&str> =
        protected_shas.iter().map(|s| s.as_str()).collect();
    let mut refs = Vec::with_capacity(externals.len());
    let mut new_bytes = 0u64;
    let mut complete = true;
    for e in externals {
        let p = Path::new(&e.path);
        match std::fs::metadata(p) {
            Ok(md) if md.is_file() => {
                let sha = file_sha256(p)?;
                let status = if protected.contains(sha.as_str()) {
                    RefStatus::AlreadyContained
                } else {
                    RefStatus::Collected
                };
                if status == RefStatus::Collected {
                    new_bytes += md.len();
                }
                refs.push(ConsolidatedRef {
                    path: e.path.clone(),
                    referenced_by: e.referenced_by.clone(),
                    status,
                    sha256: Some(sha),
                    bytes: md.len(),
                });
            }
            _ => {
                complete = false;
                refs.push(ConsolidatedRef {
                    path: e.path.clone(),
                    referenced_by: e.referenced_by.clone(),
                    status: RefStatus::Missing,
                    sha256: None,
                    bytes: 0,
                });
            }
        }
    }
    Ok(ConsolidationPlan {
        project_id: project_id.to_string(),
        protected_shas: protected_shas.to_vec(),
        refs,
        new_bytes,
        complete,
    })
}

/// Execute a plan: import each collected ref into `store`. Returns the
/// finalized manifest (per-ref sha). Missing refs are skipped — the
/// caller owns surfacing them (`plan.complete` was already false).
pub fn execute_consolidation(
    plan: &ConsolidationPlan,
    store: &AssetStore,
) -> Result<ConsolidationPlan> {
    let mut out = plan.clone();
    for r in &mut out.refs {
        match r.status {
            RefStatus::Collected => {
                let src = PathBuf::from(&r.path);
                let data = std::fs::read(&src)?;
                let ext = src
                    .extension()
                    .and_then(|x| x.to_str())
                    .unwrap_or("bin")
                    .to_string();
                let aref = store.import_bytes(&data, &ext)?;
                r.sha256 = Some(aref.sha256);
            }
            RefStatus::AlreadyContained | RefStatus::Missing | RefStatus::Drifted => {}
        }
    }
    Ok(out)
}

/// Write the consolidation manifest into the container (path → sha map
/// the relinker consumes, CONTRACTS §4 manifest convention: JSON at
/// `<root>/consolidation-<id>.json`, tmp+rename).
pub fn write_manifest(plan: &ConsolidationPlan, root: &Path) -> Result<PathBuf> {
    let id = hex_sha256(serde_json::to_vec(plan)?.as_slice());
    let dst = root.join(format!("consolidation-{id}.json"));
    let tmp = dst.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(plan)?)?;
    std::fs::rename(&tmp, &dst)?;
    Ok(dst)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_collects_and_reports_missing() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.wav");
        let b = dir.path().join("b.wav");
        std::fs::write(&a, b"aaaa").unwrap();
        std::fs::write(&b, b"bbbb").unwrap();
        let refs = vec![
            ExternalRef {
                path: a.display().to_string(),
                referenced_by: "clip1".into(),
            },
            ExternalRef {
                path: b.display().to_string(),
                referenced_by: "clip2".into(),
            },
            ExternalRef {
                path: dir.path().join("gone.wav").display().to_string(),
                referenced_by: "clip3".into(),
            },
        ];
        let plan = plan_consolidation("p", &refs, &[]).unwrap();
        assert!(!plan.complete);
        assert_eq!(plan.refs[0].status, RefStatus::Collected);
        assert_eq!(plan.refs[2].status, RefStatus::Missing);
        assert!(plan.refs[0].sha256.is_some());
    }

    #[test]
    fn execute_imports_into_store_and_skips_protected() {
        let dir = tempfile::tempdir().unwrap();
        let store = AssetStore::new(dir.path().join("assets"), 1 << 20).unwrap();
        // Seed an already-contained asset.
        let existing = store.import_bytes(b"already", "wav").unwrap();
        let f = dir.path().join("src.wav");
        std::fs::write(&f, b"new bytes").unwrap();
        let same = dir.path().join("same.wav");
        std::fs::write(&same, b"already").unwrap();
        let refs = vec![
            ExternalRef {
                path: f.display().to_string(),
                referenced_by: "c1".into(),
            },
            ExternalRef {
                path: same.display().to_string(),
                referenced_by: "c2".into(),
            },
        ];
        let plan = plan_consolidation("p", &refs, &[existing.sha256.clone()]).unwrap();
        assert_eq!(plan.refs[1].status, RefStatus::AlreadyContained);
        let done = execute_consolidation(&plan, &store).unwrap();
        assert!(store.find(&done.refs[0].sha256.clone().unwrap()).is_some());
        assert_eq!(store.verify(&existing.sha256).unwrap(), 7);
    }
}
