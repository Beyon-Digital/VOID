//! argv-worker wire protocol v1 (workers/PROTOCOL.md).
//!
//! stdout carries JSON lines: `{"v":1,"kind":"progress",...}` and one
//! final `{"v":1,"kind":"result",...}`. Non-JSON lines are log noise —
//! tolerated, never authoritative. The worker's declared status is
//! checked against reality: artifact files are re-hashed by the runner.

use serde::{Deserialize, Serialize};

pub const WORKER_PROTOCOL_VERSION: u32 = 1;

/// A parsed stdout line. `Progress` updates the job's live view;
/// `Result` is the terminal outcome the worker claims.
#[derive(Debug, Clone, PartialEq)]
pub enum WorkerEvent {
    Progress {
        percent: Option<f64>,
        message: Option<String>,
    },
    Result(WorkerResult),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerOutcome {
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkerResult {
    pub status: WorkerOutcome,
    /// Declared artifact paths RELATIVE to the staging dir. The runner
    /// canonicalizes each — anything escaping staging fails the job.
    pub artifacts: Vec<String>,
    pub error: Option<String>,
    pub warnings: Vec<String>,
}

/// One telemetry-shaped event the coordinator can publish per progress
/// line / status transition. Snake_case on the wire — the studio's
/// `parseJobEvent` (export/job.ts) reads exactly this shape.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JobEvent {
    pub kind: &'static str,
    pub project_id: String,
    pub job_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quarantined: Option<bool>,
}

impl JobEvent {
    pub fn new(
        project_id: &str,
        job_id: &str,
        status: &str,
        percent: Option<f64>,
        message: Option<String>,
    ) -> Self {
        Self {
            kind: "JobEvent",
            project_id: project_id.into(),
            job_id: job_id.into(),
            status: status.into(),
            percent,
            message,
            quarantined: None,
        }
    }
}

/// Live progress state the runner keeps per running job.
#[derive(Debug, Clone, Default)]
pub struct JobProgress {
    pub percent: Option<f64>,
    pub message: Option<String>,
    pub events_seen: u32,
}

/// Parse one stdout line. Non-JSON noise → `Ok(None)`. A malformed
/// protocol line (JSON but wrong shape/version) → `Err` — the runner
/// counts protocol violations; a worker that cannot speak v1 fails.
pub fn parse_worker_line(line: &str) -> Result<Option<WorkerEvent>, String> {
    let t = line.trim();
    if t.is_empty() || !t.starts_with('{') {
        return Ok(None);
    }
    let v: serde_json::Value = match serde_json::from_str(t) {
        Ok(v) => v,
        Err(e) => return Err(format!("bad worker json: {e}")),
    };
    match v.get("v").and_then(|x| x.as_u64()) {
        Some(1) => {}
        Some(n) => return Err(format!("unsupported worker protocol v{n}")),
        None => return Err("worker line missing protocol version".into()),
    }
    match v.get("kind").and_then(|k| k.as_str()) {
        Some("progress") => {
            let percent = v
                .get("percent")
                .and_then(|p| p.as_f64())
                .map(|p| p.clamp(0.0, 100.0));
            let message = v.get("message").and_then(|m| m.as_str()).map(|s| s.to_string());
            Ok(Some(WorkerEvent::Progress { percent, message }))
        }
        Some("result") => {
            let status = match v.get("status").and_then(|s| s.as_str()) {
                Some("succeeded") => WorkerOutcome::Succeeded,
                Some("failed") => WorkerOutcome::Failed,
                other => return Err(format!("bad result status {other:?}")),
            };
            let artifacts = v
                .get("artifacts")
                .and_then(|a| a.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|e| {
                            e.get("path").and_then(|p| p.as_str()).map(|s| s.to_string())
                        })
                        .collect()
                })
                .unwrap_or_default();
            let error = v.get("error").and_then(|e| e.as_str()).map(|s| s.to_string());
            let warnings = v
                .get("warnings")
                .and_then(|w| w.as_array())
                .map(|w| {
                    w.iter()
                        .filter_map(|e| e.as_str().map(|s| s.to_string()))
                        .collect()
                })
                .unwrap_or_default();
            Ok(Some(WorkerEvent::Result(WorkerResult {
                status,
                artifacts,
                error,
                warnings,
            })))
        }
        Some(other) => Err(format!("unknown worker event kind {other:?}")),
        None => Err("worker line missing kind".into()),
    }
}
