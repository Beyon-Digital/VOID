//! Job glue — av exports run inside the void-jobs state machine
//! (CONTRACTS.md §6): queued → running → succeeded | failed |
//! cancelling → cancelled, with late results quarantined. Mirrors
//! `void_export::job::ExportRunner`; the JobDb row is the status
//! authority and the filesystem is the artifact authority — a partial
//! encode can never be marked success.

use crate::error::{AvError, Result};
use crate::ffmpeg::{AvRenderer, FfmpegRunner};
use crate::layout;
use crate::probe::{FfprobeRunner, MediaProbe};
use crate::session::AvSession;
use crate::spec::AvExportSpec;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use void_export::CancelToken;
use void_jobs::{JobDb, JobSpec, JobStatus, Reservations};

/// Map an av spec onto the §6 job spec fields. `parameters` carries the
/// whole spec echo; `context_sha256` binds the job to the immutable
/// checkpoint manifest hash.
pub fn job_spec_for(spec: &AvExportSpec, checkpoint_manifest_sha: &str) -> Result<JobSpec> {
    Ok(JobSpec {
        job_id: spec.job_id.clone(),
        project_id: spec.project_id.clone(),
        source_revision: spec.source_revision.clone(),
        context_sha256: checkpoint_manifest_sha.to_string(),
        kind: void_jobs::JobKind::AvExport,
        runtime_id: "void-av".into(),
        runtime_sha256: crate_sha256(),
        model_id: None,
        model_sha256: None,
        inputs: spec.asset_hashes.clone(),
        parameters: serde_json::to_value(spec)?,
        reservations: Reservations {
            // An encode is CPU-bound disk IO with a bounded subprocess —
            // same conservative small-job reservation as void-export.
            ram_bytes: "536870912".into(),
            vram_bytes: "0".into(),
            cpu_threads: 1,
        },
        deadline_monotonic_ns: spec
            .deadline_monotonic_ns
            .clone()
            .unwrap_or_else(|| "0".into()),
        output_scope_token: format!("av:{}", spec.job_id),
        cloud_consent_id: None,
    })
}

fn crate_sha256() -> String {
    // No signed binary artifact yet (D06); record a deterministic
    // name+version hash so provenance stays sha256-shaped and honest.
    void_assets::hex_sha256(format!("void-av/{}", env!("CARGO_PKG_VERSION")).as_bytes())
}

/// The tool pair an av run needs. Prod wiring passes real
/// ffmpeg/ffprobe paths; tests pass the fake binary for both.
pub struct AvTools {
    pub encoder: FfmpegRunner,
    pub probe: FfprobeRunner,
}

impl AvTools {
    pub fn new(encoder_exe: impl Into<PathBuf>, probe_exe: impl Into<PathBuf>) -> Self {
        Self {
            encoder: FfmpegRunner::new(encoder_exe),
            probe: FfprobeRunner::new(probe_exe),
        }
    }
}

/// Coordinates queued av-export jobs for one project container.
pub struct AvRunner {
    root: PathBuf,
    db: JobDb,
    cancels: Mutex<HashMap<String, CancelToken>>,
    /// Test-only crash-boundary injection (same pattern as
    /// ExportSession failpoints).
    failpoints: Arc<Mutex<Vec<crate::session::AvStep>>>,
}

impl AvRunner {
    pub fn new(root: &Path, db: JobDb) -> Self {
        Self {
            root: root.to_path_buf(),
            db,
            cancels: Mutex::new(HashMap::new()),
            failpoints: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// In-memory runner for tests.
    pub fn memory(root: &Path) -> Result<Self> {
        Ok(Self::new(root, JobDb::open_memory()?))
    }

    pub fn set_failpoints(&self, steps: Vec<crate::session::AvStep>) {
        *self.failpoints.lock().unwrap() = steps;
    }

    /// Validate the spec, verify the immutable checkpoint input and
    /// queue the job — bound to the checkpoint manifest hash, not to
    /// "the project right now".
    pub fn submit(&self, spec: &AvExportSpec) -> Result<()> {
        spec.validate()?;
        let verified = void_project::verify_checkpoint(&self.root, &spec.checkpoint_id)
            .map_err(|e| AvError::CheckpointInvalid(e.to_string()))?;
        self.db
            .submit(&job_spec_for(spec, &verified.manifest_sha256)?)
            .map_err(AvError::Job)
    }

    /// Queue with an explicit pre-submit codec gate: absent ffmpeg or
    /// absent codec fails the job at submit as `CodecUnavailable`
    /// rather than at run time (T88 — callers that have the tools at
    /// submit time can refuse earlier).
    pub fn submit_checked(&self, spec: &AvExportSpec, tools: &AvTools) -> Result<()> {
        spec.validate()?;
        let row = codec::codec(&spec.codec_id)?;
        let enc = tools.encoder.detect_encoders()?;
        codec::require_encoders(row, &|n| enc.contains(n))?;
        self.submit(spec)
    }

    /// Run a queued job through begin → render → verify → provenance →
    /// publish. Returns the terminal JobStatus. Cancel → `cancelled`
    /// and staged bytes removed; error → `failed`; only a verified
    /// publish → `succeeded`.
    pub fn run(&self, job_id: &str, tools: &AvTools) -> Result<JobStatus> {
        self.run_parts(job_id, &tools.encoder, &tools.probe)
    }

    /// Same as `run` with explicit renderer/probe implementations —
    /// the seam tests use to drive alternate tool behaviors (hang,
    /// cancel-first, drift) through the same pipeline code.
    pub fn run_parts(
        &self,
        job_id: &str,
        renderer: &dyn AvRenderer,
        probe: &dyn MediaProbe,
    ) -> Result<JobStatus> {
        let rec = self.db.get(job_id)?;
        if rec.status != JobStatus::Queued {
            return Err(AvError::Job(void_jobs::JobError::InvalidTransition {
                from: rec.status.as_str(),
                to: "running",
            }));
        }
        let spec: AvExportSpec =
            serde_json::from_value(rec.spec.parameters.clone()).map_err(AvError::Json)?;
        let cancel = CancelToken::new();
        self.cancels
            .lock()
            .expect("cancels")
            .insert(job_id.to_string(), cancel.clone());

        self.db.start(job_id)?;
        let outcome = self.drive(&spec, renderer, probe, &cancel);
        self.cancels.lock().expect("cancels").remove(job_id);

        match outcome {
            Ok(dest) => {
                let result = serde_json::json!({
                    "kind": "AvExportResult",
                    "jobId": spec.job_id,
                    "exportDir": dest.to_string_lossy(),
                    "provenance": layout::PROVENANCE_FILE,
                });
                // record_result applies Running→Succeeded; a result
                // landing after a cancel is quarantined by the db and
                // the published dir is torn down (§6 late-result rule).
                let st = self.db.record_result(job_id, &result)?;
                if st != JobStatus::Succeeded {
                    let _ = std::fs::remove_dir_all(layout::export_dir(&self.root, job_id));
                }
                Ok(st)
            }
            Err(AvError::Cancelled) => {
                let st = self.db.get(job_id)?.status;
                if st == JobStatus::Cancelling || st == JobStatus::Cancelled {
                    self.db.finish_cancel(job_id)?;
                } else {
                    self.db.cancel(job_id).ok();
                    self.db.finish_cancel(job_id).ok();
                }
                Ok(JobStatus::Cancelled)
            }
            Err(e) => {
                tracing::warn!(job = job_id, error = %e, "av export failed");
                match self.db.get(job_id)?.status {
                    JobStatus::Cancelling | JobStatus::Cancelled => {
                        self.db.finish_cancel(job_id).ok();
                        Ok(JobStatus::Cancelled)
                    }
                    _ => {
                        // CodecUnavailable/Timeout/etc all settle as
                        // `failed` — the typed error rides in the log,
                        // the status stays honest.
                        self.db.fail(job_id)?;
                        Ok(JobStatus::Failed)
                    }
                }
            }
        }
    }

    fn drive(
        &self,
        spec: &AvExportSpec,
        renderer: &dyn AvRenderer,
        probe: &dyn MediaProbe,
        cancel: &CancelToken,
    ) -> Result<PathBuf> {
        let mut session =
            AvSession::begin(&self.root, spec.clone(), probe, self.failpoints.clone())?;
        let r = (|| {
            session.render(renderer, cancel)?;
            session.verify(probe)?;
            session.write_provenance(renderer, &probe.exe_name())?;
            session.publish()
        })();
        if let Err(e) = &r {
            session.abort();
            return Err(clone_err(e));
        }
        r
    }

    /// Cancel a job (queued → cancelled; running → cancelling + token).
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

    pub fn status(&self, job_id: &str) -> Result<void_jobs::JobRecord> {
        self.db.get(job_id).map_err(AvError::Job)
    }

    /// The job db (quarantine/settlement assertions in tests).
    pub fn db(&self) -> &JobDb {
        &self.db
    }

    pub fn list(&self, project_id: &str) -> Result<Vec<void_jobs::JobRecord>> {
        self.db.list_for_project(project_id).map_err(AvError::Job)
    }
}

use crate::codec;

fn clone_err(e: &AvError) -> AvError {
    match e {
        AvError::Cancelled => AvError::Cancelled,
        AvError::Timeout => AvError::Timeout,
        AvError::OutputLimitExceeded => AvError::OutputLimitExceeded,
        AvError::Failpoint(s) => AvError::Failpoint(s),
        AvError::CodecUnavailable(m) => AvError::CodecUnavailable(m.clone()),
        AvError::EncoderFailed(m) => AvError::EncoderFailed(m.clone()),
        AvError::VerifyFailed(m) => AvError::VerifyFailed(m.clone()),
        AvError::ProbeFailed(m) => AvError::ProbeFailed(m.clone()),
        AvError::InvalidSpec(m) => AvError::InvalidSpec(m.clone()),
        AvError::CheckpointInvalid(m) => AvError::CheckpointInvalid(m.clone()),
        other => AvError::EncoderFailed(format!("{other}")),
    }
}
