//! Staged av-export session: begin → render → verify → provenance →
//! publish, with quarantine-on-failure and crash-boundary failpoints.
//! Mirrors `void_export::ExportSession` — same atomic-publish contract:
//! output lands in `exports/av-<job>/` only after every check passes,
//! and partial output is never marked success (CONTRACTS.md §6/§7).

use crate::codec;
use crate::error::{AvError, Result};
use crate::ffmpeg::{AvRenderer, RenderOutcome, ResolvedInput};
use crate::layout;
use crate::probe::{MediaProbe, ProbeInfo};
use crate::provenance::{artifact_record, cue_reports, AvProvenance, NONDETERMINISM_NOTE};
use crate::spec::{parse_u64, AvExportSpec, AvFramePlan, AvInput};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use void_assets::{file_sha256, hex_sha256, sync_dir, write_atomic};
use void_export::{argv_sha256, CancelToken};

/// Step boundaries for crash-injection tests (T88: no partial success).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AvStep {
    StageDir,
    ResolveInputs,
    Render,
    Verify,
    Provenance,
    Rename,
    Publish,
}

/// Per-artifact verification result — expected vs measured.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvVerifyReport {
    pub video_frames_expected: String,
    pub video_frames_measured: String,
    pub audio_samples_expected: String,
    pub audio_samples_measured: String,
    pub audio_sample_rate: u32,
    pub audio_channels: u32,
    pub fps_matches_spec: bool,
    /// `|measured - expected|` audio samples — the honest drift.
    pub audio_sample_drift: String,
    /// Declared audio tolerance: one video frame's worth of samples.
    pub audio_tolerance_samples: String,
    pub video_codec: String,
    pub audio_codec: String,
}

pub struct AvSession {
    project_root: PathBuf,
    pub spec: AvExportSpec,
    pub plan: AvFramePlan,
    checkpoint_manifest_sha256: String,
    staging_dir: PathBuf,
    dest_dir: PathBuf,
    resolved: Vec<ResolvedInput>,
    outcome: Option<RenderOutcome>,
    probe_info: Option<ProbeInfo>,
    verify_report: Option<AvVerifyReport>,
    config_sha256: String,
    argv_hash: Arc<Mutex<Option<String>>>,
    encode_wall_ms: u128,
    failpoints: Arc<Mutex<Vec<AvStep>>>,
    renamed: bool,
    published: bool,
}

impl AvSession {
    /// Validate spec, verify the pinned checkpoint, create the staging
    /// dir, and resolve every input to a scoped path. Any failure here
    /// means *nothing* was staged for render.
    pub fn begin(
        project_root: impl Into<PathBuf>,
        spec: AvExportSpec,
        probe: &dyn MediaProbe,
        failpoints: Arc<Mutex<Vec<AvStep>>>,
    ) -> Result<Self> {
        let _ = probe; // probe participates at verify(), not begin
        let project_root = project_root.into();
        spec.validate()?;
        let plan = spec.frame_plan()?;

        let verified = void_project::verify_checkpoint(&project_root, &spec.checkpoint_id)
            .map_err(|e| AvError::CheckpointInvalid(e.to_string()))?;
        if verified.manifest.project_id != spec.project_id {
            return Err(AvError::CheckpointInvalid(format!(
                "checkpoint {} belongs to project {}, not {}",
                spec.checkpoint_id, verified.manifest.project_id, spec.project_id
            )));
        }
        let rev = parse_u64("sourceRevision", &spec.source_revision)?;
        if verified.revision != rev {
            return Err(AvError::CheckpointInvalid(format!(
                "checkpoint revision {} != declared sourceRevision {rev}",
                verified.revision
            )));
        }

        let staging_dir = layout::staging_dir(&project_root, &spec.job_id);
        let dest_dir = layout::export_dir(&project_root, &spec.job_id);
        trip(&failpoints, AvStep::StageDir)?;
        if staging_dir.exists() {
            std::fs::remove_dir_all(&staging_dir)?;
        }
        std::fs::create_dir_all(&staging_dir)?;

        // Resolve inputs to scoped paths BEFORE any encode: checkpoint
        // files are re-hashed against their declared pin; asset blobs
        // must exist in the store. A wrong hash is a hard error, never
        // a silent substitution. A failure here must not litter the
        // freshly-created staging dir.
        let resolved = (|| -> Result<Vec<ResolvedInput>> {
            trip(&failpoints, AvStep::ResolveInputs)?;
            let mut resolved = Vec::new();
            for input in &spec.inputs {
                match input {
                    AvInput::CheckpointFile {
                        rel_path, sha256, ..
                    } => {
                        let p = verified.dir.join(rel_path);
                        if !p.is_file() {
                            return Err(AvError::CheckpointInvalid(format!(
                                "input '{rel_path}' not in checkpoint {}",
                                spec.checkpoint_id
                            )));
                        }
                        let got = file_sha256(&p)?;
                        if &got != sha256 {
                            return Err(AvError::CheckpointInvalid(format!(
                                "input '{rel_path}' sha256 mismatch: pinned {sha256}, on-disk {got}"
                            )));
                        }
                        resolved.push(ResolvedInput {
                            input: input.clone(),
                            argv_path: p.to_string_lossy().into_owned(),
                        });
                    }
                    AvInput::AssetBlob { sha256, .. } => {
                        let store = void_assets::AssetStore::new(&project_root, u64::MAX)?;
                        let p = store.find(sha256).ok_or_else(|| {
                            AvError::CheckpointInvalid(format!("asset blob {sha256} not in store"))
                        })?;
                        resolved.push(ResolvedInput {
                            input: input.clone(),
                            argv_path: p.to_string_lossy().into_owned(),
                        });
                    }
                    AvInput::LavfiTest { src, .. } => {
                        resolved.push(ResolvedInput {
                            input: input.clone(),
                            argv_path: format!("lavfi:{src}"),
                        });
                    }
                }
            }
            Ok(resolved)
        })();
        let resolved = match resolved {
            Ok(r) => r,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&staging_dir);
                return Err(e);
            }
        };

        let config_sha256 = hex_sha256(serde_json::to_string(&spec)?.as_bytes());
        Ok(Self {
            project_root,
            spec,
            plan,
            checkpoint_manifest_sha256: verified.manifest_sha256,
            staging_dir,
            dest_dir,
            resolved,
            outcome: None,
            probe_info: None,
            verify_report: None,
            config_sha256,
            argv_hash: Arc::new(Mutex::new(None)),
            encode_wall_ms: 0,
            failpoints,
            renamed: false,
            published: false,
        })
    }

    pub fn staging_dir(&self) -> &Path {
        &self.staging_dir
    }

    /// Run the encoder. The argv hash is recorded BEFORE the spawn so
    /// provenance always reflects what was attempted.
    pub fn render(&mut self, renderer: &dyn AvRenderer, cancel: &CancelToken) -> Result<()> {
        trip(&self.failpoints, AvStep::Render)?;
        let row = codec::codec(&self.spec.codec_id)?;
        let out_path = self
            .staging_dir
            .join(format!("{}.{}", self.spec.output_name, row.container_ext));
        let argv = renderer
            .as_any()
            .downcast_ref::<crate::ffmpeg::FfmpegRunner>()
            .map(|r| r.build_argv(&self.spec, &self.plan, &self.resolved, &out_path))
            .transpose()?;
        if let Some(argv) = argv {
            let argv_os: Vec<OsString> = argv.iter().map(OsString::from).collect();
            *self.argv_hash.lock().unwrap() = Some(argv_sha256(&argv_os));
        }
        let t0 = Instant::now();
        let outcome = renderer.render(
            &self.spec,
            &self.plan,
            &self.resolved,
            &self.staging_dir,
            &self.spec.output_name,
            cancel,
        )?;
        self.encode_wall_ms = t0.elapsed().as_millis();
        self.outcome = Some(outcome);
        Ok(())
    }

    /// Probe-back: re-read the staged artifact and compare against the
    /// declared plan. Exact video frame count; audio duration within
    /// the declared tolerance (one video frame of samples — mux/audio
    /// codec framing can shift the tail by sub-frame amounts with real
    /// encoders; the fake is exact).
    pub fn verify(&mut self, probe: &dyn MediaProbe) -> Result<AvVerifyReport> {
        trip(&self.failpoints, AvStep::Verify)?;
        let outcome = self
            .outcome
            .as_ref()
            .ok_or_else(|| AvError::VerifyFailed("verify before render".into()))?;
        let file = self.staging_dir.join(&outcome.file);
        if !file.is_file() {
            return Err(AvError::VerifyFailed("staged artifact missing".into()));
        }
        let info = probe.probe(&file)?;
        let expected_frames = parse_u64("videoTotalFrames", &self.plan.video_total_frames)?;
        if info.video_frames != expected_frames {
            return Err(AvError::VerifyFailed(format!(
                "video frame count {} != declared {}",
                info.video_frames, expected_frames
            )));
        }
        let (num, den) = (self.plan.frame_rate_num, self.plan.frame_rate_den);
        if info.video_fps_num != num || info.video_fps_den != den {
            return Err(AvError::VerifyFailed(format!(
                "container fps {}/{} != declared {}/{}",
                info.video_fps_num, info.video_fps_den, num, den
            )));
        }
        if info.audio_sample_rate != self.spec.sample_rate {
            return Err(AvError::VerifyFailed(format!(
                "audio sample rate {} != declared {}",
                info.audio_sample_rate, self.spec.sample_rate
            )));
        }
        let audio_samples = info
            .audio_samples
            .ok_or_else(|| AvError::ProbeFailed("probe returned no audio sample count".into()))?;
        let expected_samples = parse_u64("audioTotalSamples", &self.plan.audio_total_samples)?;
        let drift = audio_samples.abs_diff(expected_samples);
        // One frame of audio at the declared rate — the stated audio
        // tolerance; cues themselves carry the ≤1-frame placement rule.
        let rate = self.spec.frame_rate()?;
        let tolerance =
            rate.samples_for_frames(1, self.spec.sample_rate, crate::rate::FrameRounding::Ceil);
        if drift > tolerance {
            return Err(AvError::VerifyFailed(format!(
                "audio duration drift {drift} samples > one-frame tolerance {tolerance}"
            )));
        }
        let cap = parse_u64("maxOutputBytes", &self.spec.max_output_bytes)?;
        if info.bytes > cap {
            return Err(AvError::OutputLimitExceeded);
        }
        let report = AvVerifyReport {
            video_frames_expected: expected_frames.to_string(),
            video_frames_measured: info.video_frames.to_string(),
            audio_samples_expected: expected_samples.to_string(),
            audio_samples_measured: audio_samples.to_string(),
            audio_sample_rate: info.audio_sample_rate,
            audio_channels: info.audio_channels,
            fps_matches_spec: true,
            audio_sample_drift: drift.to_string(),
            audio_tolerance_samples: tolerance.to_string(),
            video_codec: info.video_codec.clone(),
            audio_codec: info.audio_codec.clone(),
        };
        self.probe_info = Some(info);
        self.verify_report = Some(report.clone());
        Ok(report)
    }

    /// provenance.json + result card inside staging, atomic writes.
    pub fn write_provenance(&mut self, renderer: &dyn AvRenderer, probe_exe: &str) -> Result<()> {
        trip(&self.failpoints, AvStep::Provenance)?;
        let outcome = self
            .outcome
            .as_ref()
            .ok_or_else(|| AvError::VerifyFailed("provenance before render".into()))?;
        let info = self
            .probe_info
            .as_ref()
            .ok_or_else(|| AvError::VerifyFailed("provenance before verify".into()))?;
        let file = self.staging_dir.join(&outcome.file);
        let sha = file_sha256(&file)?;
        let row = codec::codec(&self.spec.codec_id)?;
        let prov = AvProvenance {
            format_version: 1,
            job_id: self.spec.job_id.clone(),
            project_id: self.spec.project_id.clone(),
            checkpoint_id: self.spec.checkpoint_id.clone(),
            source_revision: self.spec.source_revision.clone(),
            config_sha256: self.config_sha256.clone(),
            argv_sha256: self
                .argv_hash
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| "unavailable".into()),
            tool_versions: outcome.tool_versions.clone(),
            codec_id: row.id.into(),
            codec_rights: row.rights,
            codec_license_note: row.license_note.into(),
            checkpoint_manifest_sha256: self.checkpoint_manifest_sha256.clone(),
            asset_hashes: self.spec.asset_hashes.clone(),
            plan: serde_json::to_value(&self.plan)?,
            artifact: artifact_record(outcome.file.clone(), sha.clone(), info, &self.plan),
            cues: cue_reports(&self.plan),
            renderer: renderer.name().into(),
            encoder_exe: renderer.exe_name(),
            probe_exe: probe_exe.into(),
            encode_wall_ms: self.encode_wall_ms.to_string(),
            nondeterminism_note: NONDETERMINISM_NOTE.into(),
            created_at: utc_now(),
        };
        write_atomic(
            &self.staging_dir.join(layout::PROVENANCE_FILE),
            serde_json::to_string_pretty(&prov)?.as_bytes(),
        )?;

        let card = serde_json::json!({
            "kind": "av_export_result",
            "jobId": self.spec.job_id,
            "projectId": self.spec.project_id,
            "checkpointId": self.spec.checkpoint_id,
            "file": outcome.file,
            "sha256": sha,
            "codecId": row.id,
            "videoFrames": info.video_frames.to_string(),
            "audioSamples": info.audio_samples.unwrap_or(0).to_string(),
            "verify": self.verify_report,
        });
        write_atomic(
            &self.staging_dir.join(layout::RESULT_FILE),
            serde_json::to_string_pretty(&card)?.as_bytes(),
        )?;
        Ok(())
    }

    /// Atomic publish: rename staging → `exports/av-<job>/`, fsync, and
    /// re-hash the published artifact against the staged sha.
    pub fn publish(&mut self) -> Result<PathBuf> {
        trip(&self.failpoints, AvStep::Rename)?;
        if self.dest_dir.exists() {
            std::fs::remove_dir_all(&self.dest_dir)?;
        }
        if let Some(parent) = self.dest_dir.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let outcome = self
            .outcome
            .as_ref()
            .ok_or_else(|| AvError::VerifyFailed("publish before render".into()))?;
        let staged_sha = file_sha256(&self.staging_dir.join(&outcome.file))?;
        std::fs::rename(&self.staging_dir, &self.dest_dir)?;
        self.renamed = true;
        trip(&self.failpoints, AvStep::Publish)?;
        sync_dir(&self.project_root.join(layout::EXPORTS_DIR))?;
        // Re-verify post-rename: the published bytes must equal what we
        // hashed in staging.
        let published_sha = file_sha256(&self.dest_dir.join(&outcome.file))?;
        if published_sha != staged_sha {
            return Err(AvError::VerifyFailed(
                "published artifact hash differs from staged hash".into(),
            ));
        }
        self.published = true;
        Ok(self.dest_dir.clone())
    }

    /// Failure path: remove staging. A rename that completed but whose
    /// publish never settled is torn down too — a `failed` job must not
    /// leave a full dir standing in `exports/` (§6: partial/late output
    /// can never masquerade as a success).
    pub fn abort(&self) {
        let _ = std::fs::remove_dir_all(&self.staging_dir);
        if self.renamed && !self.published {
            let _ = std::fs::remove_dir_all(&self.dest_dir);
        }
    }
}

fn trip(failpoints: &Mutex<Vec<AvStep>>, step: AvStep) -> Result<()> {
    let mut fp = failpoints.lock().unwrap();
    if let Some(i) = fp.iter().position(|s| *s == step) {
        fp.remove(i);
        return Err(AvError::Failpoint(match step {
            AvStep::StageDir => "stage_dir",
            AvStep::ResolveInputs => "resolve_inputs",
            AvStep::Render => "render",
            AvStep::Verify => "verify",
            AvStep::Provenance => "provenance",
            AvStep::Rename => "rename",
            AvStep::Publish => "publish",
        }));
    }
    Ok(())
}

/// Civil-from-days UTC timestamp (no chrono dep — same approach as
/// void-export).
pub fn utc_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = secs / 86_400;
    let tod = secs % 86_400;
    let (y, m, d) = civil_from_days(days as i64);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        tod / 3600,
        (tod % 3600) / 60,
        tod % 60
    )
}

fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}
