//! App-private SQLite store (CONTRACTS.md §4/DATA-01).
//!
//! The DB is a *reconstructable index* — committed checkpoints, recent
//! projects, asset index — plus a transactional store for app jobs. It is
//! never the save authority: CURRENT + verified manifests are. A mismatch
//! is reconciled FROM the container, never toward it.
//!
//! `bundled` rusqlite pins the linked SQLite build; `sqlite_version()` is
//! recorded in evidence at qualification time.

use crate::error::{JobError, Result};
use crate::job::{JobRecord, JobSpec, JobStatus};
use rusqlite::{params, Connection};
use std::path::Path;

const SCHEMA_VERSION: i64 = 1;

pub struct JobDb {
    conn: Connection,
}

impl JobDb {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(p) = path.as_ref().parent() {
            std::fs::create_dir_all(p).map_err(|e| JobError::InvalidSpec(e.to_string()))?;
        }
        let conn = Connection::open(path)?;
        Self::init(conn)
    }

    /// In-memory store for tests.
    pub fn open_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "synchronous", "FULL")?;
        conn.execute_batch(
            "
            CREATE TABLE IF NOT EXISTS recent_projects (
                project_id TEXT PRIMARY KEY,
                container_path TEXT NOT NULL,
                last_opened_at TEXT NOT NULL,
                last_checkpoint_id TEXT
            );
            CREATE TABLE IF NOT EXISTS checkpoints (
                project_id TEXT NOT NULL,
                checkpoint_id TEXT NOT NULL,
                revision TEXT NOT NULL,
                manifest_sha256 TEXT NOT NULL,
                is_current INTEGER NOT NULL DEFAULT 0,
                verified_at TEXT NOT NULL,
                PRIMARY KEY (project_id, checkpoint_id)
            );
            CREATE TABLE IF NOT EXISTS assets (
                project_id TEXT NOT NULL,
                sha256 TEXT NOT NULL,
                bytes TEXT NOT NULL,
                first_seen_checkpoint TEXT,
                PRIMARY KEY (project_id, sha256)
            );
            CREATE TABLE IF NOT EXISTS jobs (
                job_id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                spec_json TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                result_json TEXT,
                quarantined INTEGER NOT NULL DEFAULT 0
            );
            ",
        )?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        Ok(Self { conn })
    }

    /// The actually-linked SQLite build — recorded as dependency evidence.
    pub fn sqlite_version(&self) -> Result<String> {
        Ok(self
            .conn
            .query_row("SELECT sqlite_version()", [], |r| r.get(0))?)
    }

    pub fn schema_version(&self) -> Result<i64> {
        let v: i64 = self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?;
        Ok(v)
    }

    // ---- jobs ----------------------------------------------------------

    pub fn submit(&self, spec: &JobSpec) -> Result<()> {
        spec.validate()?;
        let now = self.utc_now();
        self.conn.execute(
            "INSERT INTO jobs(job_id, project_id, spec_json, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'queued', ?4, ?4)",
            params![
                spec.job_id,
                spec.project_id,
                serde_json::to_string(spec)?,
                now
            ],
        )?;
        Ok(())
    }

    fn set_status(&self, job_id: &str, to: JobStatus) -> Result<()> {
        let rec = self.get(job_id)?;
        crate::job::transition(rec.status, to)?;
        self.conn.execute(
            "UPDATE jobs SET status=?1, updated_at=?2 WHERE job_id=?3",
            params![to.as_str(), self.utc_now(), job_id],
        )?;
        Ok(())
    }

    pub fn start(&self, job_id: &str) -> Result<()> {
        self.set_status(job_id, JobStatus::Running)
    }

    pub fn cancel(&self, job_id: &str) -> Result<()> {
        let rec = self.get(job_id)?;
        match rec.status {
            JobStatus::Queued => self.set_status(job_id, JobStatus::Cancelled),
            JobStatus::Running => self.set_status(job_id, JobStatus::Cancelling),
            _ => Err(JobError::InvalidTransition {
                from: rec.status.as_str(),
                to: JobStatus::Cancelled.as_str(),
            }),
        }
    }

    /// Worker confirmed teardown → Cancelling→Cancelled.
    pub fn finish_cancel(&self, job_id: &str) -> Result<()> {
        self.set_status(job_id, JobStatus::Cancelled)
    }

    pub fn fail(&self, job_id: &str) -> Result<()> {
        self.set_status(job_id, JobStatus::Failed)
    }

    /// Failure with a stored reason (W12): same transition as `fail`,
    /// plus a small result card so the reason survives the process.
    pub fn record_failure(&self, job_id: &str, error: &str) -> Result<()> {
        let rec = self.get(job_id)?;
        match rec.status {
            JobStatus::Running => {
                let card = serde_json::json!({"v":1,"status":"failed","error":error});
                self.conn.execute(
                    "UPDATE jobs SET status='failed', result_json=?1, updated_at=?2 WHERE job_id=?3",
                    params![card.to_string(), self.utc_now(), job_id],
                )?;
                Ok(())
            }
            // A failing worker that already cancelled: store quarantined.
            JobStatus::Cancelling | JobStatus::Cancelled => {
                let card = serde_json::json!({"v":1,"status":"failed","error":error});
                self.conn.execute(
                    "UPDATE jobs SET result_json=?1, quarantined=1, updated_at=?2 WHERE job_id=?3",
                    params![card.to_string(), self.utc_now(), job_id],
                )?;
                Ok(())
            }
            other => Err(JobError::LateResult(format!(
                "job in terminal state {}",
                other.as_str()
            ))),
        }
    }

    /// Jobs in one status — admission control + orphan sweep inputs.
    pub fn list_by_status(&self, status: JobStatus) -> Result<Vec<JobRecord>> {
        let mut st = self
            .conn
            .prepare("SELECT job_id FROM jobs WHERE status=?1 ORDER BY created_at")?;
        let ids = st
            .query_map(params![status.as_str()], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ids.into_iter().map(|id| self.get(&id)).collect()
    }

    /// Non-terminal jobs (queued/running/cancelling), oldest first.
    pub fn nonterminal(&self) -> Result<Vec<JobRecord>> {
        let mut st = self.conn.prepare(
            "SELECT job_id FROM jobs WHERE status IN ('queued','running','cancelling') ORDER BY created_at",
        )?;
        let ids = st
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ids.into_iter().map(|id| self.get(&id)).collect()
    }

    /// Record a worker result. Results for cancelled/cancelling jobs — or
    /// any terminal job — are quarantined, not applied (§6).
    pub fn record_result(&self, job_id: &str, result: &serde_json::Value) -> Result<JobStatus> {
        let rec = self.get(job_id)?;
        match rec.status {
            JobStatus::Running => {
                self.conn.execute(
                    "UPDATE jobs SET status='succeeded', result_json=?1, updated_at=?2 WHERE job_id=?3",
                    params![serde_json::to_string(result)?, self.utc_now(), job_id],
                )?;
                Ok(JobStatus::Succeeded)
            }
            JobStatus::Cancelling | JobStatus::Cancelled => {
                self.conn.execute(
                    "UPDATE jobs SET result_json=?1, quarantined=1, updated_at=?2 WHERE job_id=?3",
                    params![serde_json::to_string(result)?, self.utc_now(), job_id],
                )?;
                Ok(rec.status) // status unchanged; result quarantined
            }
            other => Err(JobError::LateResult(format!(
                "job in terminal state {}",
                other.as_str()
            ))),
        }
    }

    pub fn get(&self, job_id: &str) -> Result<JobRecord> {
        self.conn
            .query_row(
                "SELECT spec_json, status, created_at, updated_at, result_json, quarantined
                 FROM jobs WHERE job_id=?1",
                params![job_id],
                |r| {
                    let spec_json: String = r.get(0)?;
                    let status: String = r.get(1)?;
                    Ok((
                        spec_json,
                        status,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                        r.get::<_, Option<String>>(4)?,
                        r.get::<_, i64>(5)?,
                    ))
                },
            )
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => JobError::NotFound(job_id.into()),
                other => JobError::Sqlite(other),
            })
            .and_then(|(spec_json, status, c, u, res, quar)| {
                Ok(JobRecord {
                    spec: serde_json::from_str(&spec_json)?,
                    status: JobStatus::parse(&status)?,
                    created_at: c,
                    updated_at: u,
                    result: res.map(|s| serde_json::from_str(&s)).transpose()?,
                    quarantined: quar != 0,
                })
            })
    }

    pub fn list_for_project(&self, project_id: &str) -> Result<Vec<JobRecord>> {
        let mut st = self
            .conn
            .prepare("SELECT job_id FROM jobs WHERE project_id=?1 ORDER BY created_at")?;
        let ids = st
            .query_map(params![project_id], |r| r.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        ids.into_iter().map(|id| self.get(&id)).collect()
    }

    // ---- index reconciliation ------------------------------------------

    /// Rebuild the committed-project index from a verified checkpoint
    /// (called by reconcile.rs after recovery). Transactional; the caller
    /// still treats CURRENT+manifest as the authority.
    pub fn upsert_current_checkpoint(
        &self,
        project_id: &str,
        checkpoint_id: &str,
        revision: u64,
        manifest_sha256: &str,
        asset_hashes: &[String],
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO checkpoints(project_id, checkpoint_id, revision, manifest_sha256, is_current, verified_at)
             VALUES (?1, ?2, ?3, ?4, 1, ?5)
             ON CONFLICT(project_id, checkpoint_id)
             DO UPDATE SET revision=excluded.revision, manifest_sha256=excluded.manifest_sha256, is_current=1, verified_at=excluded.verified_at",
            params![project_id, checkpoint_id, revision.to_string(), manifest_sha256, self.utc_now()],
        )?;
        tx.execute(
            "UPDATE checkpoints SET is_current=0 WHERE project_id=?1 AND checkpoint_id<>?2",
            params![project_id, checkpoint_id],
        )?;
        for h in asset_hashes {
            tx.execute(
                "INSERT OR IGNORE INTO assets(project_id, sha256, bytes, first_seen_checkpoint)
                 VALUES (?1, ?2, '0', ?3)",
                params![project_id, h, checkpoint_id],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// What the index currently believes is the published checkpoint —
    /// compared against CURRENT during recovery.
    pub fn indexed_current(&self, project_id: &str) -> Result<Option<String>> {
        let mut st = self.conn.prepare(
            "SELECT checkpoint_id FROM checkpoints WHERE project_id=?1 AND is_current=1",
        )?;
        let mut rows = st.query(params![project_id])?;
        Ok(match rows.next()? {
            Some(r) => Some(r.get(0)?),
            None => None,
        })
    }

    /// Checkpoints indexed for a project that are NOT the given pointer —
    /// stale rows a manifest-verified recovery may prune or mark.
    pub fn stale_checkpoints(
        &self,
        project_id: &str,
        current_checkpoint_id: &str,
    ) -> Result<Vec<String>> {
        let mut st = self.conn.prepare(
            "SELECT checkpoint_id FROM checkpoints WHERE project_id=?1 AND checkpoint_id<>?2",
        )?;
        let rows = st
            .query_map(params![project_id, current_checkpoint_id], |r| r.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(rows)
    }

    pub fn touch_recent_project(
        &self,
        project_id: &str,
        container_path: &str,
        last_checkpoint_id: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO recent_projects(project_id, container_path, last_opened_at, last_checkpoint_id)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(project_id) DO UPDATE SET last_opened_at=excluded.last_opened_at, last_checkpoint_id=excluded.last_checkpoint_id",
            params![project_id, container_path, self.utc_now(), last_checkpoint_id],
        )?;
        Ok(())
    }
}

impl JobDb {
    /// UTC timestamp rendered by the linked SQLite itself (no extra dep).
    fn utc_now(&self) -> String {
        self.conn
            .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |r| {
                r.get(0)
            })
            .unwrap_or_else(|_| "1970-01-01T00:00:00.000Z".into())
    }
}
