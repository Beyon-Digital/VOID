//! Proposal record store — `proposals/<proposalId>/proposal.json`
//! inside the project container (peer of `jobs/`). Append-only-ish:
//! records mutate only through the legal lifecycle transitions; files
//! are written tmp+rename so a torn write never produces a half record.

use crate::error::{ProposalError, Result};
use crate::record::ProposalRecord;
use std::fs;
use std::path::PathBuf;

pub const PROPOSALS_DIR: &str = "proposals";
pub const RECORD_FILE: &str = "proposal.json";

pub struct ProposalStore {
    root: PathBuf,
}

impl ProposalStore {
    /// `root` is the project container path (song.void/).
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn dir(&self, proposal_id: &str) -> PathBuf {
        self.root.join(PROPOSALS_DIR).join(proposal_id)
    }

    pub fn path(&self, proposal_id: &str) -> PathBuf {
        self.dir(proposal_id).join(RECORD_FILE)
    }

    /// Persist a record atomically (staging-then-rename, same-fs).
    pub fn save(&self, rec: &ProposalRecord) -> Result<()> {
        let dir = self.dir(&rec.proposal_id);
        fs::create_dir_all(&dir)?;
        let tmp = dir.join(format!(".{}.tmp", rec.proposal_id));
        fs::write(&tmp, serde_json::to_vec_pretty(rec)?)?;
        fs::rename(&tmp, self.path(&rec.proposal_id))?;
        Ok(())
    }

    pub fn load(&self, proposal_id: &str) -> Result<ProposalRecord> {
        let p = self.path(proposal_id);
        if !p.is_file() {
            return Err(ProposalError::NotFound(proposal_id.into()));
        }
        Ok(serde_json::from_slice(&fs::read(p)?)?)
    }

    /// All records for a project, oldest first. Malformed records are
    /// skipped — a corrupt file must not wedge the lane.
    pub fn list_for_project(&self, project_id: &str) -> Vec<ProposalRecord> {
        let dir = self.root.join(PROPOSALS_DIR);
        let mut out = Vec::new();
        if let Ok(rd) = fs::read_dir(dir) {
            for e in rd.flatten() {
                let p = e.path().join(RECORD_FILE);
                if let Ok(bytes) = fs::read(&p) {
                    if let Ok(r) = serde_json::from_slice::<ProposalRecord>(&bytes) {
                        if r.project_id == project_id {
                            out.push(r);
                        }
                    }
                }
            }
        }
        out.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        out
    }

    /// Absolute path of the staging dir a generation job writes into —
    /// mirrors void-jobs layout (`staging/job-<jobId>`).
    pub fn job_staging(&self, job_id: &str) -> PathBuf {
        self.root.join("staging").join(format!("job-{job_id}"))
    }
}

/// RFC3339 UTC without pulling a clock dep — the job record already
/// trusts SQLite for timestamps; records get one via a tiny formatter.
pub fn utc_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    civil_from_unix(secs)
}

fn civil_from_unix(t: u64) -> String {
    // days → civil calendar (Howard Hinnant's algorithm), no deps.
    let days = (t / 86400) as i64;
    let secs = t % 86400;
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        secs / 3600,
        (secs % 3600) / 60,
        secs % 60
    )
}
