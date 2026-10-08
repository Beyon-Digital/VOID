//! Container format migration (CONTRACTS.md §4, T20).
//!
//! - `open` never silently upgrades: format < current returns
//!   `NeedsMigration`; the caller runs `migrate()` explicitly.
//! - Migration is copy-on-write: every file that would change is first
//!   copied byte-exact into `migrations/v<from>-backup-<n>/`, then the
//!   upgraded content is written and verified; `rollback` restores the
//!   preserved bytes and re-verifies them.
//! - Format > current is newer-unknown: read-only open or refused; the
//!   original bytes are never overwritten.

use crate::error::{ProjectError, Result};
use crate::layout;
use crate::meta::{ProjectMeta, FORMAT_VERSION};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenMode {
    Read,
    Write,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenOutcome {
    /// Container is at the current format and usable.
    Current(ProjectMeta),
    /// Older supported format — writable only after `migrate()`.
    NeedsMigration { found: u32 },
    /// Newer-than-known format. Read access allowed; write refused and
    /// original bytes are never touched.
    ReadOnlyNewer { found: u32 },
}

/// Inspect `project.json` without mutating anything.
pub fn inspect(root: &Path) -> Result<OpenOutcome> {
    let meta = read_meta(root)?;
    if meta.format_version > FORMAT_VERSION {
        return Ok(OpenOutcome::ReadOnlyNewer {
            found: meta.format_version,
        });
    }
    if meta.format_version < FORMAT_VERSION {
        // Older containers are migration candidates. FORMAT_READ_MIN is
        // the floor for a future "too old to migrate" refusal — today it
        // is 0, so every older version qualifies.
        return Ok(OpenOutcome::NeedsMigration {
            found: meta.format_version,
        });
    }
    Ok(OpenOutcome::Current(meta))
}

/// Open a container honouring the version policy.
pub fn open(root: &Path, mode: OpenMode) -> Result<OpenOutcome> {
    match inspect(root)? {
        OpenOutcome::ReadOnlyNewer { found } => match mode {
            OpenMode::Read => Ok(OpenOutcome::ReadOnlyNewer { found }),
            OpenMode::Write => Err(ProjectError::UnsupportedNewer {
                found,
                supported_max: FORMAT_VERSION,
            }),
        },
        other => Ok(other),
    }
}

/// Create a new empty container (v1 layout) at `root`.
pub fn create(root: &Path, meta: &ProjectMeta) -> Result<()> {
    if layout::project_file(root).exists() {
        return Err(ProjectError::ManifestInvalid(
            "project.json already exists".into(),
        ));
    }
    for d in [
        layout::CHECKPOINTS_DIR,
        layout::STAGING_DIR,
        layout::RECORDINGS_DIR,
        layout::ASSETS_DIR,
        layout::SHA256_DIR,
    ] {
        fs::create_dir_all(root.join(d))?;
    }
    let bytes = serde_json::to_vec_pretty(meta)?;
    void_assets::write_atomic(&layout::project_file(root), &bytes)?;
    void_assets::sync_dir(root)?;
    Ok(())
}

fn read_meta(root: &Path) -> Result<ProjectMeta> {
    let bytes = fs::read(layout::project_file(root))
        .map_err(|_| ProjectError::NotAProject("project.json missing".into()))?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Record of a migration for verified rollback.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Migration {
    pub from_version: u32,
    pub to_version: u32,
    pub backup_dir: PathBuf,
    /// `(container-relative path, sha256-of-preserved-bytes)`
    pub preserved: Vec<(String, String)>,
}

/// Boundaries at which a migration can be killed for T20.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrateStep {
    AfterBackup,
    AfterRewrite,
}

/// Copy-on-write migrate to the current format. `fail_after` injects a
/// crash at a named boundary for tests.
pub fn migrate(root: &Path, fail_after: Option<MigrateStep>) -> Result<Migration> {
    let meta = read_meta(root)?;
    let from = meta.format_version;
    match inspect(root)? {
        OpenOutcome::NeedsMigration { .. } => {}
        OpenOutcome::Current(_) => {
            return Err(ProjectError::ManifestInvalid(
                "already current format".into(),
            ))
        }
        OpenOutcome::ReadOnlyNewer { found } => {
            return Err(ProjectError::UnsupportedNewer {
                found,
                supported_max: FORMAT_VERSION,
            })
        }
    }

    // 1. Preserve original bytes.
    let backup_dir = fresh_backup_dir(root, from);
    fs::create_dir_all(&backup_dir)?;
    let mut preserved = Vec::new();
    let rel = layout::PROJECT_FILE;
    let src = root.join(rel);
    let bytes = fs::read(&src)?;
    let dest = backup_dir.join(rel);
    if let Some(p) = dest.parent() {
        fs::create_dir_all(p)?;
    }
    fs::write(&dest, &bytes)?;
    preserved.push((rel.to_string(), void_assets::hex_sha256(&bytes)));
    if fail_after == Some(MigrateStep::AfterBackup) {
        return Err(ProjectError::Failpoint("migration after backup"));
    }

    // 2. Rewrite only what the format bump requires, via atomic writes.
    let mut new_meta = meta;
    new_meta.format_version = FORMAT_VERSION;
    let bytes = serde_json::to_vec_pretty(&new_meta)?;
    void_assets::write_atomic(&layout::project_file(root), &bytes)?;
    // Container dirs the current format requires.
    for d in [
        layout::CHECKPOINTS_DIR,
        layout::STAGING_DIR,
        layout::RECORDINGS_DIR,
        layout::SHA256_DIR,
    ] {
        fs::create_dir_all(root.join(d))?;
    }
    // Verify the rewrite landed byte-exact.
    let on_disk = fs::read(layout::project_file(root))?;
    if on_disk != bytes {
        return Err(ProjectError::VerifyFailed {
            path: layout::PROJECT_FILE.into(),
            reason: "post-migration bytes differ from verified write".into(),
        });
    }
    if fail_after == Some(MigrateStep::AfterRewrite) {
        return Err(ProjectError::Failpoint("migration after rewrite"));
    }
    void_assets::sync_dir(&layout::migrations_dir(root))?;
    Ok(Migration {
        from_version: from,
        to_version: FORMAT_VERSION,
        backup_dir,
        preserved,
    })
}

/// Restore preserved bytes exactly; verifies each restored file's hash.
pub fn rollback(root: &Path, m: &Migration) -> Result<()> {
    for (rel, sha) in &m.preserved {
        let preserved_bytes = fs::read(m.backup_dir.join(rel))?;
        if void_assets::hex_sha256(&preserved_bytes) != *sha {
            return Err(ProjectError::VerifyFailed {
                path: rel.clone(),
                reason: "backup copy corrupt; refusing rollback".into(),
            });
        }
        void_assets::write_atomic(&root.join(rel), &preserved_bytes)?;
        let after = void_assets::file_sha256(&root.join(rel))?;
        if &after != sha {
            return Err(ProjectError::VerifyFailed {
                path: rel.clone(),
                reason: "rollback restore verification failed".into(),
            });
        }
    }
    Ok(())
}

fn fresh_backup_dir(root: &Path, from: u32) -> PathBuf {
    let base = layout::migrations_dir(root);
    let mut n = 0;
    loop {
        let d = base.join(format!("v{from}-backup-{n}"));
        if !d.exists() {
            return d;
        }
        n += 1;
    }
}
