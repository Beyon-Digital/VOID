//! argv job runner (W12, CONTRACTS.md §2/§6; TEST_MATRIX T49–T52).
//!
//! Mirrors void-export's ExportRunner shape: synchronous `run()` —
//! db.start → spawn/drive → verify+import artifacts → write
//! provenance + result.json → db.record_result → cleanup. The job's
//! state machine stays the only authority: queued → running →
//! succeeded|failed|cancelling→cancelled. A cancelled job's late
//! result is recorded into `jobs/<id>` as `{cancelled:true,...}` and
//! never commits artifacts.

use crate::budget::{monotonic_ns, JobBudget};
use crate::db::JobDb;
use crate::error::{JobError, Result};
use crate::job::{JobSpec, JobStatus};
use crate::layout;
use crate::proto::{parse_worker_line, JobEvent, JobProgress, WorkerEvent, WorkerOutcome};
use crate::provenance::{JobProvenance, Measured, OutputArtifact, RuntimeProvenance};
use crate::worker::{CancelToken, WorkerRuntime};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::Child;
use std::sync::Arc;
use void_assets::AssetStore;

/// One artifact verified + imported into the project's asset store.
#[derive(Debug)]
pub struct ArtifactRecord {
    pub name: String,
    pub sha256: String,
    pub bytes: u64,
    /// `assets/sha256/<hash>.<ext>` — container-relative path.
    pub asset_rel: String,
    pub asset_abs: PathBuf,
}

/// What `run()` produced — mirrors export's RunOutcome.
#[derive(Debug)]
pub enum RunOutcome {
    Succeeded {
        artifacts: Vec<ArtifactRecord>,
        warnings: Vec<String>,
        wall_ns: u64,
        stray_files: u32,
    },
    Failed {
        error: String,
        exit_code: Option<i32>,
        wall_ns: u64,
    },
    /// Cancelled mid-run; whatever arrived after the token tripped is
    /// discarded (T49: late output does not commit).
    Cancelled {
        wall_ns: u64,
    },
}

/// Subscriber for live events (studio subscribes progress + status).
pub type EventSink = Arc<dyn Fn(JobEvent) + Send + Sync>;

/// The argv job runner. Construct per run; db + store are shared.
pub struct JobRunner {
    db: JobDb,
    store: AssetStore,
    /// Project container root — jobs/, staging/, assets/ live inside.
    project_root: PathBuf,
    cancel: CancelToken,
    /// Wired into the coordinator for telemetry; optional.
    event_sink: Option<EventSink>,
}

/// Everything `run()` needs beyond the spec (keeps signature bounded).
pub struct RunContext<'a> {
    pub runtime: &'a WorkerRuntime,
    pub budget: JobBudget,
    /// Qualified model version (None for runtime-only jobs).
    pub model_version: Option<String>,
}

const STDOUT_DRAIN_MAX: u64 = 64 * 1024;
const STDERR_TAIL_MAX: usize = 4096;

impl JobRunner {
    /// `run_at` is filesystem-canonicalized by callers before spawn so
    /// staging paths in provenance are absolute.
    pub fn new(db: JobDb, store: AssetStore, project_root: PathBuf) -> Self {
        Self {
            db,
            store,
            project_root,
            cancel: CancelToken::new(),
            event_sink: None,
        }
    }

    pub fn with_events(mut self, sink: EventSink) -> Self {
        self.event_sink = Some(sink);
        self
    }

    pub fn cancel_token(&self) -> CancelToken {
        self.cancel.clone()
    }

    /// Request cancellation (queued or running): trips the token so the
    /// spawn loop kills the child promptly, then drives the db edge.
    /// Queued jobs cancel directly; running → cancelling (worker teardown
    /// is confirmed by `finish_cancel` once the pump unwinds).
    pub fn request_cancel(&self, job_id: &str) -> Result<JobStatus> {
        let before = self.db.get(job_id)?.status;
        if before.is_terminal() {
            return Ok(before);
        }
        self.db.cancel(job_id)?;
        self.cancel.cancel();
        Ok(if before == JobStatus::Queued {
            JobStatus::Cancelled
        } else {
            JobStatus::Cancelling
        })
    }

    fn emit(&self, spec: &JobSpec, status: &str, percent: Option<f64>, msg: Option<String>) {
        if let Some(sink) = &self.event_sink {
            sink(JobEvent::new(&spec.project_id, &spec.job_id, status, percent, msg));
        }
    }

    /// Run a queued job to its single terminal state.
    ///
    /// Rejects non-queued specs (exactly one terminal outcome; a
    /// second run on a finished job is an error, not a re-run).
    pub fn run(&self, spec: &JobSpec, ctx: &RunContext<'_>) -> Result<RunOutcome> {
        self.db.start(&spec.job_id)?; // InvalidTransition if not queued
        let staging = layout::staging_dir(&self.project_root, &spec.job_id);
        fs::create_dir_all(&staging)?;
        let spawned = monotonic_ns();
        self.emit(spec, "running", Some(0.0), Some("worker spawning".into()));

        let spec_json = serde_json::to_string(spec)?;
        let outcome = match self.drive(spec, ctx, &staging, &spec_json, spawned) {
            Ok(o) => o,
            Err(e) => {
                let _ = self.db.record_failure(&spec.job_id, &e.to_string());
                let _ = fs::remove_dir_all(&staging);
                return Err(e);
            }
        };

        // Record the terminal state FIRST — the state machine decides
        // what commits (T49). A job cancelled while running is in
        // `cancelling`: its result is quarantined, nothing publishes.
        let recorded = match &outcome {
            RunOutcome::Succeeded { artifacts, warnings, .. } => {
                let card = serde_json::json!({
                    "v": 1,
                    "status": "succeeded",
                    "artifacts": artifacts.iter().map(|a| serde_json::json!({
                        "name": a.name, "sha256": a.sha256,
                        "bytes": a.bytes.to_string(), "asset": a.asset_rel,
                    })).collect::<Vec<_>>(),
                    "warnings": warnings,
                });
                self.db.record_result(&spec.job_id, &card)?
            }
            RunOutcome::Failed { error, .. } => {
                self.db.record_failure(&spec.job_id, error)?;
                JobStatus::Failed
            }
            RunOutcome::Cancelled { .. } => {
                match self.db.get(&spec.job_id)?.status {
                    JobStatus::Running => {
                        self.db.cancel(&spec.job_id)?;
                        self.db.finish_cancel(&spec.job_id)?;
                    }
                    JobStatus::Cancelling => self.db.finish_cancel(&spec.job_id)?,
                    _ => {}
                }
                JobStatus::Cancelled
            }
        };

        // Quarantine: cancelled-underneath-us results never publish —
        // imported asset bytes stay as unreferenced content (safe), but
        // no provenance/result card and no staging survives (T49).
        let quarantined = matches!(recorded, JobStatus::Cancelling | JobStatus::Cancelled);
        if quarantined {
            self.emit(spec, "cancelled", None, Some("late result quarantined".into()));
        } else if let RunOutcome::Succeeded {
            artifacts,
            warnings,
            wall_ns,
            stray_files,
        } = &outcome
        {
            self.publish(spec, ctx, artifacts, warnings, *wall_ns, *stray_files)?;
        }

        if recorded != JobStatus::Succeeded {
            let _ = fs::remove_dir_all(&staging);
        }
        self.emit(spec, recorded.as_str(), None, None);
        Ok(if quarantined {
            RunOutcome::Cancelled { wall_ns: 0 }
        } else {
            outcome
        })
    }

    /// Spawn → stdout pump → wait → classify. stdout drains on its own
    /// thread into a channel; the main loop ticks every 10ms so cancel
    /// and deadline fire even while a worker is silent.
    fn drive(
        &self,
        spec: &JobSpec,
        ctx: &RunContext<'_>,
        staging: &Path,
        spec_json: &str,
        spawned_ns: u64,
    ) -> Result<RunOutcome> {
        let deadline = ctx.budget.effective_deadline(spawned_ns);
        let mut child = ctx.runtime.spawn(staging, spec_json, &ctx.budget)?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| JobError::SpawnFailed("worker stdout unavailable".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| JobError::SpawnFailed("worker stderr unavailable".into()))?;

        // stderr drains on its own thread — a chatty-fail worker can't
        // deadlock the pump.
        let err_buf: Arc<std::sync::Mutex<Vec<u8>>> = Arc::new(std::sync::Mutex::new(Vec::new()));
        let err_buf_t = err_buf.clone();
        let err_handle = std::thread::spawn(move || {
            let mut b = [0u8; 2048];
            let mut r = stderr;
            while let Ok(n) = r.read(&mut b) {
                if n == 0 {
                    break;
                }
                let mut g = err_buf_t.lock().unwrap();
                g.extend_from_slice(&b[..n]);
                if g.len() > STDERR_TAIL_MAX * 4 {
                    let drain = g.len() - STDERR_TAIL_MAX * 4;
                    g.drain(..drain);
                }
            }
        });

        // stdout → channel; sender ends when the pipe hits EOF.
        let (tx, rx) = std::sync::mpsc::channel::<std::io::Result<String>>();
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                let mut line = String::new();
                match reader.read_line(&mut line) {
                    Ok(0) => return,               // EOF
                    Ok(_) => {
                        if tx.send(Ok(line)).is_err() {
                            return; // runner gone — stop draining
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        return;
                    }
                }
            }
        });

        let mut progress = JobProgress::default();
        let mut protocol_errors = 0u32;
        let mut declared: Option<crate::proto::WorkerResult> = None;
        let mut stdout_bytes = 0u64;
        let mut killed_for = KilledFor::None;
        let mut eof = false;

        while !eof {
            match rx.recv_timeout(std::time::Duration::from_millis(10)) {
                Ok(Ok(line)) => {
                    stdout_bytes += line.len() as u64;
                    if stdout_bytes > STDOUT_DRAIN_MAX {
                        killed_for = KilledFor::Output;
                        break;
                    }
                    match parse_worker_line(&line) {
                        Ok(Some(WorkerEvent::Progress { percent, message })) => {
                            progress.events_seen += 1;
                            if let Some(p) = percent {
                                progress.percent = Some(p);
                            }
                            if let Some(m) = &message {
                                progress.message = Some(m.clone());
                            }
                            self.emit(spec, "running", percent, message);
                        }
                        Ok(Some(WorkerEvent::Result(r))) => {
                            declared = Some(r);
                            break;
                        }
                        Ok(None) => {}
                        Err(_) => protocol_errors += 1,
                    }
                }
                Ok(Err(e)) => return Err(JobError::Io(e)),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => eof = true,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            }
            if self.cancel.is_cancelled() {
                killed_for = KilledFor::Cancel;
                break;
            }
            if let Some(d) = deadline {
                if monotonic_ns() >= d {
                    killed_for = KilledFor::Deadline;
                    break;
                }
            }
        }

        // After a result line the worker may still be writing files;
        // reap it, but enforce deadline/cancel on the reap too.
        let exit_code = reap(&mut child, self.cancel.clone(), deadline, killed_for)?;
        let _ = err_handle.join();

        if killed_for == KilledFor::Cancel || self.cancel.is_cancelled() {
            return Ok(RunOutcome::Cancelled {
                wall_ns: monotonic_ns().saturating_sub(spawned_ns),
            });
        }
        let wall_ns = monotonic_ns().saturating_sub(spawned_ns);
        let stderr_tail = tail_lossy(&err_buf.lock().unwrap(), STDERR_TAIL_MAX);

        if killed_for == KilledFor::Deadline {
            return Ok(RunOutcome::Failed {
                error: "deadline exceeded — worker killed".into(),
                exit_code,
                wall_ns,
            });
        }
        if killed_for == KilledFor::Output {
            return Ok(RunOutcome::Failed {
                error: "worker exceeded stdout bound".into(),
                exit_code,
                wall_ns,
            });
        }

        match declared {
            Some(res) if res.status == WorkerOutcome::Succeeded => {
                match self.verify_and_import(spec, staging, &res) {
                    Ok((artifacts, stray)) => Ok(RunOutcome::Succeeded {
                        artifacts,
                        warnings: res.warnings,
                        wall_ns,
                        stray_files: stray,
                    }),
                    Err(e) => Ok(RunOutcome::Failed {
                        error: e.to_string(),
                        exit_code,
                        wall_ns,
                    }),
                }
            }
            Some(res) => Ok(RunOutcome::Failed {
                error: res
                    .error
                    .unwrap_or_else(|| "worker reported failure".into()),
                exit_code,
                wall_ns,
            }),
            None => {
                let note = if stderr_tail.is_empty() {
                    String::new()
                } else {
                    format!("; stderr: {stderr_tail}")
                };
                let proto = if protocol_errors > 0 {
                    format!("; {protocol_errors} protocol violations")
                } else {
                    String::new()
                };
                Ok(RunOutcome::Failed {
                    error: format!(
                        "worker exited {exit_code:?} without a result line{note}{proto}"
                    ),
                    exit_code,
                    wall_ns,
                })
            }
        }
    }

    /// Re-hash every declared artifact, reject staging escapes, sweep
    /// strays, import into the content-addressed store.
    fn verify_and_import(
        &self,
        spec: &JobSpec,
        staging: &Path,
        res: &crate::proto::WorkerResult,
    ) -> Result<(Vec<ArtifactRecord>, u32)> {
        let mut records = Vec::with_capacity(res.artifacts.len());
        let mut declared_set = std::collections::BTreeSet::new();
        for a in &res.artifacts {
            // Declared paths are staging-relative; escapes fail the job
            // — a worker must never reach outside its staging dir.
            let rel = Path::new(a);
            if rel.is_absolute() || rel.components().any(|c| matches!(c, std::path::Component::ParentDir)) {
                return Err(JobError::EscapesStaging(a.clone()));
            }
            let abs = staging.join(rel);
            let canon = abs.canonicalize().map_err(|_| JobError::ArtifactNotFound(a.clone()))?;
            let staging_canon = staging.canonicalize()?;
            if !canon.starts_with(&staging_canon) {
                return Err(JobError::EscapesStaging(a.clone()));
            }
            if !canon.is_file() {
                return Err(JobError::ArtifactNotFound(a.clone()));
            }
            let bytes = fs::read(&canon)?;
            let ext = canon
                .extension()
                .map(|e| e.to_string_lossy().to_string())
                .unwrap_or_else(|| "bin".into());
            // Re-hash on import — the asset store verifies content itself
            // (import returns the computed ref; declared bytes are the
            // truth because anything else would have failed the run).
            let asset = self.store.import_bytes(&bytes, &ext)?;
            declared_set.insert(canon.clone());
            let asset_rel = layout::asset_rel(&asset.sha256, &asset.ext);
            records.push(ArtifactRecord {
                name: a.clone(),
                sha256: asset.sha256,
                bytes: asset.bytes,
                asset_rel,
                asset_abs: asset.path,
            });
        }
        let _ = spec; // spec carried for future input-verification rules
        // Stray sweep: files the worker wrote but never declared are
        // dropped before publish — never auto-committed.
        let mut strays = 0u32;
        if staging.is_dir() {
            for e in fs::read_dir(staging)?.flatten() {
                if e.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    if let Ok(c) = e.path().canonicalize() {
                        if !declared_set.contains(&c) {
                            strays += 1;
                        }
                    }
                }
            }
        }
        Ok((records, strays))
    }

    /// Write provenance.json + result.json inside `jobs/<jobId>`.
    /// Skips publish entirely when the job already cancelled (late
    /// results never produce metadata — the staging is torn down).
    fn publish(
        &self,
        spec: &JobSpec,
        ctx: &RunContext<'_>,
        artifacts: &[ArtifactRecord],
        warnings: &[String],
        wall_ns: u64,
        stray_files: u32,
    ) -> Result<()> {
        if self.db.get(&spec.job_id)?.status == JobStatus::Cancelled {
            return Err(JobError::Cancelled);
        }
        let dir = layout::job_dir(&self.project_root, &spec.job_id);
        fs::create_dir_all(&dir)?;
        let argv = ctx.runtime.argv_for(&layout::staging_dir(&self.project_root, &spec.job_id));
        let argv_sha = {
            let joined: Vec<String> = argv.iter().map(|a| a.to_string_lossy().to_string()).collect();
            let mut h = Sha256::new();
            h.update(joined.join("\x1f").as_bytes());
            let d: [u8; 32] = h.finalize().into();
            d.iter().map(|b| format!("{b:02x}")).collect::<String>()
        };
        let arts: Vec<OutputArtifact> = artifacts
            .iter()
            .map(|a| OutputArtifact {
                name: a.name.clone(),
                sha256: a.sha256.clone(),
                bytes: a.bytes.to_string(),
                asset: a.asset_rel.clone(),
            })
            .collect();
        let prov = JobProvenance::new(
            spec,
            &ctx.budget,
            RuntimeProvenance {
                runtime_id: spec.runtime_id.clone(),
                runtime_sha256: spec.runtime_sha256.clone(),
                executable: ctx.runtime.exe_basename(),
                argv_sha256: argv_sha,
            },
            ctx.model_version.clone(),
            arts.clone(),
            warnings.to_vec(),
            Measured {
                wall_ns: wall_ns.to_string(),
                stray_files,
            },
            utc_now(),
        );
        write_json_atomic(&dir.join(layout::PROVENANCE_FILE), &prov)?;
        let result = serde_json::json!({
            "v": 1,
            "jobId": spec.job_id,
            "status": "succeeded",
            "artifacts": arts.iter().map(|a| serde_json::json!({
                "name": a.name, "sha256": a.sha256, "bytes": a.bytes, "asset": a.asset,
            })).collect::<Vec<_>>(),
            "warnings": warnings,
        });
        write_json_atomic(&dir.join(layout::RESULT_FILE), &result)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KilledFor {
    None,
    Cancel,
    Deadline,
    Output,
}

/// Reap a child with a 10ms poll — cancellation/deadline kill it.
fn reap(
    child: &mut Child,
    cancel: CancelToken,
    deadline: Option<u64>,
    mut killed_for: KilledFor,
) -> Result<Option<i32>> {
    loop {
        match child.try_wait()? {
            Some(s) => return Ok(s.code()),
            None => {}
        }
        if killed_for == KilledFor::None {
            if cancel.is_cancelled() {
                killed_for = KilledFor::Cancel;
                let _ = child.kill();
            } else if let Some(d) = deadline {
                if monotonic_ns() >= d {
                    killed_for = KilledFor::Deadline;
                    let _ = child.kill();
                }
            }
        } else {
            // Already marked — still ensure the child dies.
            let _ = child.kill();
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
}

fn tail_lossy(buf: &[u8], max: usize) -> String {
    let start = buf.len().saturating_sub(max);
    String::from_utf8_lossy(&buf[start..]).trim().to_string()
}

fn write_json_atomic(path: &Path, v: &impl serde::Serialize) -> Result<()> {
    let body = serde_json::to_vec_pretty(v)?;
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, &body)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

fn utc_now() -> String {
    // RFC3339 from system time — provenance timestamps are wall clock.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86400;
    let rem = secs % 86400;
    let (y, mo, d) = civil_from_days(days as i64);
    format!(
        "{y:04}-{mo:02}-{d:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = z.div_euclid(146097);
    let doe = z.rem_euclid(146097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
