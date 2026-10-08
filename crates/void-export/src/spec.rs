//! Render/export specification (CONTRACTS.md §7).
//!
//! Inputs are immutable: `checkpoint_id` + `source_revision` pin the exact
//! captured state, `asset_hashes` lists the immutable blobs the render may
//! read, and every frame/tail count is computed here — explicitly, with
//! documented rounding — so the renderer is never left to re-decide them.
//!
//! Large integers are decimal strings in JSON, matching the codec
//! contract (`crates/void-protocol`, `packages/void-client`).

use crate::error::{ExportError, Result};
use serde::{Deserialize, Serialize};

/// 960,000 ticks per quarter note (CONTRACTS.md §1).
pub const TICKS_PER_QUARTER: u64 = 960_000;

/// Sample rates the export contract approves for v1.
pub const APPROVED_SAMPLE_RATES: [u32; 4] = [44_100, 48_000, 88_200, 96_000];

/// Bound on computed output frames (≈6h at 96 kHz) — a render is a
/// bounded job, not an unbounded one (§2 resource policy).
pub const MAX_RENDER_FRAMES: u64 = 96_000 * 6 * 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportFormat {
    /// PCM/float WAV — `channels`/`bit_depth` apply.
    Wav,
    /// Standard MIDI file — channels/bit depth do not apply.
    Midi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelLayout {
    Mono,
    Stereo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BitDepth {
    Pcm16,
    Pcm24,
    Float32,
}

/// How much audio beyond `range_end` is rendered to let releases/reverb
/// finish. Explicit and bounded — never renderer-implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum TailPolicy {
    /// Hard cut at `range_end_ticks` — `tail_frames` is 0.
    None,
    /// Extra milliseconds after the range end.
    Milliseconds { ms: u64 },
    /// Extra musical ticks after the range end.
    Ticks { ticks: i64 },
}

/// One tempo-map segment for ticks→frames conversion. The engine owns the
/// authoritative map; the export DTO carries a simplified piecewise list so
/// frame counts are computable without the engine (nonlinear maps are an
/// engine-side refinement — callers pass the *effective* segments).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TempoSegment {
    /// Tick at which this tempo starts (0 for the first segment).
    pub at_ticks: String,
    /// BPM, finite and > 0.
    pub bpm: f64,
}

fn default_tail() -> TailPolicy {
    TailPolicy::None
}

/// The immutable render input set. Everything the renderer needs is here;
/// nothing may be looked up live after submit — that is the "fixed
/// snapshot" rule of T42.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportSpec {
    /// UUID of this export job.
    pub job_id: String,
    pub project_id: String,
    /// Immutable checkpoint the render reads (§7) — must be a verified,
    /// complete checkpoint in the container; validated against CURRENT
    /// lineage by the caller, shape-checked here.
    pub checkpoint_id: String,
    /// Coordinator revision the checkpoint represents — decimal u64.
    pub source_revision: String,
    /// SHA-256 hashes of the immutable asset blobs the render may read.
    #[serde(default)]
    pub asset_hashes: Vec<String>,
    /// Musical range to render: [start, end) ticks; `end > start`.
    pub range_start_ticks: String,
    pub range_end_ticks: String,
    pub format: ExportFormat,
    /// Required for `wav`; must be absent for `midi`.
    #[serde(default)]
    pub channels: Option<ChannelLayout>,
    /// Required for `wav`; must be absent for `midi`.
    #[serde(default)]
    pub bit_depth: Option<BitDepth>,
    /// Required for `wav`; must be absent for `midi`.
    #[serde(default)]
    pub sample_rate: Option<u32>,
    #[serde(default = "default_tail")]
    pub tail: TailPolicy,
    /// Piecewise tempo map used for the explicit frame computation.
    /// Must cover [0, range_end + tail); first segment must start at 0.
    pub tempo_map: Vec<TempoSegment>,
    /// Scoped output base name — sanitized `[A-Za-z0-9._-]`, no slashes.
    /// The extension comes from `format`; the path is never taken from
    /// user text verbatim (§7 scoped filenames).
    pub output_name: String,
    /// Optional render deadline — monotonic ns, decimal string (§6 job).
    #[serde(default)]
    pub deadline_monotonic_ns: Option<String>,
}

/// Explicit frame plan derived from the spec. Carried on the wire so the
/// renderer applies these counts verbatim — rounding is decided here.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FramePlan {
    /// Frames in [range_start, range_end) at `sample_rate`.
    pub range_frames: String,
    /// Extra frames appended for the tail policy.
    pub tail_frames: String,
    /// range_frames + tail_frames — the total the renderer must produce.
    pub total_frames: String,
}

fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

/// Convert `ticks` to sample frames under `bpm` at `sample_rate`, rounding
/// to nearest with ties away from zero (CONTRACTS.md §1).
///
/// frames = ticks * sample_rate * 60 / (TICKS_PER_QUARTER * bpm)
///
/// Computed in u128 integer arithmetic after turning bpm into a rational
/// (micro-bpm numerator): bpm is rounded to 6 decimal places of precision
/// so the conversion is exact, overflow-checked and reproducible.
pub fn ticks_to_frames(ticks: i64, bpm: f64, sample_rate: u32) -> Result<u64> {
    if ticks < 0 {
        return Err(ExportError::InvalidSpec(format!(
            "cannot render a negative tick length: {ticks}"
        )));
    }
    if !bpm.is_finite() || bpm <= 0.0 {
        return Err(ExportError::InvalidSpec(format!(
            "bpm must be finite and > 0, got {bpm}"
        )));
    }
    // micro-bpm: 6 decimals keep tempo-map precision well below one frame.
    let bpm_micro = (bpm * 1_000_000.0).round() as u128;
    if bpm_micro == 0 {
        return Err(ExportError::InvalidSpec("bpm rounds to 0".into()));
    }
    // frames = ticks * sr * 60 * 1_000_000 / (960_000 * bpm_micro)
    let num = (ticks as u128)
        .checked_mul(sample_rate as u128)
        .and_then(|n| n.checked_mul(60_000_000u128))
        .ok_or_else(|| ExportError::InvalidSpec("frame count overflow".into()))?;
    let den = (TICKS_PER_QUARTER as u128)
        .checked_mul(bpm_micro)
        .ok_or_else(|| ExportError::InvalidSpec("frame count overflow".into()))?;
    // Nearest, ties away from zero: (num + den/2) / den for positive num.
    let frames = (num + den / 2) / den;
    u64::try_from(frames).map_err(|_| ExportError::InvalidSpec("frame count exceeds u64".into()))
}

/// Frames covered by [from_ticks, to_ticks) across a piecewise tempo map —
/// each segment contributes its own span so nonlinear maps stay exact.
pub fn range_to_frames(
    from_ticks: i64,
    to_ticks: i64,
    tempo_map: &[TempoSegment],
    sample_rate: u32,
) -> Result<u64> {
    if tempo_map.is_empty() {
        return Err(ExportError::InvalidSpec("tempo_map is empty".into()));
    }
    let mut segs: Vec<(i64, f64)> = Vec::with_capacity(tempo_map.len());
    for s in tempo_map {
        let at: i64 = s
            .at_ticks
            .trim()
            .parse()
            .map_err(|_| ExportError::InvalidSpec("tempo segment at_ticks not i64".into()))?;
        if !s.bpm.is_finite() || s.bpm <= 0.0 {
            return Err(ExportError::InvalidSpec(format!(
                "tempo bpm must be finite and > 0, got {}",
                s.bpm
            )));
        }
        segs.push((at, s.bpm));
    }
    segs.sort_by_key(|(at, _)| *at);
    if segs[0].0 > 0 {
        return Err(ExportError::InvalidSpec(
            "tempo_map first segment must start at tick 0".into(),
        ));
    }
    for w in segs.windows(2) {
        if w[1].0 <= w[0].0 {
            return Err(ExportError::InvalidSpec(
                "tempo_map segments must be strictly increasing".into(),
            ));
        }
    }
    let mut frames: u64 = 0;
    for (i, (at, bpm)) in segs.iter().enumerate() {
        let seg_end = segs.get(i + 1).map(|(a, _)| *a).unwrap_or(i64::MAX);
        let lo = from_ticks.max(*at);
        let hi = to_ticks.min(seg_end);
        if hi > lo {
            frames = frames
                .checked_add(ticks_to_frames(hi - lo, *bpm, sample_rate)?)
                .ok_or_else(|| ExportError::InvalidSpec("frame count overflow".into()))?;
        }
    }
    Ok(frames)
}

/// Sanitize a user-supplied base name into a scoped filename component:
/// `[A-Za-z0-9._-]` only, no leading dot, no slashes, ≤ 120 chars.
pub fn sanitize_output_name(name: &str) -> Result<String> {
    let n = name.trim();
    if n.is_empty() || n.len() > 120 {
        return Err(ExportError::InvalidSpec(format!(
            "output name empty or oversized ({})",
            n.len()
        )));
    }
    if n.starts_with('.')
        || !n
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
    {
        return Err(ExportError::InvalidSpec(format!(
            "output name {n:?} is not a scoped filename"
        )));
    }
    Ok(n.to_string())
}

impl ExportSpec {
    /// Shape + semantic validation. Rejects bad ranges, mismatched
    /// format/channel/depth combos, unapproved rates and unsafe names.
    pub fn validate(&self) -> Result<()> {
        let bad = |m: &str| ExportError::InvalidSpec(m.into());
        if !void_protocol::ids::is_valid_id(&self.job_id) {
            return Err(bad("jobId not a UUID"));
        }
        if !void_protocol::ids::is_valid_id(&self.project_id) {
            return Err(bad("projectId not a UUID"));
        }
        if !void_protocol::ids::is_valid_id(&self.checkpoint_id) {
            return Err(bad("checkpointId not a UUID"));
        }
        if self.source_revision.parse::<u64>().is_err() {
            return Err(bad("sourceRevision not a decimal u64"));
        }
        if self.asset_hashes.len() > 128 {
            return Err(bad("assetHashes exceeds 128 entries"));
        }
        for h in &self.asset_hashes {
            if !is_sha256_hex(h) {
                return Err(bad("assetHashes[] not sha256 hex"));
            }
        }
        let start: i64 = self
            .range_start_ticks
            .trim()
            .parse()
            .map_err(|_| bad("rangeStartTicks not i64"))?;
        let end: i64 = self
            .range_end_ticks
            .trim()
            .parse()
            .map_err(|_| bad("rangeEndTicks not i64"))?;
        if end <= start {
            return Err(bad("rangeEndTicks must be > rangeStartTicks"));
        }
        match self.format {
            ExportFormat::Wav => {
                if self.channels.is_none() {
                    return Err(bad("wav export requires channels"));
                }
                if self.bit_depth.is_none() {
                    return Err(bad("wav export requires bitDepth"));
                }
                match self.sample_rate {
                    Some(r) if APPROVED_SAMPLE_RATES.contains(&r) => {}
                    Some(_) => return Err(bad("sampleRate not in approved set")),
                    None => return Err(bad("wav export requires sampleRate")),
                }
            }
            ExportFormat::Midi => {
                if self.channels.is_some() || self.bit_depth.is_some() || self.sample_rate.is_some()
                {
                    return Err(bad(
                        "midi export must not carry channels/bitDepth/sampleRate",
                    ));
                }
            }
        }
        if let TailPolicy::Ticks { ticks } = self.tail {
            if ticks < 0 {
                return Err(bad("tail ticks must be >= 0"));
            }
        }
        if let TailPolicy::Milliseconds { ms } = self.tail {
            if ms > 60_000 {
                return Err(bad("tail ms exceeds 60 s bound"));
            }
        }
        if let Some(d) = &self.deadline_monotonic_ns {
            if d.parse::<u64>().is_err() {
                return Err(bad("deadlineMonotonicNs not a decimal u64"));
            }
        }
        sanitize_output_name(&self.output_name)?;
        // Frame plan must be computable — validation catches it early.
        self.frame_plan()?;
        Ok(())
    }

    /// The explicit frame plan: exact range + tail frames at the declared
    /// sample rate. MIDI exports also carry one (the engine still needs a
    /// render bound for its own audio tail), computed at 48 kHz.
    pub fn frame_plan(&self) -> Result<FramePlan> {
        let sr = match self.format {
            ExportFormat::Wav => self.sample_rate.unwrap_or(48_000),
            ExportFormat::Midi => 48_000,
        };
        let start: i64 = self.range_start_ticks.trim().parse().unwrap_or(0);
        let end: i64 = self.range_end_ticks.trim().parse().unwrap_or(0);
        let range_frames = range_to_frames(start, end, &self.tempo_map, sr)?;
        let tail_frames = match self.tail {
            TailPolicy::None => 0,
            TailPolicy::Milliseconds { ms } => {
                // ms * sr / 1000 — exact, already integral-safe.
                ((ms as u128 * sr as u128 + 500) / 1000) as u64
            }
            TailPolicy::Ticks { ticks } => range_to_frames(end, end + ticks, &self.tempo_map, sr)?,
        };
        let total = range_frames
            .checked_add(tail_frames)
            .ok_or_else(|| ExportError::InvalidSpec("frame total overflow".into()))?;
        if total > MAX_RENDER_FRAMES {
            return Err(ExportError::InvalidSpec(format!(
                "render exceeds {MAX_RENDER_FRAMES} frames bound"
            )));
        }
        Ok(FramePlan {
            range_frames: range_frames.to_string(),
            tail_frames: tail_frames.to_string(),
            total_frames: total.to_string(),
        })
    }

    /// File extension for the declared format.
    pub fn extension(&self) -> &'static str {
        match self.format {
            ExportFormat::Wav => "wav",
            ExportFormat::Midi => "mid",
        }
    }

    /// Scoped output filename (`<sanitized name>.<ext>`).
    pub fn output_file_name(&self) -> Result<String> {
        Ok(format!(
            "{}.{}",
            sanitize_output_name(&self.output_name)?,
            self.extension()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> ExportSpec {
        ExportSpec {
            job_id: "00000000-0000-4000-8000-000000000001".into(),
            project_id: "00000000-0000-4000-8000-000000000002".into(),
            checkpoint_id: "00000000-0000-4000-8000-000000000003".into(),
            source_revision: "42".into(),
            asset_hashes: vec!["a".repeat(64)],
            range_start_ticks: "0".into(),
            range_end_ticks: "3840000".into(), // one bar of 4/4
            format: ExportFormat::Wav,
            channels: Some(ChannelLayout::Stereo),
            bit_depth: Some(BitDepth::Pcm24),
            sample_rate: Some(48_000),
            tail: TailPolicy::None,
            tempo_map: vec![TempoSegment {
                at_ticks: "0".into(),
                bpm: 120.0,
            }],
            output_name: "mix-v1".into(),
            deadline_monotonic_ns: None,
        }
    }

    #[test]
    fn valid_spec_passes() {
        assert!(spec().validate().is_ok());
    }

    #[test]
    fn rejects_inverted_range() {
        let mut s = spec();
        s.range_start_ticks = "3840000".into();
        s.range_end_ticks = "0".into();
        assert!(matches!(s.validate(), Err(ExportError::InvalidSpec(_))));
    }

    #[test]
    fn rejects_mismatched_format_fields() {
        let mut s = spec();
        s.channels = None;
        assert!(s.validate().is_err());
        let mut s = spec();
        s.format = ExportFormat::Midi;
        assert!(s.validate().is_err()); // midi must not carry channel/rate/depth
        let mut s = spec();
        s.format = ExportFormat::Midi;
        s.channels = None;
        s.bit_depth = None;
        s.sample_rate = None;
        assert!(s.validate().is_ok());
    }

    #[test]
    fn rejects_unapproved_sample_rate_and_bad_name() {
        let mut s = spec();
        s.sample_rate = Some(22_050);
        assert!(s.validate().is_err());
        let mut s = spec();
        s.output_name = "../escape".into();
        assert!(s.validate().is_err());
        let mut s = spec();
        s.output_name = "a/b".into();
        assert!(s.validate().is_err());
    }

    #[test]
    fn ticks_to_frames_is_exact_with_ties_away() {
        // 1 bar of 4/4 @120bpm @48k = 96000 frames exactly.
        assert_eq!(ticks_to_frames(3_840_000, 120.0, 48_000).unwrap(), 96_000);
        // 960_000 ticks @120bpm @44.1k = 22050 exactly.
        assert_eq!(ticks_to_frames(960_000, 120.0, 44_100).unwrap(), 22_050);
        // Rounding: 1 tick @120bpm @44.1k → 22050/960000*... = 0.023 → 0.
        assert_eq!(ticks_to_frames(1, 120.0, 44_100).unwrap(), 0);
        // 43 ticks @120 @44100: 43*44100*60/(960000*120)= 0.9909… → 1
        assert_eq!(ticks_to_frames(43, 120.0, 44_100).unwrap(), 1);
        // Tie-away case: pick ticks where fraction = exactly 0.5.
        // frames = ticks*sr*60/(960000*bpm); want fraction .5 → e.g.
        // ticks=8, bpm=120, sr=22050-ish… choose sr 48000,bpm 60:
        // per-tick frames = 48000*60/(960000*60) = 0.05; 10 ticks → 0.5 → 1 (away from 0)
        assert_eq!(ticks_to_frames(10, 60.0, 48_000).unwrap(), 1);
    }

    #[test]
    fn piecewise_tempo_map_sums_segments() {
        // Bar 0 @120 (96000f) + bar 1 @60 (192000f) = 288000 total.
        let map = vec![
            TempoSegment {
                at_ticks: "0".into(),
                bpm: 120.0,
            },
            TempoSegment {
                at_ticks: "3840000".into(),
                bpm: 60.0,
            },
        ];
        let f = range_to_frames(0, 7_680_000, &map, 48_000).unwrap();
        assert_eq!(f, 288_000);
    }

    #[test]
    fn frame_plan_is_explicit_and_bounded() {
        let s = spec();
        let p = s.frame_plan().unwrap();
        assert_eq!(p.range_frames, "96000");
        assert_eq!(p.tail_frames, "0");
        assert_eq!(p.total_frames, "96000");
        // tail in ticks: +1 beat @120 @48k = 24000
        let mut s2 = spec();
        s2.tail = TailPolicy::Ticks { ticks: 960_000 };
        let p2 = s2.frame_plan().unwrap();
        assert_eq!(p2.tail_frames, "24000");
        assert_eq!(p2.total_frames, "120000");
        // tail in ms: 500ms @48k = 24000
        let mut s3 = spec();
        s3.tail = TailPolicy::Milliseconds { ms: 500 };
        assert_eq!(s3.frame_plan().unwrap().tail_frames, "24000");
    }

    #[test]
    fn rejects_negative_tail_and_bad_hash() {
        let mut s = spec();
        s.tail = TailPolicy::Ticks { ticks: -5 };
        assert!(s.validate().is_err());
        let mut s = spec();
        s.asset_hashes = vec!["ZZ".repeat(32)];
        assert!(s.validate().is_err());
    }
}
