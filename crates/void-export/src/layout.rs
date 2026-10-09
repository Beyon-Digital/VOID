//! Export paths inside the project container (CONTRACTS.md §4/§7).
//!
//! Output stages under `staging/export-<jobId>/` — never authoritative —
//! and publishes by rename into `exports/<jobId>/` on the same filesystem,
//! mirroring the checkpoint staging→publish boundary.

use std::path::{Path, PathBuf};

/// Delivered render outputs live here, one directory per export job.
pub const EXPORTS_DIR: &str = "exports";
/// Container staging root — mirrors the CONTRACTS.md §4 `staging/`
/// layout entry (void_project::layout::STAGING_DIR is crate-private;
/// the name is frozen by the contract).
pub const STAGING_ROOT: &str = "staging";
/// Staging subdirectory prefix; the job id is a UUID so names are scoped.
pub const STAGING_PREFIX: &str = "export-";
/// Provenance record written inside the staged dir before publish.
pub const PROVENANCE_FILE: &str = "provenance.json";
/// Small result card the UI/read views can summarize without parsing the
/// artifact itself.
pub const RESULT_FILE: &str = "result.json";

pub fn exports_dir(root: &Path) -> PathBuf {
    root.join(EXPORTS_DIR)
}

pub fn export_dir(root: &Path, job_id: &str) -> PathBuf {
    exports_dir(root).join(job_id)
}

/// Staging dir for a job — sits inside the container's existing
/// `staging/` tree so recovery's quarantine sweep covers it too.
pub fn staging_dir(root: &Path, job_id: &str) -> PathBuf {
    root.join(STAGING_ROOT)
        .join(format!("{STAGING_PREFIX}{job_id}"))
}

/// List published export job dirs (oldest first by directory order —
/// callers sort by provenance `createdAt` when needed).
pub fn list_exports(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let dir = exports_dir(root);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for e in std::fs::read_dir(&dir)? {
        let e = e?;
        if e.file_type()?.is_dir() {
            out.push(e.path());
        }
    }
    out.sort();
    Ok(out)
}
