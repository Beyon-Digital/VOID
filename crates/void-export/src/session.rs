//! Export session — staged output → verified → published (CONTRACTS.md §7).
//!
//! Mirrors the checkpoint save state machine (void-project `SaveSession`):
//!
//! 1. `begin`   — validate spec, verify the immutable checkpoint input on
//!    disk, create `staging/export-<jobId>/` (never authoritative).
//! 2. `render`  — the renderer writes the artifact into staging only.
//! 3. `verify`  — re-read staged bytes: format header, declared channel/
//!    rate/depth and the explicit frame plan; compute sha256.
//! 4. `write_provenance` — params + checkpoint + artifact SHA-256 + tool
//!    versions written inside staging so they publish atomically with it.
//! 5. `publish` — rename staging → `exports/<jobId>/`, fsync; only now is
//!    the result visible. Anything less is `running`/`failed`, never a
//!    fake artifact (T43).
//!
//! `ExportStep` exposes every boundary so crash-injection tests can die
//! between any two steps and prove nothing partial published.

use crate::error::{ExportError, Result};
use crate::layout;
use crate::provenance::{ArtifactRecord, ExportProvenance, ToolRecord};
use crate::renderer::{argv_sha256, CancelToken, RenderOutcome, Renderer};
use crate::spec::{ExportFormat, ExportSpec};
use crate::wav;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Boundaries at which a crash cannot publish a partial export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExportStep {
    StageDir,
    Render,
    Verify,
    Provenance,
    Rename,
    Publish,
}

fn step_name(s: ExportStep) -> &'static str {
    match s {
        ExportStep::StageDir => "staging dir create",
        ExportStep::Render => "renderer invocation",
        ExportStep::Verify => "artifact verification",
        ExportStep::Provenance => "provenance write",
        ExportStep::Rename => "export rename",
        ExportStep::Publish => "publish durability",
    }
}

/// Proof returned only after the export dir is durable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReceipt {
    pub job_id: String,
    pub project_id: String,
    /// Container-relative export dir (e.g. `exports/<jobId>`).
    pub export_dir: String,
    pub file: String,
    pub sha256: String,
    /// Decimal string per §1.
    pub bytes: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frames: Option<String>,
}

pub struct ExportSession {
    root: PathBuf,
    spec: ExportSpec,
    plan: crate::spec::FramePlan,
    checkpoint_dir: PathBuf,
    out_name: String,
    outcome: Option<RenderOutcome>,
    artifact: Option<ArtifactRecord>,
    argv_hash: String,
    renderer_name: String,
    exe_name: Option<String>,
    fail_after: Option<ExportStep>,
}

impl ExportSession {
    /// Validate the spec, verify the immutable checkpoint input and create
    /// the scoped staging dir. The checkpoint must verify end-to-end —
    /// a render of an unverified snapshot is not a contract render.
    pub fn begin(root: &Path, spec: ExportSpec) -> Result<Self> {
        spec.validate()?;
        let verified = void_project::verify_checkpoint(root, &spec.checkpoint_id)
            .map_err(|e| ExportError::CheckpointInvalid(e.to_string()))?;
        if verified.manifest.project_id != spec.project_id {
            return Err(ExportError::CheckpointInvalid(
                "checkpoint projectId != spec projectId".into(),
            ));
        }
        // The pinned input is immutable by construction: edits after the
        // render starts land in a NEW checkpoint, never in this dir.
        let staging = layout::staging_dir(root, &spec.job_id);
        if staging.exists() {
            return Err(ExportError::InvalidSpec(format!(
                "export staging id collision: {}",
                spec.job_id
            )));
        }
        fs::create_dir_all(&staging)?;
        void_assets::sync_dir(
            staging
                .parent()
                .ok_or_else(|| ExportError::InvalidSpec("staging root".into()))?,
        )?;
        fs::create_dir_all(layout::exports_dir(root))?;
        let out_name = spec.output_file_name()?;
        let plan = spec.frame_plan()?;
        Ok(Self {
            root: root.to_path_buf(),
            spec,
            plan,
            checkpoint_dir: verified.dir,
            out_name,
            outcome: None,
            artifact: None,
            argv_hash: String::new(),
            renderer_name: "unknown".into(),
            exe_name: None,
            fail_after: None,
        })
    }

    /// Test hook: after `step` finishes its disk effects, report
    /// `Failpoint` — modelling a crash at that exact boundary.
    pub fn set_failpoint(&mut self, step: ExportStep) {
        self.fail_after = Some(step);
    }

    fn trip(&self, step: ExportStep) -> Result<()> {
        if self.fail_after == Some(step) {
            return Err(ExportError::Failpoint(step_name(step)));
        }
        Ok(())
    }

    /// Invoke the renderer. It may only write inside the staging dir —
    /// the artifact must appear under `out_name`.
    pub fn render(&mut self, renderer: &dyn Renderer, cancel: &CancelToken) -> Result<()> {
        self.trip(ExportStep::StageDir)?;
        self.renderer_name = renderer_name(renderer);
        self.exe_name = renderer_exe(renderer);
        // Record the argv hash for ArgvRenderer before it runs so a crash
        // mid-render still leaves invocation evidence for forensics.
        if let Some(r) = renderer
            .as_any()
            .and_then(|a| a.downcast_ref::<crate::renderer::ArgvRenderer>())
        {
            let argv = r.argv_for(
                &self.spec,
                &self.plan,
                &self.checkpoint_dir,
                &layout::staging_dir(&self.root, &self.spec.job_id),
                &self.out_name,
            );
            self.argv_hash = argv_sha256(&argv);
        }
        let outcome = renderer.render(
            &self.spec,
            &self.plan,
            &self.checkpoint_dir,
            &layout::staging_dir(&self.root, &self.spec.job_id),
            &self.out_name,
            cancel,
        )?;
        if outcome.file != self.out_name {
            return Err(ExportError::RendererFailed(format!(
                "renderer produced {:?}, expected {:?}",
                outcome.file, self.out_name
            )));
        }
        self.trip(ExportStep::Render)?;
        self.outcome = Some(outcome);
        Ok(())
    }

    /// Re-read the staged artifact and prove it matches the declared spec
    /// — bytes, channels, rate, depth and the explicit total frames.
    /// Never trust a zero-exit code as evidence of a completed render.
    pub fn verify(&mut self) -> Result<()> {
        if self.outcome.is_none() {
            return Err(ExportError::VerifyFailed("verify before render".into()));
        }
        let staging = layout::staging_dir(&self.root, &self.spec.job_id);
        let file = staging.join(&self.out_name);
        let data = fs::read(&file).map_err(|e| ExportError::VerifyFailed(format!("read: {e}")))?;
        if data.is_empty() {
            return Err(ExportError::VerifyFailed("artifact is empty".into()));
        }
        let total_frames: u64 = self.plan.total_frames.parse().unwrap_or(0);
        let frames = match self.spec.format {
            ExportFormat::Wav => {
                let info = wav::verify_wav(&file, &self.spec, total_frames)?;
                Some(info.data_frames.to_string())
            }
            ExportFormat::Midi => {
                wav::verify_midi(&file)?;
                None
            }
        };
        let sha = void_assets::hex_sha256(&data);
        self.trip(ExportStep::Verify)?;
        self.artifact = Some(ArtifactRecord {
            file: self.out_name.clone(),
            sha256: sha,
            bytes: data.len().to_string(),
            frames,
            declared_total_frames: self.plan.total_frames.clone(),
        });
        Ok(())
    }

    /// Write `provenance.json` inside staging — moves with the rename.
    pub fn write_provenance(&mut self) -> Result<()> {
        let artifact = self
            .artifact
            .clone()
            .ok_or_else(|| ExportError::VerifyFailed("provenance before verify".into()))?;
        let outcome = self.outcome.clone().unwrap_or(RenderOutcome {
            file: String::new(),
            tool_versions: Vec::new(),
            warnings: Vec::new(),
        });
        let prov = ExportProvenance {
            format_version: 1,
            job_id: self.spec.job_id.clone(),
            project_id: self.spec.project_id.clone(),
            checkpoint_id: self.spec.checkpoint_id.clone(),
            source_revision: self.spec.source_revision.clone(),
            asset_hashes: self.spec.asset_hashes.clone(),
            spec: self.spec.clone(),
            frame_plan: self.plan.clone(),
            artifact,
            tool: ToolRecord {
                renderer: self.renderer_name.clone(),
                executable: self.exe_name.clone(),
                argv_sha256: self.argv_hash.clone(),
                versions: outcome.tool_versions,
            },
            created_at: utc_now(),
            result: "succeeded".into(),
        };
        let bytes = serde_json::to_vec_pretty(&prov)?;
        let staging = layout::staging_dir(&self.root, &self.spec.job_id);
        void_assets::write_atomic(&staging.join(layout::PROVENANCE_FILE), &bytes)?;
        // The result card for read views/UI lists.
        let card = serde_json::json!({
            "kind": "ExportResult",
            "jobId": self.spec.job_id,
            "status": "succeeded",
            "file": prov.artifact.file,
            "sha256": prov.artifact.sha256,
            "bytes": prov.artifact.bytes,
            "frames": prov.artifact.frames,
            "warnings": outcome.warnings,
        });
        void_assets::write_atomic(
            &staging.join(layout::RESULT_FILE),
            &serde_json::to_vec_pretty(&card)?,
        )?;
        self.trip(ExportStep::Provenance)?;
        Ok(())
    }

    /// Rename staging → `exports/<jobId>` and fsync — the only moment the
    /// export becomes visible. Refuses to overwrite an existing dir.
    pub fn publish(&mut self) -> Result<ExportReceipt> {
        let staging = layout::staging_dir(&self.root, &self.spec.job_id);
        let dest = layout::export_dir(&self.root, &self.spec.job_id);
        if dest.exists() {
            return Err(ExportError::InvalidSpec(format!(
                "export dir already exists: {}",
                self.spec.job_id
            )));
        }
        fs::rename(&staging, &dest).map_err(|_| {
            ExportError::VerifyFailed("export rename failed (cross-fs or busy dir)".into())
        })?;
        void_assets::sync_dir(&layout::exports_dir(&self.root))?;
        self.trip(ExportStep::Rename)?;
        // Verify the published tree one last time: the bytes that shipped
        // are the bytes that were verified.
        let a = self
            .artifact
            .clone()
            .ok_or_else(|| ExportError::VerifyFailed("publish before verify".into()))?;
        let shipped = dest.join(&a.file);
        let actual = void_assets::file_sha256(&shipped)?;
        if actual != a.sha256 {
            return Err(ExportError::VerifyFailed(
                "published artifact hash != staged hash".into(),
            ));
        }
        void_assets::sync_dir(&self.root)?;
        self.trip(ExportStep::Publish)?;
        Ok(ExportReceipt {
            job_id: self.spec.job_id.clone(),
            project_id: self.spec.project_id.clone(),
            export_dir: format!("{}/{}", layout::EXPORTS_DIR, self.spec.job_id),
            file: a.file,
            sha256: a.sha256,
            bytes: a.bytes,
            frames: a.frames,
        })
    }

    /// Discard the staged export — explicit abort/cancel path. Staging is
    /// never authoritative, so removal is the documented cleanup policy
    /// (T43): an incomplete export is never presented as a result.
    pub fn abort(self) -> Result<()> {
        let dir = layout::staging_dir(&self.root, &self.spec.job_id);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        Ok(())
    }

    /// Container-relative artifact path as the job result should report.
    pub fn artifact_rel(&self) -> Option<String> {
        self.artifact
            .as_ref()
            .map(|a| format!("exports/{}/{}", self.spec.job_id, a.file))
    }

    pub fn spec(&self) -> &ExportSpec {
        &self.spec
    }
}

fn renderer_name(r: &dyn Renderer) -> String {
    r.name().unwrap_or("renderer").to_string()
}
fn renderer_exe(r: &dyn Renderer) -> Option<String> {
    r.exe_name().map(|s| s.to_string())
}

fn utc_now() -> String {
    // Reuse the project convention (RFC3339 via `time` when present) —
    // void-project::fsutil::utc_now is crate-private; keep a minimal copy
    // to avoid a new dependency for one formatting call.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // civil-from-days (Howard Hinnant) — no chrono dependency.
    let days = secs / 86_400;
    let rem = secs % 86_400;
    let (y, m, d) = civil(days as i64);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
