//! Length-prefixed wire framing: `u32 LE len | flatbuffer payload`.
//! Enforces the 1 MiB frame cap before payload access (CONTRACTS.md §2).

use crate::limits::CONTROL_FRAME_MAX;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum FrameError {
    #[error("frame length {0} exceeds limit {CONTROL_FRAME_MAX}")]
    Oversized(u32),
    #[error("truncated frame: declared {declared} bytes, only {available} available")]
    Truncated { declared: u32, available: usize },
    #[error("incomplete length prefix")]
    IncompleteHeader,
    #[error("flatbuffers verification failed")]
    InvalidBuffer,
    #[error("unexpected frame variant")]
    UnexpectedVariant,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Build a framed message from a finished FlatBuffer.
pub fn encode_frame(payload: &[u8]) -> Result<Vec<u8>, FrameError> {
    let len = u32::try_from(payload.len()).map_err(|_| FrameError::Oversized(u32::MAX))?;
    if len > CONTROL_FRAME_MAX {
        return Err(FrameError::Oversized(len));
    }
    let mut out = Vec::with_capacity(4 + payload.len());
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(payload);
    Ok(out)
}

/// Parse one frame from the head of `buf`. Returns payload size + total
/// consumed bytes (header + payload), or None if more bytes are needed.
pub fn try_decode_frame(buf: &[u8]) -> Result<Option<(usize, usize)>, FrameError> {
    if buf.len() < 4 {
        return Ok(None);
    }
    let declared = u32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]);
    if declared > CONTROL_FRAME_MAX {
        return Err(FrameError::Oversized(declared));
    }
    let need = 4 + declared as usize;
    if buf.len() < need {
        return Ok(None);
    }
    Ok(Some((declared as usize, need)))
}

/// Streaming frame reader: accumulate bytes, yield complete frames.
#[derive(Default)]
pub struct FrameReader {
    buf: Vec<u8>,
}

impl FrameReader {
    pub fn push(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// Pop the next complete frame payload, if any.
    pub fn next_frame(&mut self) -> Result<Option<Vec<u8>>, FrameError> {
        match try_decode_frame(&self.buf)? {
            Some((payload_len, consumed)) => {
                let payload = self.buf[4..4 + payload_len].to_vec();
                self.buf.drain(..consumed);
                Ok(Some(payload))
            }
            None => Ok(None),
        }
    }

    pub fn buffered_len(&self) -> usize {
        self.buf.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_single_frame() {
        let payload = b"hello-void";
        let framed = encode_frame(payload).unwrap();
        let mut r = FrameReader::default();
        r.push(&framed);
        assert_eq!(r.next_frame().unwrap(), Some(payload.to_vec()));
        assert!(r.next_frame().unwrap().is_none());
    }

    #[test]
    fn split_delivery() {
        let framed = encode_frame(&[7u8; 100]).unwrap();
        let mut r = FrameReader::default();
        for chunk in framed.chunks(17) {
            r.push(chunk);
        }
        assert_eq!(r.next_frame().unwrap().unwrap().len(), 100);
    }

    #[test]
    fn oversized_rejected() {
        // declared length > 1 MiB
        let mut bad = (CONTROL_FRAME_MAX + 1).to_le_bytes().to_vec();
        bad.extend_from_slice(&[0u8; 8]);
        let mut r = FrameReader::default();
        r.push(&bad);
        assert!(matches!(r.next_frame(), Err(FrameError::Oversized(_))));
    }

    #[test]
    fn multiple_frames_in_order() {
        let mut r = FrameReader::default();
        r.push(&encode_frame(b"a").unwrap());
        r.push(&encode_frame(b"b").unwrap());
        r.push(&encode_frame(b"c").unwrap());
        assert_eq!(r.next_frame().unwrap(), Some(b"a".to_vec()));
        assert_eq!(r.next_frame().unwrap(), Some(b"b".to_vec()));
        assert_eq!(r.next_frame().unwrap(), Some(b"c".to_vec()));
    }
}
