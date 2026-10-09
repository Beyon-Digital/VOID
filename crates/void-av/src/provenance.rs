//! av-export provenance (CONTRACTS.md §7, T87/T89). Written inside the
//! staging dir before publish; carried across the atomic rename.
//!
//! Honest-nondeterminism rule: provenance records the *declared* plan,
//! the *measured* probe, and the per-cue expected-vs-measured drift. It
//! does NOT claim bit-identical rerenders — GPU/visual inference output
//! is not reproducible across runs, and this file is where that is said
//! out loud rather than implied (T89 "no bit-identical claim").

use crate::codec::CodecRights;
use crate::probe::ProbeInfo;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvArtifactRecord {
    /// Basename inside the published dir.
    pub file: String,
    /// sha256 of the artifact bytes, lowercase hex.
    pub sha256: String,
    /// Bytes on disk — decimal string.
    pub bytes: String,
    /// Video frames probe counted — decimal string.
    pub video_frames: String,
    /// Video frames the plan declared — decimal string.
    pub declared_video_frames: String,
    /// Audio samples probe measured — decimal string.
    pub audio_samples: String,
    /// Audio samples the plan declared — decimal string.
    pub declared_audio_samples: String,
}

/// Per-cue expected-vs-measured placement (T87 cue-timestamp evidence).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CueReport {
    pub cue_id: String,
    /// Expected audio sample — decimal string.
    pub at_sample: String,
    /// Frame the cue lands in — decimal string.
    pub frame_index: String,
    /// Sample where that frame starts — decimal string.
    pub frame_start_sample: String,
    /// Samples the cue is into its frame — the measured drift.
    pub offset_samples: String,
    /// Declared tolerance = one video frame; `true` when within it.
    pub within_tolerance: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AvProvenance {
    pub format_version: u32,
    pub job_id: String,
    pub project_id: String,
    pub checkpoint_id: String,
    pub source_revision: String,
    /// sha256 of the serialized spec (config hash — the *entire* spec
    /// drives the outcome, so the config hash covers all of it).
    pub config_sha256: String,
    /// sha256 of the canonical argv encoding.
    pub argv_sha256: String,
    /// Tool version line(s) reported by the encoder + probe.
    pub tool_versions: Vec<String>,
    /// Codec matrix row that was selected, plus its rights flag.
    pub codec_id: String,
    pub codec_rights: CodecRights,
    pub codec_license_note: String,
    /// Immutable input pins.
    pub checkpoint_manifest_sha256: String,
    pub asset_hashes: Vec<String>,
    /// Declared plan — decimal strings.
    pub plan: serde_json::Value,
    /// Measured artifact facts.
    pub artifact: AvArtifactRecord,
    /// Per-cue measured drift evidence.
    pub cues: Vec<CueReport>,
    /// Renderer name + exe basename (never a full path).
    pub renderer: String,
    pub encoder_exe: String,
    pub probe_exe: String,
    /// Wall-clock seconds the encode took — decimal string.
    pub encode_wall_ms: String,
    /// Honest statement, not a claim: what a rerun can and cannot
    /// guarantee. Always emitted (T89).
    pub nondeterminism_note: String,
    /// RFC3339-ish UTC timestamp (civil-from-days, no chrono dep).
    pub created_at: String,
}

pub const NONDETERMINISM_NOTE: &str =
    "Deterministic inputs (checkpoint, asset pins, lavfi fixtures) reproduce this container's \
     frame/sample counts, but bit-identical rerenders are NOT claimed: encoder threading, GPU \
     decode, and generated/visual-inference inputs may differ run-to-run. Cue placements are \
     verified within the declared one-frame tolerance, not to the sample.";

pub fn cue_reports(plan: &crate::spec::AvFramePlan) -> Vec<CueReport> {
    plan.cues
        .iter()
        .map(|c| CueReport {
            cue_id: c.cue_id.clone(),
            at_sample: c.at_sample.clone(),
            frame_index: c.placement.frame_index.to_string(),
            frame_start_sample: c.placement.frame_start_sample.to_string(),
            offset_samples: c.placement.offset_samples.to_string(),
            within_tolerance: c.placement.within_cue_tolerance,
        })
        .collect()
}

pub fn artifact_record(
    file: String,
    sha256: String,
    probe: &ProbeInfo,
    plan: &crate::spec::AvFramePlan,
) -> AvArtifactRecord {
    AvArtifactRecord {
        file,
        sha256,
        bytes: probe.bytes.to_string(),
        video_frames: probe.video_frames.to_string(),
        declared_video_frames: plan.video_total_frames.clone(),
        audio_samples: probe.audio_samples.unwrap_or(0).to_string(),
        declared_audio_samples: plan.audio_total_samples.clone(),
    }
}
