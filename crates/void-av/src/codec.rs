//! Declarative codec matrix (W24 / T88): `codec_id → argv template +
//! rights flag`. Selection is gated on (a) the id existing here and
//! (b) the detected ffmpeg build actually containing both encoders —
//! either gap is a typed `CodecUnavailable`, never a fake success.
//!
//! Rights semantics recorded here (CONTRACTS.md §7 "codec/build rights
//! documented"): every entry carries a `CodecRights` flag that lands in
//! provenance so a published artifact provably records whether its
//! codec path was distribution-cleared. `DevelopmentOnly` means the
//! encode may run (dev/test builds) but the artifact is not cleared for
//! redistribution — see NEEDS.md (patent/licensing audit is a release
//! decision, D06-class).

use crate::error::{AvError, Result};
use serde::{Deserialize, Serialize};

/// Recorded distribution rights for a codec id (T88 rights flag).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CodecRights {
    /// Royalties/licence state recorded as cleared for distribution of
    /// encoder output (codec specs only — the ffmpeg build licence is
    /// still the deployer's responsibility).
    Cleared,
    /// Usable in development and CI; output is not cleared for
    /// redistribution until a rights review lands (D06-class decision).
    DevelopmentOnly,
}

/// One row of the codec matrix: a complete, declarative argv template.
/// `extra_args` are fixed argv fragments — they are data, not user
/// input, and are appended verbatim to the argv array.
#[derive(Debug, Clone)]
pub struct CodecSpec {
    pub id: &'static str,
    /// Container/extension written by this codec row.
    pub container_ext: &'static str,
    /// ffmpeg video encoder name (`-c:v`).
    pub video_encoder: &'static str,
    /// ffmpeg audio encoder name (`-c:a`).
    pub audio_encoder: &'static str,
    /// ffmpeg `-pix_fmt` value for the video stream.
    pub pixel_format: &'static str,
    /// Extra fixed argv (quality flags, mux flags, movflags, …).
    pub extra_args: &'static [&'static str],
    pub rights: CodecRights,
    /// Human-readable licence note, surfaced in provenance + UI.
    pub license_note: &'static str,
}

/// The declared codec matrix. Addition is deliberate: a codec is
/// selectable iff it is listed AND the encoder pair exists in the
/// detected tool build.
pub const CODEC_MATRIX: &[CodecSpec] = &[
    CodecSpec {
        id: "ffv1_flac_mkv",
        container_ext: "mkv",
        video_encoder: "ffv1",
        audio_encoder: "flac",
        pixel_format: "yuv420p",
        extra_args: &[],
        rights: CodecRights::Cleared,
        license_note: "FFV1 + FLAC in Matroska — lossless, royalty-free (BSD)",
    },
    CodecSpec {
        id: "vp9_opus_webm",
        container_ext: "webm",
        video_encoder: "libvpx-vp9",
        audio_encoder: "libopus",
        pixel_format: "yuv420p",
        extra_args: &["-b:v", "0", "-crf", "32"],
        rights: CodecRights::Cleared,
        license_note: "VP9 + Opus in WebM — royalty-free (BSD/Google patent grant)",
    },
    CodecSpec {
        id: "h264_aac_mp4",
        container_ext: "mp4",
        video_encoder: "libx264",
        audio_encoder: "aac",
        pixel_format: "yuv420p",
        extra_args: &["-movflags", "+faststart"],
        rights: CodecRights::DevelopmentOnly,
        license_note: "H.264 + AAC — patent-pool codecs; x264 is GPL; output not cleared for redistribution (D06)",
    },
    CodecSpec {
        id: "prores_pcm_mov",
        container_ext: "mov",
        video_encoder: "prores_ks",
        audio_encoder: "pcm_s16le",
        pixel_format: "yuv422p10le",
        extra_args: &["-profile:v", "3"],
        rights: CodecRights::DevelopmentOnly,
        license_note: "Apple ProRes + PCM — trademark/format rights not audited for redistribution (D06)",
    },
];

pub fn codec(id: &str) -> Result<&'static CodecSpec> {
    CODEC_MATRIX
        .iter()
        .find(|c| c.id == id)
        .ok_or_else(|| AvError::CodecUnavailable(format!("unknown codec id '{id}'")))
}

/// Gate a codec row against the detected encoder set. `encoders` are
/// the encoder names reported by `ffmpeg -encoders` (or the declared
/// list of the fake). Returns the row for chaining.
pub fn require_encoders(
    row: &'static CodecSpec,
    encoders: &dyn Fn(&str) -> bool,
) -> Result<&'static CodecSpec> {
    if !encoders(row.video_encoder) {
        return Err(AvError::CodecUnavailable(format!(
            "codec '{}' needs video encoder '{}' — not in detected ffmpeg build",
            row.id, row.video_encoder
        )));
    }
    if !encoders(row.audio_encoder) {
        return Err(AvError::CodecUnavailable(format!(
            "codec '{}' needs audio encoder '{}' — not in detected ffmpeg build",
            row.id, row.audio_encoder
        )));
    }
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_id_is_codec_unavailable() {
        match codec("hevc_silk_mkv") {
            Err(AvError::CodecUnavailable(m)) => assert!(m.contains("hevc_silk_mkv")),
            _ => panic!("expected CodecUnavailable"),
        }
    }

    #[test]
    fn every_row_has_consistent_fields() {
        for r in CODEC_MATRIX {
            assert!(!r.id.is_empty() && !r.container_ext.is_empty());
            assert!(!r.video_encoder.is_empty() && !r.audio_encoder.is_empty());
            assert!(!r.license_note.is_empty());
        }
    }

    #[test]
    fn encoder_gate_reports_missing_side() {
        let row = codec("ffv1_flac_mkv").unwrap();
        let e = require_encoders(row, &|n| n == "ffv1");
        match e {
            Err(AvError::CodecUnavailable(m)) => assert!(m.contains("flac")),
            _ => panic!("expected CodecUnavailable for missing flac"),
        }
    }
}
