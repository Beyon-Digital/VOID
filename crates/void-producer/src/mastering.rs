//! Analysis-led mastering records (W21 T79; INTEL-07, OUT-03, DOC-06).
//!
//! NOT a mastering engine — the deliverable here is the honest
//! proposal layer: a real BS.1770 `measure()` result in, a
//! `MasteringProposal` record out carrying gain/EQ suggestions derived
//! from measured deltas, plus the A-B audition spec. Lifecycle mirrors
//! `void_proposals` explicitly (pending → ready → accepted|rejected|
//! stale|failed, terminal states final, revalidation mints NEW record
//! via `supersedes`) — we reuse the pattern rather than forking the
//! crate, per lane instructions.
//!
//! "Meter alone = mastering" is rejected: a proposal NEVER claims the
//! program is mastered — it carries `measured` + `suggested` deltas and
//! requires explicit accept of concrete processing ops.

use crate::error::{ProducerError, Result};
use crate::loudness::LoudnessReport;
use crate::pcm::{read_wav, PcmBuffer};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

/// Tolerated loudness delta before a gain suggestion is emitted (LU).
pub const LOUDNESS_EPSILON_LU: f64 = 0.5;
/// True-peak ceiling per AES streaming practice (−1 dBTP).
pub const TRUE_PEAK_CEILING_DBTP: f64 = -1.0;
/// LRA window used for "over-compressed" suggestions.
pub const LRA_SQUASHED_LU: f64 = 4.0;
/// Default streaming target.
pub const DEFAULT_TARGET_LUFS: f64 = -14.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MasteringStatus {
    Pending,
    Ready,
    Accepted,
    Rejected,
    Stale,
    Failed,
}

impl MasteringStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Accepted | Self::Rejected | Self::Stale | Self::Failed
        )
    }
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Ready => "ready",
            Self::Accepted => "accepted",
            Self::Rejected => "rejected",
            Self::Stale => "stale",
            Self::Failed => "failed",
        }
    }
}

/// One concrete processing op the proposal suggests. These are real,
/// bounded DSP parameter records — the engine worker (NEEDS gap, not
/// implemented on this lane) executes them on accept.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MasteringOp {
    /// Broadband gain to hit the integrated loudness target.
    Gain { db: f64 },
    /// Safety limiter when the measured/gain-adjusted true peak would
    /// exceed the ceiling.
    TruePeakLimiter {
        ceiling_dbtp: f64,
        /// Gain reduction needed at the measured peak.
        max_gr_db: f64,
    },
    /// Static EQ band suggestion derived from spectral evidence
    /// (only emitted when the measurement supports it — no canned EQ).
    EqBand {
        freq_hz: f64,
        gain_db: f64,
        q: f64,
        /// What in the measurement motivated this band.
        basis: String,
    },
}

/// A–B audition spec (T79: loudness-aware audition): both versions
/// loudness-matched so "louder = better" bias can't drive the decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditionSpec {
    /// Asset hash of the source PCM (A side).
    pub a_sha256: String,
    /// The suggested op list rendered to (B side). The engine renders;
    /// this spec is what gets rendered.
    pub ops: Vec<MasteringOp>,
    /// Loudness-match offset applied to B during audition so both
    /// sides play at the same integrated LUFS.
    pub level_match_db: f64,
    /// Loop range for the comparison, in ticks (may be whole file).
    pub loop_start_ticks: i64,
    pub loop_length_ticks: i64,
}

/// The mastering proposal record — the thing the store persists and
/// the lifecycle acts on.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MasteringProposal {
    pub proposal_id: String,
    pub project_id: String,
    /// sha256 of the analyzed PCM (source identity + revalidation).
    pub source_sha256: String,
    /// sha256 of the context (project revision / clip set) the
    /// measurement was taken under — same staleness model as
    /// void-proposals' context hash.
    pub context_sha256: String,
    pub status: MasteringStatus,
    /// What we measured. Never hidden — the evidence the suggestions
    /// are derived from.
    pub measured: LoudnessReport,
    /// The suggested chain, in render order.
    pub ops: Vec<MasteringOp>,
    /// Human-readable derivation notes ("integrated −16.2 LUFS vs
    /// target −14 → +2.2 dB broadband gain" — provenance, not fluff).
    pub rationale: Vec<String>,
    /// A-B audition spec for explicit accept.
    pub audition: Option<AuditionSpec>,
    /// When this proposal supersedes a stale one, the old id.
    pub supersedes: Option<String>,
    pub created_utc: String,
    pub decided_utc: Option<String>,
    /// Failure detail when status = failed (safe message only).
    pub error: Option<String>,
}

/// Target policy for the analysis→suggestion mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MasteringTarget {
    pub integrated_lufs: f64,
    pub true_peak_ceiling_dbtp: f64,
    /// Above this LRA, dynamics suggestions are skipped.
    pub lra_min_lu: f64,
}

impl Default for MasteringTarget {
    fn default() -> Self {
        Self {
            integrated_lufs: DEFAULT_TARGET_LUFS,
            true_peak_ceiling_dbtp: TRUE_PEAK_CEILING_DBTP,
            lra_min_lu: LRA_SQUASHED_LU,
        }
    }
}

fn utc_now() -> String {
    // Same pattern as void-proposals: seconds since epoch, Z-suffixed.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, mo, d) = {
        // minimal civil-from-days (Howard Hinnant) — no chrono dep.
        let days = (secs / 86400) as i64;
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z.rem_euclid(146_097);
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let mo = if mp < 10 { mp + 3 } else { mp - 9 };
        (if mo <= 2 { y + 1 } else { y }, mo, d)
    };
    let s = secs % 86400;
    format!(
        "{y:04}-{mo:02}-{d:02}T{:02}:{:02}:{:02}Z",
        s / 3600,
        (s % 3600) / 60,
        s % 60
    )
}

fn sha256_hex(b: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(b);
    hex::encode(h.finalize())
}

mod hex {
    pub fn encode(b: impl AsRef<[u8]>) -> String {
        b.as_ref().iter().map(|x| format!("{x:02x}")).collect()
    }
}

/// Derive the suggestion chain from a measurement + target. Pure
/// function of the evidence — same report ⇒ same ops.
pub fn suggest_ops(
    report: &LoudnessReport,
    target: &MasteringTarget,
) -> (Vec<MasteringOp>, Vec<String>) {
    let mut ops = Vec::new();
    let mut why = Vec::new();
    if !report.integrated_lufs.is_finite() {
        why.push("programme is silence-gated; no loudness suggestion".into());
        return (ops, why);
    }
    let delta = target.integrated_lufs - report.integrated_lufs;
    if delta.abs() > LOUDNESS_EPSILON_LU {
        ops.push(MasteringOp::Gain {
            db: (delta * 10.0).round() / 10.0,
        });
        why.push(format!(
            "integrated {:.1} LUFS vs target {:.1} → {:+.1} dB broadband gain",
            report.integrated_lufs, target.integrated_lufs, delta
        ));
    } else {
        why.push(format!(
            "integrated {:.1} LUFS within ±{:.1} LU of target",
            report.integrated_lufs, LOUDNESS_EPSILON_LU
        ));
    }
    // Post-gain peak estimate → limiter only if actually needed.
    let post_peak = report.true_peak_dbtp + delta.max(0.0);
    if post_peak > target.true_peak_ceiling_dbtp {
        let gr = post_peak - target.true_peak_ceiling_dbtp;
        ops.push(MasteringOp::TruePeakLimiter {
            ceiling_dbtp: target.true_peak_ceiling_dbtp,
            max_gr_db: (gr * 10.0).round() / 10.0,
        });
        why.push(format!(
            "post-gain true peak {:.1} dBTP exceeds {:.1} → limiter, {:.1} dB max GR",
            post_peak, target.true_peak_ceiling_dbtp, gr
        ));
    }
    if report.lra_lu < target.lra_min_lu && report.lra_lu > 0.0 {
        why.push(format!(
            "LRA {:.1} LU below {:.1} — programme is already dense; no dynamic-suggestion emitted (preserve intent)",
            report.lra_lu, target.lra_min_lu
        ));
    }
    (ops, why)
}

/// Build a proposal record from a measurement (before persistence).
pub fn draft(
    project_id: &str,
    source_sha256: &str,
    context_sha256: &str,
    measured: LoudnessReport,
    target: &MasteringTarget,
    loop_ticks: (i64, i64),
) -> MasteringProposal {
    let (ops, rationale) = suggest_ops(&measured, target);
    let level_match = if measured.integrated_lufs.is_finite() {
        target.integrated_lufs - measured.integrated_lufs
    } else {
        0.0
    };
    let audition = if ops.is_empty() {
        None
    } else {
        Some(AuditionSpec {
            a_sha256: source_sha256.to_string(),
            ops: ops.clone(),
            level_match_db: level_match,
            loop_start_ticks: loop_ticks.0,
            loop_length_ticks: loop_ticks.1,
        })
    };
    MasteringProposal {
        proposal_id: format!("mprop_{}", uuid::Uuid::new_v4().simple()),
        project_id: project_id.to_string(),
        source_sha256: source_sha256.to_string(),
        context_sha256: context_sha256.to_string(),
        status: MasteringStatus::Ready,
        measured,
        ops,
        rationale,
        audition,
        supersedes: None,
        created_utc: utc_now(),
        decided_utc: None,
        error: None,
    }
}

/// Measure a PCM buffer and draft a proposal. `source_sha256`/`context`
/// are the caller's identity claims — revalidation rechecks them.
pub fn analyze(
    project_id: &str,
    source_sha256: &str,
    context_sha256: &str,
    buf: &PcmBuffer,
    target: &MasteringTarget,
    loop_ticks: (i64, i64),
) -> Result<MasteringProposal> {
    let report = crate::loudness::measure(buf)?;
    Ok(draft(
        project_id,
        source_sha256,
        context_sha256,
        report,
        target,
        loop_ticks,
    ))
}

/// Convenience: read a WAV file, compute its content sha, measure.
pub fn analyze_wav(
    project_id: &str,
    context_sha256: &str,
    wav_path: &Path,
    target: &MasteringTarget,
    loop_ticks: (i64, i64),
) -> Result<MasteringProposal> {
    let bytes = std::fs::read(wav_path)?;
    let sha = sha256_hex(&bytes);
    let buf = read_wav(&bytes)?;
    analyze(project_id, &sha, context_sha256, &buf, target, loop_ticks)
}

// ---------------------------------------------------------------------
// Lifecycle — mirrors void_proposals' transition rules exactly:
// pending|ready → accepted|rejected; any non-terminal → stale; terminal
// states are final (no revive — revalidation mints a NEW record).
// ---------------------------------------------------------------------

fn transition(p: &mut MasteringProposal, to: MasteringStatus) -> Result<()> {
    use MasteringStatus::*;
    let legal = matches!(
        (p.status, to),
        (Pending | Ready, Accepted | Rejected) | (Pending | Ready, Stale | Failed)
    );
    if !legal {
        return Err(ProducerError::InvalidTransition {
            from: p.status.as_str(),
            to: to.as_str(),
        });
    }
    p.status = to;
    if to.is_terminal() {
        p.decided_utc = Some(utc_now());
    }
    Ok(())
}

impl MasteringProposal {
    /// Explicit accept — the user chose the suggestion after audition.
    pub fn accept(&mut self) -> Result<()> {
        transition(self, MasteringStatus::Accepted)
    }
    pub fn reject(&mut self) -> Result<()> {
        transition(self, MasteringStatus::Rejected)
    }
    /// Mark stale when the source context moved under it.
    pub fn mark_stale(&mut self) -> Result<()> {
        transition(self, MasteringStatus::Stale)
    }
    pub fn fail(&mut self, safe_error: String) -> Result<()> {
        self.error = Some(safe_error);
        transition(self, MasteringStatus::Failed)
    }
    /// Revalidate against the current context hash. Same rule as
    /// void-proposals: only a stale record revalidates, and success
    /// mints a NEW record (the old one stays stale — never revived).
    pub fn revalidate(&self, current_context_sha256: &str) -> Result<MasteringProposal> {
        if self.status != MasteringStatus::Stale {
            return Err(ProducerError::Revalidation(format!(
                "only stale records revalidate (status={})",
                self.status.as_str()
            )));
        }
        if self.context_sha256 == current_context_sha256 {
            return Err(ProducerError::Revalidation(
                "context unchanged — stale flag was premature".into(),
            ));
        }
        // The measurement is source-keyed: a revalidated record must be
        // re-analyzed by the caller. Here we mint the pending shell
        // that supersedes this one.
        let mut next = self.clone();
        next.proposal_id = format!("mprop_{}", uuid::Uuid::new_v4().simple());
        next.status = MasteringStatus::Pending;
        next.supersedes = Some(self.proposal_id.clone());
        next.context_sha256 = current_context_sha256.to_string();
        next.created_utc = utc_now();
        next.decided_utc = None;
        next.error = None;
        Ok(next)
    }
}

/// JSON-file store, tmp+rename atomicity — same on-disk shape as
/// `ProposalStore` (`mastering/<id>/proposal.json`).
pub struct MasteringStore {
    root: PathBuf,
}

impl MasteringStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
    fn dir(&self, id: &str) -> PathBuf {
        self.root.join("mastering").join(id)
    }
    pub fn save(&self, p: &MasteringProposal) -> Result<()> {
        let dir = self.dir(&p.proposal_id);
        std::fs::create_dir_all(&dir)?;
        let tmp = dir.join("proposal.json.tmp");
        let dst = dir.join("proposal.json");
        std::fs::write(&tmp, serde_json::to_vec_pretty(p)?)?;
        std::fs::rename(&tmp, &dst)?;
        Ok(())
    }
    pub fn load(&self, id: &str) -> Result<MasteringProposal> {
        let bytes = std::fs::read(self.dir(id).join("proposal.json"))?;
        Ok(serde_json::from_slice(&bytes)?)
    }
    pub fn list(&self) -> Result<Vec<MasteringProposal>> {
        let base = self.root.join("mastering");
        let mut out = Vec::new();
        if !base.exists() {
            return Ok(out);
        }
        for e in std::fs::read_dir(&base)? {
            let p = e?.path().join("proposal.json");
            if p.exists() {
                out.push(serde_json::from_slice(&std::fs::read(p)?)?);
            }
        }
        out.sort_by(|a, b| a.proposal_id.cmp(&b.proposal_id));
        Ok(out)
    }
    /// Sweep: non-terminal proposals whose context no longer matches
    /// `current` become stale (void-proposals mark_stale pattern).
    pub fn sweep_stale(&self, current_context_sha256: &str) -> Result<usize> {
        let mut n = 0;
        for mut p in self.list()? {
            if !p.status.is_terminal() && p.context_sha256 != current_context_sha256 {
                p.mark_stale()?;
                self.save(&p)?;
                n += 1;
            }
        }
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(lufs: f64, lra: f64, tp: f64) -> LoudnessReport {
        LoudnessReport {
            sample_rate: 48_000,
            channels: 2,
            frames: 192_000,
            integrated_lufs: lufs,
            lra_lu: lra,
            true_peak_dbtp: tp,
            sample_peak_dbfs: tp - 0.3,
            gated_blocks: 40,
            block_lufs_max: lufs + 2.0,
            block_lufs_min: lufs - 2.0,
        }
    }

    #[test]
    fn gain_suggestion_from_delta() {
        let t = MasteringTarget::default();
        // −16.2 → +2.2 dB; TP −3.5 stays below −1 dBTP after gain →
        // no limiter.
        let (ops, why) = suggest_ops(&report(-16.2, 8.0, -3.5), &t);
        assert!(ops
            .iter()
            .any(|o| matches!(o, MasteringOp::Gain { db } if (*db - 2.2).abs() < 0.05)));
        assert!(!ops
            .iter()
            .any(|o| matches!(o, MasteringOp::TruePeakLimiter { .. })));
        assert!(!why.is_empty());
    }

    #[test]
    fn limiter_only_when_peak_exceeds() {
        let t = MasteringTarget::default();
        // −18 LUFS +4 dB gain → −20−(−4) peak headroom: measured −0.2
        // post-gain +0.2 over ceiling → limiter.
        let (ops, _) = suggest_ops(&report(-18.0, 8.0, -0.2), &t);
        assert!(ops
            .iter()
            .any(|o| matches!(o, MasteringOp::TruePeakLimiter { ceiling_dbtp, .. } if *ceiling_dbtp == -1.0)));
    }

    #[test]
    fn quiet_programme_gets_no_ops() {
        let t = MasteringTarget::default();
        let (ops, why) = suggest_ops(&report(f64::NEG_INFINITY, 0.0, f64::NEG_INFINITY), &t);
        assert!(ops.is_empty());
        assert_eq!(why.len(), 1);
    }

    #[test]
    fn lifecycle_pending_ready_accept_reject_stale() {
        let t = MasteringTarget::default();
        let mut p = draft(
            "p1",
            "src",
            "ctx",
            report(-16.0, 8.0, -3.0),
            &t,
            (0, 960_000),
        );
        assert_eq!(p.status, MasteringStatus::Ready);
        assert!(p.audition.is_some());
        p.accept().unwrap();
        assert!(p.accept().is_err()); // terminal — no re-accept
        assert!(p.reject().is_err());

        let mut q = draft(
            "p1",
            "src",
            "ctx",
            report(-16.0, 8.0, -3.0),
            &t,
            (0, 960_000),
        );
        q.reject().unwrap();
        assert!(q.mark_stale().is_err());

        let mut s = draft(
            "p1",
            "src",
            "ctx",
            report(-16.0, 8.0, -3.0),
            &t,
            (0, 960_000),
        );
        s.mark_stale().unwrap();
        // Revalidate mints a NEW record — stale stays stale.
        let next = s.revalidate("ctx2").unwrap();
        assert_eq!(next.status, MasteringStatus::Pending);
        assert_eq!(next.supersedes.as_deref(), Some(s.proposal_id.as_str()));
        assert_eq!(s.status, MasteringStatus::Stale);
        assert!(s.revalidate("ctx2").is_ok()); // still allowed (stale)
        assert!(s.accept().is_err()); // stale is terminal
    }

    #[test]
    fn store_roundtrip_and_sweep() {
        let dir = tempfile::tempdir().unwrap();
        let store = MasteringStore::new(dir.path());
        let t = MasteringTarget::default();
        let p = draft(
            "p1",
            "src",
            "ctx-A",
            report(-16.0, 8.0, -3.0),
            &t,
            (0, 960_000),
        );
        store.save(&p).unwrap();
        let back = store.load(&p.proposal_id).unwrap();
        assert_eq!(back.proposal_id, p.proposal_id);
        assert_eq!(back.status, MasteringStatus::Ready);
        assert_eq!(store.sweep_stale("ctx-A").unwrap(), 0);
        assert_eq!(store.sweep_stale("ctx-B").unwrap(), 1);
        assert_eq!(
            store.load(&p.proposal_id).unwrap().status,
            MasteringStatus::Stale
        );
    }
}
