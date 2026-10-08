//! Job paths inside the project container (mirrors void-export layout).
//!
//! `staging/job-<jobId>/` — worker's only write scope, never
//! authoritative. `jobs/<jobId>/` — published provenance + result card
//! once artifacts are imported into the content-addressed store.

use std::path::{Path, PathBuf};

/// Published job metadata lives here (provenance.json + result.json).
pub const JOBS_DIR: &str = "jobs";
/// Container staging root (contract §4).
pub const STAGING_ROOT: &str = "staging";
/// Staging subdirectory prefix for AI jobs.
pub const STAGING_PREFIX: &str = "job-";
/// Provenance record inside the published job dir.
pub const PROVENANCE_FILE: &str = "provenance.json";
/// Result card the UI/read views summarize.
pub const RESULT_FILE: &str = "result.json";
/// Project asset store root (content-addressed outputs land here).
pub const ASSETS_DIR: &str = "assets";

pub fn jobs_dir(root: &Path) -> PathBuf {
    root.join(JOBS_DIR)
}

pub fn job_dir(root: &Path, job_id: &str) -> PathBuf {
    jobs_dir(root).join(job_id)
}

pub fn staging_dir(root: &Path, job_id: &str) -> PathBuf {
    root.join(STAGING_ROOT)
        .join(format!("{STAGING_PREFIX}{job_id}"))
}

/// `assets/sha256/<hash>.<ext>` — container-relative result path the
/// UI resolves to the asset store.
pub fn asset_rel(sha256: &str, ext: &str) -> String {
    format!("{ASSETS_DIR}/sha256/{sha256}.{ext}")
}

/// List `staging/job-*` dirs — orphan sweep input.
pub fn list_job_staging(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let dir = root.join(STAGING_ROOT);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for e in std::fs::read_dir(&dir)? {
        let e = e?;
        if e.file_type()?.is_dir() && e.file_name().to_string_lossy().starts_with(STAGING_PREFIX) {
            out.push(e.path());
        }
    }
    out.sort();
    Ok(out)
}
