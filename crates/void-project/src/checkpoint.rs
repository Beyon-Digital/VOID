//! Checkpoint save state machine (CONTRACTS.md §4):
//!
//! 1. Caller holds the serialized mutation barrier and captures the
//!    engine/app/receipt boundary at revision R (audio keeps running).
//! 2. Payloads are written under `staging/<checkpointId>/`, flushed and
//!    verified.
//! 3. The complete manifest is written and flushed.
//! 4. The staging dir is renamed into `checkpoints/` on the same
//!    filesystem and made durable.
//! 5. `CURRENT` is atomically replaced with `{checkpointId, manifestSha256}`
//!    and made durable — only now is the save `SAVE_DURABLE(R)`.
//! 6. The app DB reconciles afterwards; it is an index, never the authority.
//!
//! `SaveSession` exposes each boundary explicitly so crash-injection tests
//! (T17) can die between any two steps; `save()` runs them in order.

use crate::current::{self, CurrentPointer};
use crate::error::{ProjectError, Result};
use crate::layout;
use crate::manifest::{CheckpointManifest, FileRef, MANIFEST_STATE_COMPLETE};
use crate::verify;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Everything the writer needs to publish one complete checkpoint. All
/// bytes were captured immutably at the mutation barrier by the caller —
/// `engine_snapshot` comes from a `SnapshotProvider` (the engine worker
/// over the control protocol; see lane A `native/void-engine`).
#[derive(Debug, Clone)]
pub struct SnapshotBundle {
    pub engine_snapshot: Vec<u8>,
    pub app_state: Vec<u8>,
    /// Serialized committed receipt records (see `receipts` module).
    pub command_receipts: Vec<u8>,
    /// Hashes of immutable assets this checkpoint references. Each must
    /// already be verified-present in `assets/sha256/`.
    pub asset_hashes: Vec<String>,
}

/// Engine-side serialization capture, invoked OFF the audio callback at
/// the mutation barrier. Implemented by the engine bridge; tests mock it.
pub trait SnapshotProvider {
    fn capture(&self, revision: u64) -> std::io::Result<Vec<u8>>;
}

/// Save boundaries at which a crash is survivable-by-construction. Tests
/// inject `Failpoint` after each boundary's disk effects to prove the
/// last verified checkpoint always survives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveStep {
    StageDir,
    EngineSnapshot,
    AppState,
    Receipts,
    Manifest,
    RenameCheckpoint,
    PublishCurrent,
}

/// Proof returned only after CURRENT is durable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveReceipt {
    pub revision: u64,
    pub checkpoint_id: String,
    pub manifest_sha256: String,
}

pub struct SaveRequest {
    pub project_id: String,
    pub revision: u64,
    pub engine_revision: String,
    pub parent_checkpoint_id: Option<String>,
}

pub struct SaveSession {
    root: PathBuf,
    checkpoint_id: String,
    request: SaveRequest,
    /// Asset hash list captured during `stage()` for the manifest.
    staged_asset_hashes: Option<Vec<String>>,
    manifest: Option<CheckpointManifest>,
    fail_after: Option<SaveStep>,
}

impl SaveSession {
    /// Create `staging/<checkpointId>/`. Fails if the staging name exists.
    pub fn begin(root: &Path, request: SaveRequest) -> Result<Self> {
        let checkpoint_id = void_protocol::ids::new_id();
        Self::begin_with_id(root, request, checkpoint_id)
    }

    /// Deterministic-id variant (tests/traceability).
    pub fn begin_with_id(root: &Path, request: SaveRequest, checkpoint_id: String) -> Result<Self> {
        let dir = layout::staging_dir(root, &checkpoint_id);
        if dir.exists() {
            return Err(ProjectError::ManifestInvalid(format!(
                "staging id collision: {checkpoint_id}"
            )));
        }
        fs::create_dir_all(&dir)?;
        void_assets::sync_dir(&dir)?;
        Ok(Self {
            root: root.to_path_buf(),
            checkpoint_id,
            request,
            staged_asset_hashes: None,
            manifest: None,
            fail_after: None,
        })
    }

    /// Test hook: after `step` completes its disk effects, return
    /// `Failpoint` — modelling a crash at that exact boundary.
    pub fn set_failpoint(&mut self, step: SaveStep) {
        self.fail_after = Some(step);
    }

    fn trip(&self, step: SaveStep) -> Result<()> {
        if self.fail_after == Some(step) {
            return Err(ProjectError::Failpoint(step_name(step)));
        }
        Ok(())
    }

    /// Write + flush + verify all three payload files into staging.
    pub fn stage(&mut self, bundle: &SnapshotBundle) -> Result<()> {
        let dir = layout::staging_dir(&self.root, &self.checkpoint_id);
        self.trip(SaveStep::StageDir)?;
        self.stage_file(&dir, layout::ENGINE_SNAPSHOT_FILE, &bundle.engine_snapshot)?;
        self.trip(SaveStep::EngineSnapshot)?;
        self.stage_file(&dir, layout::APP_STATE_FILE, &bundle.app_state)?;
        self.trip(SaveStep::AppState)?;
        self.stage_file(&dir, layout::RECEIPTS_FILE, &bundle.command_receipts)?;
        self.trip(SaveStep::Receipts)?;
        self.staged_asset_hashes = Some(bundle.asset_hashes.clone());
        Ok(())
    }

    fn stage_file(&self, dir: &Path, name: &str, data: &[u8]) -> Result<FileRef> {
        void_assets::write_atomic(&dir.join(name), data)?;
        Ok(FileRef {
            path: name.into(),
            sha256: void_assets::hex_sha256(data),
            bytes: data.len().to_string(),
        })
    }

    /// Compute and write `manifest.json`, then verify every staged byte on
    /// disk against it — publication is refused if anything mismatches.
    pub fn write_manifest(&mut self) -> Result<CheckpointManifest> {
        let dir = layout::staging_dir(&self.root, &self.checkpoint_id);
        let file_ref = |name: &str| -> Result<FileRef> {
            let data = fs::read(dir.join(name))?;
            Ok(FileRef {
                path: name.into(),
                sha256: void_assets::hex_sha256(&data),
                bytes: data.len().to_string(),
            })
        };
        let manifest = CheckpointManifest {
            format_version: 1,
            project_id: self.request.project_id.clone(),
            checkpoint_id: self.checkpoint_id.clone(),
            parent_checkpoint_id: self.request.parent_checkpoint_id.clone(),
            revision: self.request.revision.to_string(),
            engine_revision: self.request.engine_revision.clone(),
            created_at: crate::fsutil::utc_now(),
            engine_snapshot: file_ref(layout::ENGINE_SNAPSHOT_FILE)?,
            app_state: file_ref(layout::APP_STATE_FILE)?,
            command_receipts: file_ref(layout::RECEIPTS_FILE)?,
            asset_hashes: self.staged_asset_hashes.clone().unwrap_or_default(),
            manifest_state: MANIFEST_STATE_COMPLETE.into(),
        };
        manifest.validate_shape()?;
        let bytes = serde_json::to_vec_pretty(&manifest)?;
        void_assets::write_atomic(&dir.join(layout::MANIFEST_FILE), &bytes)?;
        // Verify the staged tree matches the manifest before allowing publish.
        verify::verify_checkpoint_dir(&dir, &manifest, &self.root)?;
        self.trip(SaveStep::Manifest)?;
        self.manifest = Some(manifest.clone());
        Ok(manifest)
    }

    /// Rename staging → checkpoints/<id>, fsync, then atomically replace
    /// CURRENT. Returns the durable receipt. Nothing is durable before this.
    pub fn publish(&mut self) -> Result<SaveReceipt> {
        if self.manifest.is_none() {
            return Err(ProjectError::SaveFailed {
                step: "publish before manifest",
            });
        }
        let staging = layout::staging_dir(&self.root, &self.checkpoint_id);
        let dest = layout::checkpoint_dir(&self.root, &self.checkpoint_id);
        fs::rename(&staging, &dest).map_err(|_| ProjectError::SaveFailed {
            step: "checkpoint rename",
        })?;
        void_assets::sync_dir(&layout::checkpoints_dir(&self.root))?;
        self.trip(SaveStep::RenameCheckpoint)?;

        let manifest_bytes = fs::read(dest.join(layout::MANIFEST_FILE))?;
        let manifest_sha = void_assets::hex_sha256(&manifest_bytes);
        current::write_current(
            &self.root,
            &CurrentPointer {
                checkpoint_id: self.checkpoint_id.clone(),
                manifest_sha256: manifest_sha.clone(),
            },
        )?;
        void_assets::sync_dir(&self.root)?;
        self.trip(SaveStep::PublishCurrent)?;

        Ok(SaveReceipt {
            revision: self.request.revision,
            checkpoint_id: self.checkpoint_id.clone(),
            manifest_sha256: manifest_sha,
        })
    }

    /// Discard the staged checkpoint — explicit abort path.
    pub fn abort(self) -> Result<()> {
        let dir = layout::staging_dir(&self.root, &self.checkpoint_id);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }

    pub fn checkpoint_id(&self) -> &str {
        &self.checkpoint_id
    }
}

fn step_name(s: SaveStep) -> &'static str {
    match s {
        SaveStep::StageDir => "staging dir write",
        SaveStep::EngineSnapshot => "engine snapshot write",
        SaveStep::AppState => "app-state write",
        SaveStep::Receipts => "receipts write",
        SaveStep::Manifest => "manifest write",
        SaveStep::RenameCheckpoint => "checkpoint rename",
        SaveStep::PublishCurrent => "CURRENT publish",
    }
}

/// Full save in one call: begin → stage → manifest → publish. On any
/// failure the staging dir is cleaned and nothing becomes durable.
pub fn save(root: &Path, request: SaveRequest, bundle: &SnapshotBundle) -> Result<SaveReceipt> {
    let mut s = SaveSession::begin(root, request)?;
    if let Err(e) = s.stage(bundle).and_then(|_| s.write_manifest()) {
        let _ = s.abort();
        return Err(e);
    }
    s.publish()
}
