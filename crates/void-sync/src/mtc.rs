//! MIDI Time Code encode/decode (PRO-03, T94 model side).
//!
//! Two real message forms, bit-exact:
//!
//! - **Full message** — universal real-time SysEx
//!   `F0 7F 7F 01 01 hr mn sc fr F7`, where `hr = 0rrhhhhh`
//!   (rr = frame-rate code in bits 6–5, hours in bits 4–0).
//! - **Quarter-frame** — `F1 nn`, `nn = 0nnndddd`: bits 6–4 select
//!   which nibble of the time is carried (0 = frames units …
//!   7 = hours-high + rate), sent as an 8-message cycle while running.
//!
//! Decode validates structure (bounds, reserved bits, terminator) and
//! rejects malformed bytes with typed [`SyncError`]s — never coerces
//! garbage into a time.

use serde::{Deserialize, Serialize};

use crate::error::SyncError;

/// MTC frame-rate codes (bits 5–6 of the packed hour nibble).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MtcFrameRate {
    F24,
    F25,
    /// 29.97 drop-frame (SMPTE drop).
    F30Drop,
    F30,
}

impl MtcFrameRate {
    /// Rate code per the MTC spec (0 = 24, 1 = 25, 2 = 29.97 DF, 3 = 30).
    pub fn code(self) -> u8 {
        match self {
            MtcFrameRate::F24 => 0,
            MtcFrameRate::F25 => 1,
            MtcFrameRate::F30Drop => 2,
            MtcFrameRate::F30 => 3,
        }
    }

    pub fn from_code(code: u8) -> Result<Self, SyncError> {
        match code {
            0 => Ok(MtcFrameRate::F24),
            1 => Ok(MtcFrameRate::F25),
            2 => Ok(MtcFrameRate::F30Drop),
            3 => Ok(MtcFrameRate::F30),
            _ => Err(SyncError::Mtc(format!("invalid rate code {code}"))),
        }
    }

    /// Frames per second for wrap-around checks (29.97 DF counted as 30).
    pub fn max_frame(self) -> u8 {
        match self {
            MtcFrameRate::F24 => 24,
            MtcFrameRate::F25 => 25,
            MtcFrameRate::F30Drop | MtcFrameRate::F30 => 30,
        }
    }
}

/// A timecode instant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MtcTime {
    /// 0..=23 (SMPTE day wrap).
    pub hours: u8,
    /// 0..=59.
    pub minutes: u8,
    /// 0..=59.
    pub seconds: u8,
    /// 0..<rate.max_frame().
    pub frames: u8,
    pub rate: MtcFrameRate,
}

impl MtcTime {
    pub fn new(
        hours: u8,
        minutes: u8,
        seconds: u8,
        frames: u8,
        rate: MtcFrameRate,
    ) -> Result<Self, SyncError> {
        let t = MtcTime {
            hours,
            minutes,
            seconds,
            frames,
            rate,
        };
        t.validate()?;
        Ok(t)
    }

    pub fn validate(&self) -> Result<(), SyncError> {
        if self.hours > 23 {
            return Err(SyncError::Mtc(format!("hours {} > 23", self.hours)));
        }
        if self.minutes > 59 {
            return Err(SyncError::Mtc(format!("minutes {} > 59", self.minutes)));
        }
        if self.seconds > 59 {
            return Err(SyncError::Mtc(format!("seconds {} > 59", self.seconds)));
        }
        if self.frames >= self.rate.max_frame() {
            return Err(SyncError::Mtc(format!(
                "frames {} >= rate max {}",
                self.frames,
                self.rate.max_frame()
            )));
        }
        Ok(())
    }

    /// Frames since midnight — monotonic comparison key within a day.
    pub fn frame_index(&self) -> u64 {
        let fps = self.rate.max_frame() as u64;
        ((((self.hours as u64) * 60 + self.minutes as u64) * 60 + self.seconds as u64) * fps)
            + self.frames as u64
    }
}

/// Byte length of the full SysEx message.
pub const FULL_MESSAGE_LEN: usize = 10;
/// Byte length of one quarter-frame message on the wire (`F1 nn`).
pub const QUARTER_FRAME_MESSAGE_LEN: usize = 2;
/// Quarter-frame messages per complete time.
pub const QUARTER_FRAME_COUNT: usize = 8;

/// Encode the full message:
/// `F0 7F 7F 01 01 hr mn sc fr F7` with `hr = (rate<<5)|hours`.
pub fn encode_full_message(time: &MtcTime) -> [u8; FULL_MESSAGE_LEN] {
    [
        0xF0,
        0x7F,
        0x7F,
        0x01,
        0x01,
        (time.rate.code() << 5) | (time.hours & 0x1F),
        time.minutes & 0x7F,
        time.seconds & 0x7F,
        time.frames & 0x7F,
        0xF7,
    ]
}

/// Decode a full message. `bytes` must be exactly the 10-byte frame
/// (a caller demuxing a byte stream slices the F0..F7 span first).
pub fn decode_full_message(bytes: &[u8]) -> Result<MtcTime, SyncError> {
    if bytes.len() != FULL_MESSAGE_LEN {
        return Err(SyncError::Mtc(format!(
            "full message must be {FULL_MESSAGE_LEN} bytes, got {}",
            bytes.len()
        )));
    }
    if bytes[0] != 0xF0 || bytes[FULL_MESSAGE_LEN - 1] != 0xF7 {
        return Err(SyncError::Mtc("missing F0/F7 SysEx frame".into()));
    }
    if bytes[1] != 0x7F || bytes[2] != 0x7F {
        return Err(SyncError::Mtc(format!(
            "expected device 7F/sub-id1 7F, got {:02X} {:02X}",
            bytes[1], bytes[2]
        )));
    }
    if bytes[3] != 0x01 || bytes[4] != 0x01 {
        return Err(SyncError::Mtc(format!(
            "expected MTC full-message id 01 01, got {:02X} {:02X}",
            bytes[3], bytes[4]
        )));
    }
    // Data bytes must have the high bit clear.
    for &b in &bytes[5..9] {
        if b & 0x80 != 0 {
            return Err(SyncError::Mtc(format!(
                "data byte {b:02X} has high bit set"
            )));
        }
    }
    let rate = MtcFrameRate::from_code((bytes[5] >> 5) & 0x03)?;
    MtcTime::new(bytes[5] & 0x1F, bytes[6], bytes[7], bytes[8], rate)
}

/// The 8 quarter-frame data bytes for `time`, in wire order (the
/// caller wraps each in `F1 nn`).
///
/// Piece index → nibble content:
/// 0 = frames bits 0–3, 1 = frames bit 4, 2 = seconds bits 0–3,
/// 3 = seconds bits 4–5, 4 = minutes bits 0–3, 5 = minutes bits 4–5,
/// 6 = hours bits 0–3, 7 = hours bit 4 | rate in bits 2–1.
pub fn encode_quarter_frame_pieces(time: &MtcTime) -> [u8; QUARTER_FRAME_COUNT] {
    [
        time.frames & 0x0F,
        (time.frames >> 4) & 0x01,
        time.seconds & 0x0F,
        (time.seconds >> 4) & 0x03,
        time.minutes & 0x0F,
        (time.minutes >> 4) & 0x03,
        time.hours & 0x0F,
        (time.rate.code() << 1) | ((time.hours >> 4) & 0x01),
    ]
}

/// Wrap piece index + nibble into the `F1 nn` wire pair.
pub fn quarter_frame_message(piece: usize, nibble: u8) -> [u8; QUARTER_FRAME_MESSAGE_LEN] {
    debug_assert!(piece < QUARTER_FRAME_COUNT && nibble <= 0x0F);
    [0xF1, ((piece as u8) << 4) | nibble]
}

/// Full 16-byte quarter-frame cycle for `time` (F1 nn × 8).
pub fn encode_quarter_frames(time: &MtcTime) -> [u8; QUARTER_FRAME_COUNT * QUARTER_FRAME_MESSAGE_LEN] {
    let pieces = encode_quarter_frame_pieces(time);
    let mut out = [0u8; QUARTER_FRAME_COUNT * QUARTER_FRAME_MESSAGE_LEN];
    for (i, &nibble) in pieces.iter().enumerate() {
        let [hi, lo] = quarter_frame_message(i, nibble);
        out[i * 2] = hi;
        out[i * 2 + 1] = lo;
    }
    out
}

/// Collects quarter-frame nibbles into a time. Feed each `F1 nn`
/// message's data byte (`nn`); when all 8 pieces are present the last
/// `feed` returns `Some(MtcTime)`. Pieces arriving out of order still
/// resolve — quarter-frame cycles start at whichever piece index the
/// sender is on.
#[derive(Debug, Default)]
pub struct QuarterFrameAssembler {
    pieces: [Option<u8>; QUARTER_FRAME_COUNT],
}

impl QuarterFrameAssembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Reset (e.g. on sender start/stop or a gap).
    pub fn clear(&mut self) {
        self.pieces = [None; QUARTER_FRAME_COUNT];
    }

    /// Number of pieces currently held.
    pub fn filled(&self) -> usize {
        self.pieces.iter().filter(|p| p.is_some()).count()
    }

    /// Feed one complete `F1 nn` message (`bytes[0] == 0xF1`).
    pub fn feed_message(&mut self, bytes: &[u8]) -> Result<Option<MtcTime>, SyncError> {
        if bytes.len() != QUARTER_FRAME_MESSAGE_LEN || bytes[0] != 0xF1 {
            return Err(SyncError::Mtc(
                "quarter-frame message must be F1 nn".into(),
            ));
        }
        self.feed_data_byte(bytes[1])
    }

    /// Feed just the data byte `nn` of an `F1 nn` message.
    pub fn feed_data_byte(&mut self, nn: u8) -> Result<Option<MtcTime>, SyncError> {
        if nn & 0x80 != 0 {
            return Err(SyncError::Mtc(format!(
                "data byte {nn:02X} has high bit set"
            )));
        }
        let piece = (nn >> 4) as usize;
        self.pieces[piece] = Some(nn & 0x0F);
        self.try_assemble()
    }

    fn try_assemble(&self) -> Result<Option<MtcTime>, SyncError> {
        if self.pieces.iter().any(|p| p.is_none()) {
            return Ok(None);
        }
        let p = |i: usize| self.pieces[i].unwrap();
        let frames = p(0) | (p(1) << 4);
        let seconds = p(2) | (p(3) << 4);
        let minutes = p(4) | (p(5) << 4);
        let hours = p(6) | ((p(7) & 0x01) << 4);
        let rate = MtcFrameRate::from_code((p(7) >> 1) & 0x03)?;
        Ok(Some(MtcTime::new(hours, minutes, seconds, frames, rate)?))
    }
}

/// A demuxer for a raw byte stream: separates full messages, quarter
/// frames and transport bytes (0xF8/0xFA/0xFB/0xFC) into typed events.
/// Anything else is surfaced as `other` so callers can't accidentally
/// swallow bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MtcWireEvent {
    Full(MtcTime),
    QuarterFrame { piece: usize, nibble: u8 },
    ClockStart,
    ClockContinue,
    ClockStop,
    ClockTick,
}

/// Split a wire byte stream into MTC/clock events. Returns the parsed
/// events plus any leftover bytes (an incomplete trailing frame).
pub fn demux(bytes: &[u8]) -> (Vec<MtcWireEvent>, Vec<u8>) {
    let mut events = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            0xF0 => {
                if i + FULL_MESSAGE_LEN <= bytes.len()
                    && bytes[i + FULL_MESSAGE_LEN - 1] == 0xF7
                {
                    if let Ok(t) = decode_full_message(&bytes[i..i + FULL_MESSAGE_LEN]) {
                        events.push(MtcWireEvent::Full(t));
                    }
                    i += FULL_MESSAGE_LEN;
                } else {
                    // Incomplete/non-MTC SysEx — preserve for caller.
                    break;
                }
            }
            0xF1 => {
                if i + 2 <= bytes.len() {
                    events.push(MtcWireEvent::QuarterFrame {
                        piece: (bytes[i + 1] >> 4) as usize,
                        nibble: bytes[i + 1] & 0x0F,
                    });
                    i += 2;
                } else {
                    break;
                }
            }
            0xFA => {
                events.push(MtcWireEvent::ClockStart);
                i += 1;
            }
            0xFB => {
                events.push(MtcWireEvent::ClockContinue);
                i += 1;
            }
            0xFC => {
                events.push(MtcWireEvent::ClockStop);
                i += 1;
            }
            0xF8 => {
                events.push(MtcWireEvent::ClockTick);
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }
    (events, bytes[i..].to_vec())
}
