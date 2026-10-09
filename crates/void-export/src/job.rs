//! Job glue — export sessions run inside the void-jobs state machine
//! (CONTRACTS.md §6): queued → running → succeeded | failed | cancelling
//! → cancelled. The JobDb row is the status authority; the filesystem is
//! the artifact authority; neither reports success without the other.
//!
//! Cancellation policy (T43): `cancel()` flips the job to `cancelling`
//! (or `cancelled` when still queued) and trips the CancelToken; the
//! runner then aborts staging, finishes the cancel and never publishes.
//! A result that arrives after cancel/terminal state is quarantined by
//! `JobDb::record_result` — late output cannot become a fake artifact.

use crate::error::{ExportError, Result};
use crate::renderer::{CancelToken, Renderer};
use crate::session::ExportSession;
use crate::spec::ExportSpec;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use void_jobs::{JobDb, JobSpec, JobStatus, Reservations};

/// Map an export spec onto the §6 job spec fields. `parameters` carries
/// the whole spec echo (validated params); `context_sha256` binds the job
/// to the immutable checkpoint content hash.
pub fn job_spec_for(spec: &ExportSpec, checkpoint_manifest_sha: &str) -> Result<JobSpec> {
    Ok(JobSpec {
        job_id: spec.job_id.clone(),
        project_id: spec.project_id.clone(),
        source_revision: spec.source_revision.clone(),
        context_sha256: checkpoint_manifest_sha.to_string(),
        kind: void_jobs::JobKind::AvExport,
        runtime_id: "void-export".into(),
        // void-export has no external runtime artifact; the crate version
        // hash field carries the spec hash instead so drift is visible.
        runtime_sha256: crate_sha256(),
        model_id: None,
        model_sha256: None,
        inputs: spec.asset_hashes.clone(),
        parameters: serde_json::to_value(spec)?,
        reservations: Reservations {
            // A render is CPU-bound disk IO, not an AI job — keep the
            // conservative small-job reservation of the §2 policy.
            ram_bytes: "536870912".into(),
            vram_bytes: "0".into(),
            cpu_threads: 1,
        },
        deadline_monotonic_ns: spec
            .deadline_monotonic_ns
            .clone()
            .unwrap_or_else(|| "0".into()),
        output_scope_token: format!("export:{}", spec.job_id),
        cloud_consent_id: None,
    })
}

fn crate_sha256() -> String {
    // No signed binary artifact exists yet (D06 distribution is blocked);
    // record a deterministic hash of name+version so provenance stays
    // sha256-shaped while remaining honest about what it covers.
    void_assets::hex_sha256(format!("void-export/{}", env!("CARGO_PKG_VERSION")).as_bytes())
}

/// Coordinates queued export jobs for one project container.
pub struct ExportRunner {
    root: PathBuf,
    db: JobDb,
    cancels: Mutex<HashMap<String, CancelToken>>,
}

impl ExportRunner {
    pub fn new(root: &Path, db: JobDb) -> Self {
        Self {
            root: root.to_path_buf(),
            db,
            cancels: Mutex::new(HashMap::new()),
        }
    }

    /// In-memory runner for tests.
    pub fn memory(root: &Path) -> Result<Self> {
        Ok(Self::new(root, JobDb::open_memory()?))
    }

    /// Validate the spec, verify the immutable checkpoint input and queue
    /// the job. The checkpoint's manifest hash becomes the job's context
    /// hash — a job is bound to bytes, not to "the project right now".
    pub fn submit(&self, spec: &ExportSpec) -> Result<()> {
        spec.validate()?;
        let verified = void_project::verify_checkpoint(&self.root, &spec.checkpoint_id)
            .map_err(|e| ExportError::CheckpointInvalid(e.to_string()))?;
        self.db
            .submit(&job_spec_for(spec, &verified.manifest_sha256)?)
            .map_err(ExportError::Job)
    }

    /// Run a queued job through the whole boundary sequence. Returns the
    /// terminal JobStatus. A cancelled job aborts staging and reports
    /// `cancelled`; a render error reports `failed`; only a verified
    /// publish reports `succeeded` with the artifact receipt in
    /// `result_json`.
    pub fn run(&self, job_id: &str, renderer: &dyn Renderer) -> Result<JobStatus> {
        let rec = self.db.get(job_id)?;
        if rec.status != JobStatus::Queued {
            return Err(ExportError::Job(void_jobs::JobError::InvalidTransition {
                from: rec.status.as_str(),
                to: "running",
            }));
        }
        let spec: ExportSpec =
            serde_json::from_value(rec.spec.parameters.clone()).map_err(ExportError::Json)?;
        let cancel = CancelToken::new();
        self.cancels
            .lock()
            .expect("cancels")
            .insert(job_id.to_string(), cancel.clone());

        self.db.start(job_id)?;
        let outcome = self.drive(&spec, renderer, &cancel);
        self.cancels.lock().expect("cancels").remove(job_id);

        match outcome {
            Ok(receipt) => {
                let result = serde_json::json!({
                    "kind": "ExportResult",
                    "jobId": receipt.job_id,
                    "exportDir": receipt.export_dir,
                    "file": receipt.file,
                    "sha256": receipt.sha256,
                    "bytes": receipt.bytes,
                    "frames": receipt.frames,
                });
                // record_result applies Running→Succeeded; if the job was
                // cancelled meanwhile the result is QUARANTINED by the db
                // — exactly the §6 late-result rule. A quarantined result
                // must not leave a published export dir standing.
                let st = self.db.record_result(job_id, &result)?;
                if st != JobStatus::Succeeded {
                    let _ = std::fs::remove_dir_all(crate::layout::export_dir(&self.root, job_id));
                }
                Ok(st)
            }
            Err(ExportError::Cancelled) => {
                let st = self.db.get(job_id)?.status;
                if st == JobStatus::Cancelling {
                    self.db.finish_cancel(job_id)?;
                    Ok(JobStatus::Cancelled)
                } else if st == JobStatus::Cancelled {
                    Ok(JobStatus::Cancelled)
                } else {
                    // Renderer self-cancelled without a db cancel — treat
                    // as an orderly cancel anyway (no artifact exists).
                    self.db.cancel(job_id).ok();
                    self.db.finish_cancel(job_id).ok();
                    Ok(JobStatus::Cancelled)
                }
            }
            Err(e) => {
                // Failure: staging aborted inside drive(). If a cancel is
                // in flight the job settles as cancelled instead — never
                // succeeded, never with an artifact.
                tracing::warn!(job = job_id, error = %e, "export failed");
                match self.db.get(job_id)?.status {
                    JobStatus::Cancelling | JobStatus::Cancelled => {
                        self.db.finish_cancel(job_id).ok();
                        Ok(JobStatus::Cancelled)
                    }
                    _ => {
                        self.db.fail(job_id)?;
                        Ok(JobStatus::Failed)
                    }
                }
            }
        }
    }

    /// begin → render → verify → provenance → publish; aborts staging on
    /// any failure so nothing partial ever sits in `exports/`.
    fn drive(
        &self,
        spec: &ExportSpec,
        renderer: &dyn Renderer,
        cancel: &CancelToken,
    ) -> Result<crate::session::ExportReceipt> {
        let mut session = ExportSession::begin(&self.root, spec.clone())?;
        let r = (|| {
            session.render(renderer, cancel)?;
            session.verify()?;
            session.write_provenance()?;
            session.publish()
        })();
        if let Err(e) = &r {
            let _ = session.abort();
            return Err(clone_err(e));
        }
        r
    }

    /// Cancel a job (queued → cancelled; running → cancelling + token).
    /// Safe to call before/without run; the runner settles to `cancelled`.
    pub fn cancel(&self, job_id: &str) -> Result<JobStatus> {
        let before = self.db.get(job_id)?.status;
        self.db.cancel(job_id)?;
        if before == JobStatus::Running {
            if let Some(t) = self.cancels.lock().expect("cancels").get(job_id) {
                t.cancel();
            }
        }
        Ok(self.db.get(job_id)?.status)
    }

    /// Read back a job record (status/result/quarantine flag).
    pub fn status(&self, job_id: &str) -> Result<void_jobs::JobRecord> {
        self.db.get(job_id).map_err(ExportError::Job)
    }

    /// All jobs for a project, oldest first.
    pub fn list(&self, project_id: &str) -> Result<Vec<void_jobs::JobRecord>> {
        self.db
            .list_for_project(project_id)
            .map_err(ExportError::Job)
    }

    /// Published export dirs for the container (`exports/<jobId>`).
    pub fn published(&self) -> Result<Vec<PathBuf>> {
        crate::layout::list_exports(&self.root).map_err(ExportError::Io)
    }
}

fn clone_err(e: &ExportError) -> ExportError {
    match e {
        ExportError::Cancelled => ExportError::Cancelled,
        ExportError::Failpoint(s) => ExportError::Failpoint(s),
        ExportError::RendererFailed(m) => ExportError::RendererFailed(m.clone()),
        ExportError::VerifyFailed(m) => ExportError::VerifyFailed(m.clone()),
        ExportError::InvalidSpec(m) => ExportError::InvalidSpec(m.clone()),
        ExportError::CheckpointInvalid(m) => ExportError::CheckpointInvalid(m.clone()),
        ExportError::Io(e) => ExportError::RendererFailed(format!("io: {e}")),
        other => ExportError::RendererFailed(format!("{other}")),
    }
}
