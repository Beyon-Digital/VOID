//! Audiovisual export specification (CONTRACTS.md §7, T87–T89).
//!
//! Like `void_export::ExportSpec`, everything the encoder needs is in the
//! spec — an immutable checkpoint pin, asset sha256s, an explicit rational
//! frame rate, an explicit rounding policy, and explicit bounds. Large
//! integers are decimal strings in JSON.

use crate::error::{AvError, Result};
use crate::rate::{place_cue, rational_round, CuePlacement, FrameRate, FrameRounding};
use serde::{Deserialize, Serialize};
use void_export::{sanitize_output_name, ChannelLayout, APPROVED_SAMPLE_RATES};

/// Bound on total video frames per job (≈2 h at 1000 fps ceiling is
/// already generous — a bounded job, not an unbounded one, §2).
pub const MAX_RENDER_VIDEO_FRAMES: u64 = 7_200_000;
/// Bound on total audio samples per job (≈12 h at 96 kHz).
pub const MAX_RENDER_SAMPLES: u64 = 96_000 * 12 * 60 * 60;
/// Bound on the declared encoder wall-clock timeout: 30 min.
pub const MAX_TIMEOUT_MS: u64 = 30 * 60 * 1000;
/// Bound on the declared output-size cap: 64 GiB.
pub const MAX_OUTPUT_BYTES: u64 = 64 << 30;
/// Upper bound on pixel dimensions (16 K).
pub const MAX_DIMENSION: u32 = 16_384;

/// How much media beyond `range_samples` is rendered — explicit and
/// bounded (tail policy is part of the contract, §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum AvTailPolicy {
    #[default]
    /// Hard cut at `range_samples`.
    None,
    /// Extra audio samples after the range end (also extended to video
    /// frames covering the tail).
    Samples { samples: u64 },
    /// Extra video frames after the video-covered range end.
    Frames { frames: u64 },
}

/// One media input the encoder consumes. Checkpoint files are resolved
/// against the verified checkpoint dir at `begin()`; lavfi sources name
/// an ffmpeg filtergraph input and are used for the deterministic test
/// fixture (T87 needs reproducible media).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AvInput {
    /// A file pinned inside the verified checkpoint:
    /// `checkpoint_dir/rel_path`, byte-pinned by `sha256`.
    CheckpointFile {
        rel_path: String,
        sha256: String,
        /// Which input stream this feeds: `video` | `audio`.
        role: AvInputRole,
    },
    /// A project asset blob pinned by sha256, materialized into staging
    /// before encode so the subprocess only ever sees scoped paths.
    AssetBlob { sha256: String, role: AvInputRole },
    /// Deterministic lavfi fixture source (testsrc2, sine, …). Never a
    /// real project input — used by the offline qualification harness.
    LavfiTest { src: String, role: AvInputRole },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AvInputRole {
    Video,
    Audio,
}

/// A cue timestamp (marker, onset, beat) whose position in the rendered
/// video is checked for the declared ≤1-frame tolerance (T87).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvCue {
    pub cue_id: String,
    /// Audio sample index in the rendered program (decimal u64).
    pub at_sample: String,
}

/// The immutable av-render input set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvExportSpec {
    /// UUID of this export job.
    pub job_id: String,
    pub project_id: String,
    /// Immutable checkpoint the render reads (§7).
    pub checkpoint_id: String,
    /// Coordinator revision the checkpoint represents — decimal u64.
    pub source_revision: String,
    /// SHA-256 hashes of the immutable asset blobs the render may read.
    #[serde(default)]
    pub asset_hashes: Vec<String>,

    /// Audio program length in samples: [0, range_samples).
    pub range_samples: String,
    /// Required; must be in APPROVED_SAMPLE_RATES.
    pub sample_rate: u32,
    pub channels: ChannelLayout,
    /// Rational video frame rate: `num/den` fps (e.g. 30000/1001).
    pub frame_rate_num: u64,
    pub frame_rate_den: u64,
    /// Explicit frame/sample rounding policy — required, never ambient.
    pub rounding: FrameRounding,
    /// Pixel dimensions of the encoded video.
    pub width: u32,
    pub height: u32,
    /// Codec id into the declarative codec matrix (`crate::codec`).
    pub codec_id: String,
    /// Media inputs; exactly one `video` role and one `audio` role.
    pub inputs: Vec<AvInput>,
    /// Cue timestamps to check against the declared tolerance.
    #[serde(default)]
    pub cues: Vec<AvCue>,
    #[serde(default)]
    pub tail: AvTailPolicy,
    /// Scoped output base name — `[A-Za-z0-9._-]`, no slashes; the
    /// container extension comes from the codec matrix (§7).
    pub output_name: String,
    /// Encoder wall-clock bound (decimal ms). Required — no unbounded
    /// encodes (§2).
    pub timeout_ms: String,
    /// Output-size bound in bytes (decimal). Enforced both via the
    /// encoder (`-fs`) and by verify-back.
    pub max_output_bytes: String,
    /// Optional render deadline — monotonic ns, decimal string (§6).
    #[serde(default)]
    pub deadline_monotonic_ns: Option<String>,
}

/// Fully computed frame/sample plan — the numbers the encoder MUST
/// produce, plus the cue placements. Provenance records expected vs
/// measured against these.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvFramePlan {
    /// Audio samples in the program range — decimal string.
    pub audio_range_samples: String,
    /// Audio samples added by the tail — decimal string.
    pub audio_tail_samples: String,
    /// Total audio samples the container must cover — decimal string.
    pub audio_total_samples: String,
    /// Video frames covering the program range — decimal string.
    pub video_range_frames: String,
    /// Video frames added by the tail — decimal string.
    pub video_tail_frames: String,
    /// Total video frames the encoder must emit — decimal string.
    pub video_total_frames: String,
    pub frame_rate_num: u64,
    pub frame_rate_den: u64,
    /// Where each declared cue lands inside the frame raster.
    pub cues: Vec<PlannedCue>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlannedCue {
    pub cue_id: String,
    /// Expected sample position — decimal string.
    pub at_sample: String,
    /// Computed placement in the frame raster.
    #[serde(flatten)]
    pub placement: CuePlacement,
}

impl AvExportSpec {
    pub fn frame_rate(&self) -> Result<FrameRate> {
        FrameRate::new(self.frame_rate_num, self.frame_rate_den)
    }

    /// The explicit frame/sample computation (§7: "frame rounding/tail
    /// duration must be explicit"). All u128 rational math.
    pub fn frame_plan(&self) -> Result<AvFramePlan> {
        let rate = self.frame_rate()?;
        let range = parse_u64("rangeSamples", &self.range_samples)?;
        if range == 0 {
            return Err(AvError::InvalidSpec("rangeSamples must be > 0".into()));
        }
        let sr = self.sample_rate;

        let (tail_samples, tail_frames) = match self.tail {
            AvTailPolicy::None => (0u64, 0u64),
            AvTailPolicy::Samples { samples } => (samples, 0),
            AvTailPolicy::Frames { frames } => (0, frames),
        };
        if tail_samples > MAX_RENDER_SAMPLES || tail_frames > MAX_RENDER_VIDEO_FRAMES {
            return Err(AvError::InvalidSpec("tail exceeds bounds".into()));
        }

        let audio_total = range
            .checked_add(tail_samples)
            .filter(|v| *v <= MAX_RENDER_SAMPLES)
            .ok_or_else(|| AvError::InvalidSpec("audio duration exceeds bound".into()))?;

        let video_range = rate.frames_covering_samples(range, sr);
        // Frames-covering-audio is used for `Samples` tails; an explicit
        // `Frames` tail adds literal frames on top.
        let tail_from_samples = if tail_samples > 0 {
            rate.frames_covering_samples(audio_total, sr) - video_range
        } else {
            0
        };
        let video_total = video_range
            .checked_add(tail_frames.max(tail_from_samples))
            .filter(|v| *v <= MAX_RENDER_VIDEO_FRAMES)
            .ok_or_else(|| AvError::InvalidSpec("video duration exceeds bound".into()))?;

        let cues = self
            .cues
            .iter()
            .map(|c| {
                let at = parse_u64("cue.atSample", &c.at_sample)?;
                if at >= audio_total {
                    return Err(AvError::InvalidSpec(format!(
                        "cue {} at sample {at} outside rendered program",
                        c.cue_id
                    )));
                }
                Ok(PlannedCue {
                    cue_id: c.cue_id.clone(),
                    at_sample: c.at_sample.clone(),
                    placement: place_cue(at, &rate, sr, self.rounding),
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(AvFramePlan {
            audio_range_samples: range.to_string(),
            audio_tail_samples: tail_samples.to_string(),
            audio_total_samples: audio_total.to_string(),
            video_range_frames: video_range.to_string(),
            video_tail_frames: video_total.saturating_sub(video_range).to_string(),
            video_total_frames: video_total.to_string(),
            frame_rate_num: rate.num,
            frame_rate_den: rate.den,
            cues,
        })
    }

    /// Exact `-t` duration string for the audio program in microseconds
    /// (`audio_total * 1e6 / sr`, rounded per spec policy, then rendered
    /// as `S.UUUUUU` — ffmpeg takes seconds; micro resolution keeps the
    /// cut sample-exact for all approved rates).
    pub fn duration_seconds(&self, plan: &AvFramePlan) -> Result<String> {
        let total = parse_u64("audioTotalSamples", &plan.audio_total_samples)?;
        let us = rational_round(
            total as u128 * 1_000_000,
            self.sample_rate as u128,
            self.rounding,
        );
        Ok(format!("{}.{:06}", us / 1_000_000, us % 1_000_000))
    }

    pub fn validate(&self) -> Result<()> {
        validate_uuid("jobId", &self.job_id)?;
        validate_uuid("projectId", &self.project_id)?;
        validate_uuid("checkpointId", &self.checkpoint_id)?;
        parse_u64("sourceRevision", &self.source_revision)?;
        for h in &self.asset_hashes {
            validate_sha256("assetHashes[]", h)?;
        }
        if !APPROVED_SAMPLE_RATES.contains(&self.sample_rate) {
            return Err(AvError::InvalidSpec(format!(
                "sampleRate {} not in approved set {APPROVED_SAMPLE_RATES:?}",
                self.sample_rate
            )));
        }
        self.frame_rate()?;
        if self.width == 0 || self.height == 0 {
            return Err(AvError::InvalidSpec("width/height must be non-zero".into()));
        }
        if self.width > MAX_DIMENSION || self.height > MAX_DIMENSION {
            return Err(AvError::InvalidSpec("pixel dimensions exceed bound".into()));
        }
        if self.codec_id.is_empty()
            || !self
                .codec_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
            || self.codec_id.len() > 64
        {
            return Err(AvError::InvalidSpec(
                "codecId must be [A-Za-z0-9._-] ≤64".into(),
            ));
        }
        let mut n_video = 0usize;
        let mut n_audio = 0usize;
        for i in &self.inputs {
            match i {
                AvInput::CheckpointFile {
                    rel_path,
                    sha256,
                    role,
                } => {
                    if rel_path.is_empty()
                        || rel_path.starts_with('/')
                        || rel_path.contains("..")
                        || rel_path.contains('\\')
                        || rel_path.len() > 512
                    {
                        return Err(AvError::InvalidSpec(format!(
                            "input relPath '{rel_path}' is not a safe relative path"
                        )));
                    }
                    validate_sha256("input.sha256", sha256)?;
                    bump(role, &mut n_video, &mut n_audio);
                }
                AvInput::AssetBlob { sha256, role } => {
                    validate_sha256("input.sha256", sha256)?;
                    bump(role, &mut n_video, &mut n_audio);
                }
                AvInput::LavfiTest { src, role } => {
                    // Lavfi sources still must not look like shell text —
                    // they are passed as a single -i argv element but we
                    // keep them to the lavfi grammar (no spaces, no
                    // shell metachars) so argv stays unambiguous.
                    if src.is_empty()
                        || src.len() > 512
                        || !src.chars().all(|c| {
                            c.is_ascii_alphanumeric()
                                || matches!(c, ':' | '=' | '_' | '-' | '.' | ',' | '\'')
                        })
                    {
                        return Err(AvError::InvalidSpec(format!(
                            "lavfi source '{src}' contains unsupported characters"
                        )));
                    }
                    bump(role, &mut n_video, &mut n_audio);
                }
            }
        }
        if n_video != 1 || n_audio != 1 {
            return Err(AvError::InvalidSpec(format!(
                "spec requires exactly one video and one audio input (got {n_video}v/{n_audio}a)"
            )));
        }
        for c in &self.cues {
            if c.cue_id.is_empty() || c.cue_id.len() > 64 {
                return Err(AvError::InvalidSpec("cueId empty or too long".into()));
            }
            parse_u64("cue.atSample", &c.at_sample)?;
        }
        let timeout = parse_u64("timeoutMs", &self.timeout_ms)?;
        if timeout == 0 || timeout > MAX_TIMEOUT_MS {
            return Err(AvError::InvalidSpec(format!(
                "timeoutMs {timeout} outside 1..={MAX_TIMEOUT_MS}"
            )));
        }
        let cap = parse_u64("maxOutputBytes", &self.max_output_bytes)?;
        if cap == 0 || cap > MAX_OUTPUT_BYTES {
            return Err(AvError::InvalidSpec(format!(
                "maxOutputBytes {cap} outside 1..={MAX_OUTPUT_BYTES}"
            )));
        }
        sanitize_output_name(&self.output_name).map_err(|e| match e {
            void_export::ExportError::InvalidSpec(m) => AvError::InvalidSpec(m),
            other => AvError::InvalidSpec(other.to_string()),
        })?;
        // Leading dash is unsafe for argv-shaped subprocesses even
        // though it's legal in a filename — the artifact path could be
        // parsed as a flag by a sloppy downstream tool.
        if self.output_name.starts_with('-') {
            return Err(AvError::InvalidSpec(
                "outputName may not start with '-'".into(),
            ));
        }
        if let Some(d) = &self.deadline_monotonic_ns {
            parse_u64("deadlineMonotonicNs", d)?;
        }
        // Full plan computation doubles as validation of every bound.
        self.frame_plan()?;
        Ok(())
    }
}

fn bump(role: &AvInputRole, v: &mut usize, a: &mut usize) {
    match role {
        AvInputRole::Video => *v += 1,
        AvInputRole::Audio => *a += 1,
    }
}

/// UUID shape check (§1 string identifiers).
pub fn validate_uuid(field: &str, s: &str) -> Result<()> {
    uuid::Uuid::parse_str(s)
        .map(|_| ())
        .map_err(|_| AvError::InvalidSpec(format!("{field} is not a uuid: '{s}'")))
}

/// Lowercase 64-hex sha256 check.
pub fn validate_sha256(field: &str, s: &str) -> Result<()> {
    if s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    {
        Ok(())
    } else {
        Err(AvError::InvalidSpec(format!("{field} not sha256 hex")))
    }
}

/// Strict decimal-u64 parser shared by every decimal-string field.
pub fn parse_u64(field: &str, s: &str) -> Result<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(AvError::InvalidSpec(format!(
            "{field} must be a decimal u64, got '{s}'"
        )));
    }
    s.parse::<u64>()
        .map_err(|_| AvError::InvalidSpec(format!("{field} out of u64 range: '{s}'")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> AvExportSpec {
        AvExportSpec {
            job_id: "1d7e6f4f-7b42-4c20-9a9f-7d0f5b6f1f11".into(),
            project_id: "9b30b5c0-1c0d-4b76-8f6e-4f1c9a11a001".into(),
            checkpoint_id: "5f6e7a8b-9c0d-4e1f-a2b3-c4d5e6f7a8b9".into(),
            source_revision: "7".into(),
            asset_hashes: vec!["a".repeat(64)],
            range_samples: "48000".into(),
            sample_rate: 48_000,
            channels: ChannelLayout::Stereo,
            frame_rate_num: 30000,
            frame_rate_den: 1001,
            rounding: FrameRounding::NearestTiesAway,
            width: 320,
            height: 240,
            codec_id: "ffv1_flac_mkv".into(),
            inputs: vec![
                AvInput::LavfiTest {
                    src: "testsrc2=size=320x240".into(),
                    role: AvInputRole::Video,
                },
                AvInput::LavfiTest {
                    src: "sine=frequency=440".into(),
                    role: AvInputRole::Audio,
                },
            ],
            cues: vec![AvCue {
                cue_id: "drop".into(),
                at_sample: "24024".into(),
            }],
            tail: AvTailPolicy::None,
            output_name: "song-viz".into(),
            timeout_ms: "30000".into(),
            max_output_bytes: "10485760".into(),
            deadline_monotonic_ns: None,
        }
    }

    #[test]
    fn valid_spec_plans() {
        let s = base();
        s.validate().unwrap();
        let p = s.frame_plan().unwrap();
        // 1 s at 30000/1001 → ceil(29.97…) = 30 frames… no: ceil(48000*30000/48048000)=ceil(29.97)=30.
        assert_eq!(p.video_total_frames, "30");
        assert_eq!(p.audio_total_samples, "48000");
        assert_eq!(p.cues[0].placement.frame_index, 15);
        assert_eq!(p.cues[0].placement.offset_samples, 0);
    }

    #[test]
    fn rejects_two_video_inputs() {
        let mut s = base();
        s.inputs.push(AvInput::LavfiTest {
            src: "testsrc2=size=64x64".into(),
            role: AvInputRole::Video,
        });
        assert!(s.validate().is_err());
    }

    #[test]
    fn rejects_unsafe_output_name() {
        let mut s = base();
        s.output_name = "../escape.mp4".into();
        assert!(s.validate().is_err());
    }

    #[test]
    fn rejects_shellish_lavfi() {
        let mut s = base();
        s.inputs[0] = AvInput::LavfiTest {
            src: "x; rm -rf /".into(),
            role: AvInputRole::Video,
        };
        assert!(s.validate().is_err());
    }

    #[test]
    fn rejects_zero_timeout_and_huge_cap() {
        let mut s = base();
        s.timeout_ms = "0".into();
        assert!(s.validate().is_err());
        s.timeout_ms = "30000".into();
        s.max_output_bytes = (MAX_OUTPUT_BYTES + 1).to_string();
        assert!(s.validate().is_err());
    }

    #[test]
    fn frames_tail_extends_video_only() {
        let mut s = base();
        s.tail = AvTailPolicy::Frames { frames: 10 };
        let p = s.frame_plan().unwrap();
        assert_eq!(p.video_total_frames, "40");
        assert_eq!(p.audio_total_samples, "48000");
    }
}
