//! Container layout (CONTRACTS.md §4). The `.void` directory is the only
//! authority; every path here is relative to the project root.

use std::path::{Path, PathBuf};

pub const PROJECT_FILE: &str = "project.json";
pub const CURRENT_FILE: &str = "CURRENT";
pub const ASSETS_DIR: &str = "assets";
pub const SHA256_DIR: &str = "assets/sha256";
pub const CHECKPOINTS_DIR: &str = "checkpoints";
pub const STAGING_DIR: &str = "staging";
pub const RECORDINGS_DIR: &str = "recordings";
pub const MIGRATIONS_DIR: &str = "migrations";
/// Incomplete staging dirs are moved here on recovery (never deleted blind).
pub const QUARANTINE_DIR: &str = "staging/_quarantine";

pub const MANIFEST_FILE: &str = "manifest.json";
pub const ENGINE_SNAPSHOT_FILE: &str = "engine.tracktionedit";
pub const APP_STATE_FILE: &str = "app-state.json";
pub const RECEIPTS_FILE: &str = "command-receipts.json";

pub fn project_file(root: &Path) -> PathBuf {
    root.join(PROJECT_FILE)
}
pub fn current_file(root: &Path) -> PathBuf {
    root.join(CURRENT_FILE)
}
pub fn staging_dir(root: &Path, checkpoint_id: &str) -> PathBuf {
    root.join(STAGING_DIR).join(checkpoint_id)
}
pub fn checkpoints_dir(root: &Path) -> PathBuf {
    root.join(CHECKPOINTS_DIR)
}
pub fn checkpoint_dir(root: &Path, checkpoint_id: &str) -> PathBuf {
    root.join(CHECKPOINTS_DIR).join(checkpoint_id)
}
pub fn recordings_dir(root: &Path) -> PathBuf {
    root.join(RECORDINGS_DIR)
}
pub fn quarantine_dir(root: &Path) -> PathBuf {
    root.join(QUARANTINE_DIR)
}
pub fn migrations_dir(root: &Path) -> PathBuf {
    root.join(MIGRATIONS_DIR)
}

/// True when `root` looks like a VOID project container.
pub fn is_project_root(root: &Path) -> bool {
    project_file(root).is_file()
}
