//! `CURRENT` — the atomic publication pointer. Publication is a tmp-file
//! rename + directory fsync; nothing else counts as durable.

use crate::error::{ProjectError, Result};
use crate::layout;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CurrentPointer {
    pub checkpoint_id: String,
    pub manifest_sha256: String,
}

pub fn read_current(root: &Path) -> Result<Option<CurrentPointer>> {
    let path = layout::current_file(root);
    if !path.exists() {
        return Ok(None);
    }
    let bytes = std::fs::read(&path)?;
    let ptr: CurrentPointer = serde_json::from_slice(&bytes)
        .map_err(|e| ProjectError::PointerCorrupt(format!("unparseable CURRENT: {e}")))?;
    if !void_protocol::ids::is_valid_id(&ptr.checkpoint_id)
        || !crate::manifest::is_sha256_hex(&ptr.manifest_sha256)
    {
        return Err(ProjectError::PointerCorrupt(
            "CURRENT fields fail shape validation".into(),
        ));
    }
    Ok(Some(ptr))
}

/// Publish the pointer atomically (tmp write + fsync + rename + dir fsync).
pub fn write_current(root: &Path, ptr: &CurrentPointer) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(ptr)?;
    void_assets::write_atomic(&layout::current_file(root), &bytes)?;
    Ok(())
}
