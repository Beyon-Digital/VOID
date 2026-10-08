//! Checkpoint manifest — the save authority (CONTRACTS.md §4, schema
//! `checkpoint/1.0.0`). Large integers are decimal strings in JSON per §1.

use serde::{Deserialize, Serialize};

pub const MANIFEST_STATE_COMPLETE: &str = "complete";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileRef {
    /// Container-relative path inside the checkpoint dir; validated against
    /// traversal before use (see fsutil::safe_rel_path).
    pub path: String,
    pub sha256: String,
    /// Decimal string per §1 (no float timestamps/sizes).
    pub bytes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CheckpointManifest {
    pub format_version: u32,
    pub project_id: String,
    pub checkpoint_id: String,
    pub parent_checkpoint_id: Option<String>,
    /// Coordinator revision R — decimal string.
    pub revision: String,
    pub engine_revision: String,
    pub created_at: String,
    pub engine_snapshot: FileRef,
    pub app_state: FileRef,
    pub command_receipts: FileRef,
    #[serde(default)]
    pub asset_hashes: Vec<String>,
    pub manifest_state: String,
}

impl CheckpointManifest {
    /// Schema-level sanity checks (shape only — file hashes are verified
    /// separately by `verify::verify_checkpoint`).
    pub fn validate_shape(&self) -> crate::error::Result<()> {
        use crate::error::ProjectError;
        let bad = |m: &str| ProjectError::ManifestInvalid(m.to_string());
        if self.format_version != 1 {
            return Err(bad("formatVersion must be 1"));
        }
        if self.manifest_state != MANIFEST_STATE_COMPLETE {
            return Err(bad("manifestState must be \"complete\""));
        }
        if !void_protocol::ids::is_valid_id(&self.project_id) {
            return Err(bad("projectId is not a valid UUID"));
        }
        if !void_protocol::ids::is_valid_id(&self.checkpoint_id) {
            return Err(bad("checkpointId is not a valid UUID"));
        }
        if let Some(p) = &self.parent_checkpoint_id {
            if !void_protocol::ids::is_valid_id(p) {
                return Err(bad("parentCheckpointId is not a valid UUID"));
            }
        }
        if self.revision.parse::<u64>().is_err() {
            return Err(bad("revision is not a decimal u64"));
        }
        for r in [
            &self.engine_snapshot,
            &self.app_state,
            &self.command_receipts,
        ] {
            crate::fsutil::safe_rel_path(&r.path)
                .map_err(|_| bad(&format!("unsafe manifest path {:?}", r.path)))?;
            if !is_sha256_hex(&r.sha256) {
                return Err(bad("file ref sha256 is not 64 lowercase hex"));
            }
            if r.bytes.parse::<u64>().is_err() {
                return Err(bad("file ref bytes is not a decimal u64"));
            }
        }
        for h in &self.asset_hashes {
            if !is_sha256_hex(h) {
                return Err(bad("assetHashes entry is not sha256 hex"));
            }
        }
        Ok(())
    }
}

pub fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}
