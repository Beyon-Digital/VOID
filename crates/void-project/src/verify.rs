//! Checkpoint verification: a checkpoint is usable only when its manifest
//! parses, validates, and every referenced byte re-hashes correctly on
//! disk. Verification is content-based — mtimes are never evidence.

use crate::error::{ProjectError, Result};
use crate::layout;
use crate::manifest::{is_sha256_hex, CheckpointManifest};
use std::fs;
use std::path::{Path, PathBuf};

/// A checkpoint whose manifest and every referenced file verified on disk.
#[derive(Debug, Clone)]
pub struct VerifiedCheckpoint {
    pub checkpoint_id: String,
    pub revision: u64,
    pub dir: PathBuf,
    pub manifest: CheckpointManifest,
    pub manifest_sha256: String,
}

/// Parse + shape-validate a manifest file.
pub fn load_manifest(checkpoint_dir: &Path) -> Result<CheckpointManifest> {
    let bytes = fs::read(checkpoint_dir.join(layout::MANIFEST_FILE))?;
    let m: CheckpointManifest = serde_json::from_slice(&bytes)
        .map_err(|e| ProjectError::ManifestInvalid(format!("parse: {e}")))?;
    m.validate_shape()?;
    Ok(m)
}

/// Verify a manifest against the bytes inside `dir` plus the referenced
/// assets under the project root.
pub fn verify_checkpoint_dir(
    dir: &Path,
    m: &CheckpointManifest,
    project_root: &Path,
) -> Result<()> {
    for r in [&m.engine_snapshot, &m.app_state, &m.command_receipts] {
        let rel = crate::fsutil::safe_rel_path(&r.path)?;
        let file = dir.join(&rel);
        let data = fs::read(&file).map_err(|e| ProjectError::VerifyFailed {
            path: r.path.clone(),
            reason: format!("unreadable: {e}"),
        })?;
        let sha = void_assets::hex_sha256(&data);
        if sha != r.sha256 {
            return Err(ProjectError::VerifyFailed {
                path: r.path.clone(),
                reason: "sha256 mismatch".into(),
            });
        }
        if data.len().to_string() != r.bytes {
            return Err(ProjectError::VerifyFailed {
                path: r.path.clone(),
                reason: "byte length mismatch".into(),
            });
        }
    }
    let store = void_assets::AssetStore::new(
        project_root.join(layout::ASSETS_DIR),
        void_assets::ASSET_IMPORT_MAX_BYTES,
    )?;
    for h in &m.asset_hashes {
        if !is_sha256_hex(h) {
            return Err(ProjectError::VerifyFailed {
                path: format!("assets/sha256/{h}"),
                reason: "not a sha256 hex string".into(),
            });
        }
        store.verify(h).map_err(|e| ProjectError::VerifyFailed {
            path: format!("assets/sha256/{h}"),
            reason: e.to_string(),
        })?;
    }
    Ok(())
}

/// Verify `checkpoints/<id>` under `root` end to end.
pub fn verify_checkpoint(root: &Path, checkpoint_id: &str) -> Result<VerifiedCheckpoint> {
    let dir = layout::checkpoint_dir(root, checkpoint_id);
    if !dir.is_dir() {
        return Err(ProjectError::VerifyFailed {
            path: format!("checkpoints/{checkpoint_id}"),
            reason: "checkpoint dir missing".into(),
        });
    }
    let manifest = load_manifest(&dir)?;
    if manifest.checkpoint_id != checkpoint_id {
        return Err(ProjectError::VerifyFailed {
            path: layout::MANIFEST_FILE.into(),
            reason: "manifest checkpointId != directory name".into(),
        });
    }
    verify_checkpoint_dir(&dir, &manifest, root)?;
    let manifest_sha = void_assets::file_sha256(&dir.join(layout::MANIFEST_FILE))?;
    Ok(VerifiedCheckpoint {
        checkpoint_id: checkpoint_id.into(),
        revision: manifest.revision.parse().unwrap_or(0),
        dir,
        manifest,
        manifest_sha256: manifest_sha,
    })
}
