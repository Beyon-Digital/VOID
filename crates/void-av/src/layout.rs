//! Directory layout for av exports. Mirrors `void_export::layout`
//! (staging inside project root, atomic rename into `exports/`).
//!
//! av jobs stage under `staging/av-<jobId>` and publish to
//! `exports/av-<jobId>/` so av artifacts never collide with audio
//! exports from the same project.

pub const EXPORTS_DIR: &str = "exports";
pub const STAGING_ROOT: &str = "staging";
pub const STAGING_PREFIX: &str = "av-";
pub const PUBLISH_PREFIX: &str = "av-";
pub const PROVENANCE_FILE: &str = "provenance.json";
pub const RESULT_FILE: &str = "result.json";

use std::path::PathBuf;

pub fn staging_dir(project_root: &std::path::Path, job_id: &str) -> PathBuf {
    project_root
        .join(STAGING_ROOT)
        .join(format!("{STAGING_PREFIX}{job_id}"))
}

pub fn export_dir(project_root: &std::path::Path, job_id: &str) -> PathBuf {
    project_root
        .join(EXPORTS_DIR)
        .join(format!("{PUBLISH_PREFIX}{job_id}"))
}
