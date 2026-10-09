//! Minimal RIFF/WAVE verification. We do not decode audio — we prove the
//! staged file's declared header matches the export spec (channels, rate,
//! depth, frame count) before it can publish. That is the "verified
//! completion" boundary of §7/T42; a file that merely exists is not a
//! completed render.

use crate::error::{ExportError, Result};
use crate::spec::{BitDepth, ChannelLayout, ExportSpec};
use std::io::{Read, Seek, SeekFrom};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WavInfo {
    pub audio_format: u16,
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    /// PCM frames declared by the data chunk.
    pub data_frames: u64,
}

fn tag(have: &[u8], want: &str) -> Result<()> {
    if have != want.as_bytes() {
        return Err(ExportError::VerifyFailed(format!(
            "expected {want} tag, got {have:?}"
        )));
    }
    Ok(())
}

/// Parse the RIFF/WAVE header of `path`. Rejects non-WAVE or truncated
/// files and files whose `fmt ` chunk is missing.
pub fn probe_wav(path: &std::path::Path) -> Result<WavInfo> {
    let mut f = std::io::BufReader::new(std::fs::File::open(path)?);
    let mut h = [0u8; 12];
    f.read_exact(&mut h)
        .map_err(|_| ExportError::VerifyFailed("file too small for RIFF header".into()))?;
    tag(&h[0..4], "RIFF")?;
    tag(&h[8..12], "WAVE")?;

    let mut fmt: Option<(u16, u16, u32, u16)> = None;
    let mut data_bytes: Option<u64> = None;
    loop {
        let mut ch = [0u8; 8];
        match f.read_exact(&mut ch) {
            Ok(()) => {}
            Err(_) => break,
        }
        let size = u32::from_le_bytes([ch[4], ch[5], ch[6], ch[7]]) as u64;
        match &ch[0..4] {
            b"fmt " => {
                if size < 16 {
                    return Err(ExportError::VerifyFailed("fmt chunk < 16 bytes".into()));
                }
                let mut b = [0u8; 16];
                f.read_exact(&mut b)?;
                fmt = Some((
                    u16::from_le_bytes([b[0], b[1]]),
                    u16::from_le_bytes([b[2], b[3]]),
                    u32::from_le_bytes([b[4], b[5], b[6], b[7]]),
                    u16::from_le_bytes([b[14], b[15]]),
                ));
                // remaining fmt bytes (extensible) are skipped below
                f.seek(SeekFrom::Current(size as i64 - 16))?;
            }
            b"data" => {
                data_bytes = Some(size);
                f.seek(SeekFrom::Current(size as i64))?;
            }
            _ => {
                f.seek(SeekFrom::Current(size as i64))?;
            }
        }
        // Chunks are word-aligned.
        if size % 2 == 1 {
            let _ = f.seek(SeekFrom::Current(1));
        }
    }

    let (audio_format, channels, sample_rate, bits_per_sample) =
        fmt.ok_or_else(|| ExportError::VerifyFailed("missing fmt chunk".into()))?;
    let data = data_bytes.ok_or_else(|| ExportError::VerifyFailed("missing data chunk".into()))?;
    if channels == 0 || bits_per_sample == 0 {
        return Err(ExportError::VerifyFailed("zero channels/bits".into()));
    }
    let bytes_per_frame = (channels as u64) * ((bits_per_sample as u64) / 8);
    if bytes_per_frame == 0 || data % bytes_per_frame != 0 {
        return Err(ExportError::VerifyFailed(
            "data chunk not a whole number of frames".into(),
        ));
    }
    Ok(WavInfo {
        audio_format,
        channels,
        sample_rate,
        bits_per_sample,
        data_frames: data / bytes_per_frame,
    })
}

fn expected_bits(depth: BitDepth) -> (u16, u16) {
    match depth {
        BitDepth::Pcm16 => (1, 16),
        BitDepth::Pcm24 => (1, 24),
        BitDepth::Float32 => (3, 32),
    }
}

/// Verify a staged WAV against the spec + explicit frame plan.
/// Exact match — the rounding was already decided upstream; a renderer
/// that produces a different length produced a different render.
pub fn verify_wav(path: &std::path::Path, spec: &ExportSpec, total_frames: u64) -> Result<WavInfo> {
    let info = probe_wav(path)?;
    let want_ch = match spec.channels {
        Some(ChannelLayout::Mono) => 1,
        Some(ChannelLayout::Stereo) => 2,
        None => 0,
    };
    if info.channels != want_ch {
        return Err(ExportError::VerifyFailed(format!(
            "channels {} != spec {}",
            info.channels, want_ch
        )));
    }
    if info.sample_rate != spec.sample_rate.unwrap_or(0) {
        return Err(ExportError::VerifyFailed(format!(
            "sample_rate {} != spec {}",
            info.sample_rate,
            spec.sample_rate.unwrap_or(0)
        )));
    }
    let (fmt, bits) = expected_bits(spec.bit_depth.unwrap_or(BitDepth::Pcm16));
    if info.audio_format != fmt || info.bits_per_sample != bits {
        return Err(ExportError::VerifyFailed(format!(
            "format {}/{}-bit != spec depth {:?}",
            info.audio_format,
            info.bits_per_sample,
            spec.bit_depth.unwrap_or(BitDepth::Pcm16)
        )));
    }
    if info.data_frames != total_frames {
        return Err(ExportError::VerifyFailed(format!(
            "frames {} != declared plan {}",
            info.data_frames, total_frames
        )));
    }
    Ok(info)
}

/// MIDI verification: an SMF starts with `MThd`. Anything else is not a
/// completed MIDI render.
pub fn verify_midi(path: &std::path::Path) -> Result<()> {
    let mut f = std::fs::File::open(path)?;
    let mut h = [0u8; 4];
    f.read_exact(&mut h)
        .map_err(|_| ExportError::VerifyFailed("file too small for MThd".into()))?;
    if &h != b"MThd" {
        return Err(ExportError::VerifyFailed("missing MThd magic".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixture writer: emits a structurally valid WAV of `frames` frames.
    /// Test asset — never presented as a real render.
    pub fn write_wav_fixture(
        path: &std::path::Path,
        channels: u16,
        sample_rate: u32,
        bits: u16,
        audio_format: u16,
        frames: u64,
    ) -> std::io::Result<()> {
        let bytes_per_frame = (channels as u64) * ((bits as u64) / 8);
        let data_len = frames * bytes_per_frame;
        let mut f = std::fs::File::create(path)?;
        use std::io::Write;
        f.write_all(b"RIFF")?;
        f.write_all(&((36 + data_len) as u32).to_le_bytes())?;
        f.write_all(b"WAVE")?;
        f.write_all(b"fmt ")?;
        f.write_all(&16u32.to_le_bytes())?;
        f.write_all(&audio_format.to_le_bytes())?;
        f.write_all(&channels.to_le_bytes())?;
        f.write_all(&sample_rate.to_le_bytes())?;
        let byte_rate = sample_rate * channels as u32 * (bits as u32 / 8);
        f.write_all(&byte_rate.to_le_bytes())?;
        let block_align = channels * (bits / 8);
        f.write_all(&block_align.to_le_bytes())?;
        f.write_all(&bits.to_le_bytes())?;
        f.write_all(b"data")?;
        f.write_all(&(data_len as u32).to_le_bytes())?;
        let zeros = [0u8; 8192];
        let mut left = data_len;
        while left > 0 {
            let n = (left as usize).min(zeros.len());
            f.write_all(&zeros[..n])?;
            left -= n as u64;
        }
        Ok(())
    }

    #[test]
    fn probe_and_verify_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.wav");
        write_wav_fixture(&p, 2, 48_000, 24, 1, 96_000).unwrap();
        let info = probe_wav(&p).unwrap();
        assert_eq!(info.channels, 2);
        assert_eq!(info.data_frames, 96_000);

        let mut spec = crate::spec::ExportSpec {
            job_id: "00000000-0000-4000-8000-000000000001".into(),
            project_id: "00000000-0000-4000-8000-000000000002".into(),
            checkpoint_id: "00000000-0000-4000-8000-000000000003".into(),
            source_revision: "1".into(),
            asset_hashes: vec![],
            range_start_ticks: "0".into(),
            range_end_ticks: "3840000".into(),
            format: crate::spec::ExportFormat::Wav,
            channels: Some(ChannelLayout::Stereo),
            bit_depth: Some(BitDepth::Pcm24),
            sample_rate: Some(48_000),
            tail: crate::spec::TailPolicy::None,
            tempo_map: vec![crate::spec::TempoSegment {
                at_ticks: "0".into(),
                bpm: 120.0,
            }],
            output_name: "x".into(),
            deadline_monotonic_ns: None,
        };
        assert!(verify_wav(&p, &spec, 96_000).is_ok());
        spec.sample_rate = Some(44_100);
        assert!(verify_wav(&p, &spec, 96_000).is_err());
        spec.sample_rate = Some(48_000);
        assert!(verify_wav(&p, &spec, 95_999).is_err());
    }

    #[test]
    fn rejects_truncated_and_wrong_magic() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("b.wav");
        std::fs::write(&p, b"RIFF").unwrap();
        assert!(probe_wav(&p).is_err());
        std::fs::write(&p, b"NOPE........WAVEfmt ").unwrap();
        assert!(probe_wav(&p).is_err());
    }

    #[test]
    fn midi_magic() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("m.mid");
        std::fs::write(&p, b"MThd\x00\x00\x00\x06").unwrap();
        assert!(verify_midi(&p).is_ok());
        std::fs::write(&p, b"BAD!").unwrap();
        assert!(verify_midi(&p).is_err());
    }
}
