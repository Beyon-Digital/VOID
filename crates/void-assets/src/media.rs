//! Media integrity probing. Decode/import is gated on this validation —
//! corrupt or truncated media is rejected with a reason, never substituted
//! (CONTRACTS.md §4, T19).

use crate::error::{AssetError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WavInfo {
    pub channels: u16,
    pub sample_rate: u32,
    pub bits_per_sample: u16,
    pub data_bytes: u64,
    /// Whole samples per channel.
    pub frames: u64,
}

/// Parse a RIFF/WAVE header and validate declared metadata against the
/// actual byte length. Rejects truncated streams, impossible channel
/// counts, unsupported encodings and missing chunks.
pub fn probe_wav(data: &[u8]) -> Result<WavInfo> {
    let corrupt = |m: &str| AssetError::CorruptMedia(m.to_string());
    if data.len() < 12 {
        return Err(corrupt("shorter than a RIFF header"));
    }
    if &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err(corrupt("missing RIFF/WAVE signature"));
    }
    let riff_size = u32::from_le_bytes(data[4..8].try_into().unwrap()) as u64;
    // A RIFF file may legally be shorter than declared when streamed, but a
    // stored blob claiming more bytes than it holds is truncated media.
    if riff_size + 8 > data.len() as u64 {
        return Err(corrupt("declared RIFF size exceeds file length"));
    }

    let mut fmt: Option<(u16, u16, u32, u16)> = None; // tag, channels, rate, bits
    let mut data_bytes: Option<u64> = None;
    let mut pos = 12usize;
    while pos + 8 <= data.len() {
        let id = &data[pos..pos + 4];
        let size = u32::from_le_bytes(data[pos + 4..pos + 8].try_into().unwrap()) as usize;
        let body = pos + 8;
        if body + size > data.len() {
            return Err(corrupt("chunk extends beyond end of file"));
        }
        match id {
            b"fmt " => {
                if size < 16 {
                    return Err(corrupt("fmt chunk too small"));
                }
                let tag = u16::from_le_bytes(data[body..body + 2].try_into().unwrap());
                let channels = u16::from_le_bytes(data[body + 2..body + 4].try_into().unwrap());
                let rate = u32::from_le_bytes(data[body + 4..body + 8].try_into().unwrap());
                let bits = u16::from_le_bytes(data[body + 14..body + 16].try_into().unwrap());
                fmt = Some((tag, channels, rate, bits));
            }
            b"data" => {
                data_bytes = Some(size as u64);
            }
            _ => {}
        }
        // Chunks are word-aligned.
        pos = body + size + (size & 1);
    }

    let (tag, channels, rate, bits) = fmt.ok_or_else(|| corrupt("missing fmt chunk"))?;
    let data_bytes = data_bytes.ok_or_else(|| corrupt("missing data chunk"))?;
    if tag != 1 && tag != 3 {
        return Err(corrupt(&format!(
            "unsupported WAV format tag {tag} (only PCM=1 / float=3)"
        )));
    }
    if channels == 0 || channels > 64 {
        return Err(corrupt(&format!("invalid channel count {channels}")));
    }
    if !(8_000..=768_000).contains(&rate) {
        return Err(corrupt(&format!("invalid sample rate {rate}")));
    }
    if !matches!(bits, 8 | 16 | 24 | 32 | 64) {
        return Err(corrupt(&format!("invalid bit depth {bits}")));
    }
    let frame_bytes = (channels as u64) * (bits as u64 / 8);
    let frames = data_bytes.checked_div(frame_bytes).unwrap_or(0);
    Ok(WavInfo {
        channels,
        sample_rate: rate,
        bits_per_sample: bits,
        data_bytes,
        frames,
    })
}
