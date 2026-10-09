//! Minimal PCM WAV reader for the mastering lane (T79). Reads RIFF/WAVE
//! `fmt ` + `data` chunks; accepts PCM integer (16/24/32-bit) and IEEE
//! float (32-bit), mono/stereo, arbitrary sample rate. Output is
//! de-interleaved f32 in [-1, 1] — the analysis layer's input domain.

use crate::error::{ProducerError, Result};

#[derive(Debug, Clone)]
pub struct PcmBuffer {
    pub sample_rate: u32,
    /// De-interleaved channel samples.
    pub channels: Vec<Vec<f32>>,
}

impl PcmBuffer {
    pub fn frames(&self) -> usize {
        self.channels.first().map(|c| c.len()).unwrap_or(0)
    }
    pub fn channel_count(&self) -> usize {
        self.channels.len()
    }
}

fn le_u16(b: &[u8], off: usize) -> Result<u16> {
    let s = b
        .get(off..off + 2)
        .ok_or_else(|| ProducerError::Pcm("truncated".into()))?;
    Ok(u16::from_le_bytes([s[0], s[1]]))
}
fn le_u32(b: &[u8], off: usize) -> Result<u32> {
    let s = b
        .get(off..off + 4)
        .ok_or_else(|| ProducerError::Pcm("truncated".into()))?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// Parse a RIFF/WAVE buffer. Rejects: non-RIFF, >2 channels, exotic
/// encodings, zero-length data — all as safe `Pcm` errors.
pub fn read_wav(bytes: &[u8]) -> Result<PcmBuffer> {
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(ProducerError::Pcm("not a RIFF/WAVE file".into()));
    }
    let mut off = 12usize;
    let mut fmt: Option<(u16, u16, u32, u16)> = None; // tag, channels, rate, bits
    let mut data: Option<&[u8]> = None;
    while off + 8 <= bytes.len() {
        let id = &bytes[off..off + 4];
        let size = le_u32(bytes, off + 4)? as usize;
        let body = off + 8;
        if body > bytes.len() {
            break;
        }
        let end = body.saturating_add(size).min(bytes.len());
        match id {
            b"fmt " => {
                let tag = le_u16(bytes, body)?;
                let ch = le_u16(bytes, body + 2)?;
                let rate = le_u32(bytes, body + 4)?;
                let bits = le_u16(bytes, body + 14)?;
                fmt = Some((tag, ch, rate, bits));
            }
            b"data" => data = Some(&bytes[body..end]),
            _ => {}
        }
        off = body + size + (size & 1); // chunks are word-aligned
    }
    let (tag, ch, rate, bits) =
        fmt.ok_or_else(|| ProducerError::Pcm("missing fmt chunk".into()))?;
    let data = data.ok_or_else(|| ProducerError::Pcm("missing data chunk".into()))?;
    if !(1..=2).contains(&ch) {
        return Err(ProducerError::Pcm(format!("{ch} channels unsupported")));
    }
    if rate == 0 {
        return Err(ProducerError::Pcm("zero sample rate".into()));
    }
    let bytes_per = (bits / 8) as usize;
    if bytes_per == 0 || data.len() < bytes_per * ch as usize {
        return Err(ProducerError::Pcm("empty/short data".into()));
    }
    let frames = data.len() / (bytes_per * ch as usize);
    let mut channels = vec![Vec::with_capacity(frames); ch as usize];
    let norm = |bits: u16| -> f64 {
        // Full-scale divisor: int formats are signed 2^(bits-1).
        (1u64 << (bits - 1)) as f64
    };
    for f in 0..frames {
        for c in 0..ch as usize {
            let o = (f * ch as usize + c) * bytes_per;
            let s = match (tag, bits) {
                (1, 16) => i16::from_le_bytes([data[o], data[o + 1]]) as f64 / norm(16),
                (1, 24) => {
                    let v = (data[o] as i32)
                        | ((data[o + 1] as i32) << 8)
                        | ((data[o + 2] as i32) << 16);
                    (v << 8 >> 8) as f64 / norm(24)
                }
                (1, 32) => {
                    i32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]) as f64
                        / norm(32)
                }
                (3, 32) => {
                    f32::from_le_bytes([data[o], data[o + 1], data[o + 2], data[o + 3]]) as f64
                }
                _ => {
                    return Err(ProducerError::Pcm(format!(
                        "encoding tag={tag} bits={bits} unsupported"
                    )))
                }
            };
            channels[c].push(s.clamp(-1.0, 1.0) as f32);
        }
    }
    Ok(PcmBuffer {
        sample_rate: rate,
        channels,
    })
}

/// Serialize `PcmBuffer` back to a 32-bit float WAV (test fixtures and
/// audition-bounce spec inputs use this).
pub fn write_wav_f32(buf: &PcmBuffer) -> Result<Vec<u8>> {
    let ch = buf.channels.len() as u16;
    if ch == 0 || buf.frames() == 0 {
        return Err(ProducerError::Pcm("empty buffer".into()));
    }
    let frames = buf.frames();
    let block = ch as usize * 4;
    let data_len = frames * block;
    let mut out = Vec::with_capacity(44 + data_len);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36u32 + data_len as u32).to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes()); // IEEE float
    out.extend_from_slice(&ch.to_le_bytes());
    out.extend_from_slice(&buf.sample_rate.to_le_bytes());
    out.extend_from_slice(&(buf.sample_rate * block as u32).to_le_bytes());
    out.extend_from_slice(&(block as u16).to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_len as u32).to_le_bytes());
    for f in 0..frames {
        for c in 0..ch as usize {
            out.extend_from_slice(&buf.channels[c][f].to_le_bytes());
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_f32() {
        let buf = PcmBuffer {
            sample_rate: 48_000,
            channels: vec![
                vec![0.0, 0.5, -0.5, 1.0, -1.0],
                vec![-0.25, 0.25, 0.0, 0.75, -0.75],
            ],
        };
        let bytes = write_wav_f32(&buf).unwrap();
        let back = read_wav(&bytes).unwrap();
        assert_eq!(back.sample_rate, 48_000);
        assert_eq!(back.channels.len(), 2);
        assert_eq!(back.channels[0], buf.channels[0]);
        assert_eq!(back.channels[1], buf.channels[1]);
    }

    #[test]
    fn int16_decode() {
        // Hand-built mono int16: frames -32768, 0, 32767.
        let mut bytes = Vec::new();
        bytes.extend_from_slice(b"RIFF");
        bytes.extend_from_slice(&(36u32 + 6).to_le_bytes());
        bytes.extend_from_slice(b"WAVEfmt ");
        bytes.extend_from_slice(&16u32.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&44_100u32.to_le_bytes());
        bytes.extend_from_slice(&88_200u32.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&16u16.to_le_bytes());
        bytes.extend_from_slice(b"data");
        bytes.extend_from_slice(&6u32.to_le_bytes());
        for v in [-32768i16, 0, 32767] {
            bytes.extend_from_slice(&v.to_le_bytes());
        }
        let b = read_wav(&bytes).unwrap();
        assert_eq!(b.channels[0][0], -1.0);
        assert_eq!(b.channels[0][1], 0.0);
        assert!((b.channels[0][2] - 0.999969).abs() < 1e-5);
    }

    #[test]
    fn rejects_garbage() {
        assert!(read_wav(b"not a wave").is_err());
        assert!(read_wav(&[]).is_err());
    }
}
