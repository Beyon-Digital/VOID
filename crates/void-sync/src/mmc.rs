//! MIDI Machine Control command model (PRO-03, T94 model side).
//!
//! MMC lives in universal non-real-time SysEx:
//! `F0 7F <deviceId> 06 <command> <data...> F7`.
//!
//! This is a command *model*: real byte encoding, bounds-checked
//! decoding, typed errors for unsupported/malformed frames. Nothing
//! here claims the wire is attached — endpoints are descriptors.

use serde::{Deserialize, Serialize};

use crate::error::SyncError;
use crate::mtc::{MtcFrameRate, MtcTime};

/// MMC command identity byte (sub-id2 = 06 for "command" frames,
/// 07 for "response" frames — VOID emits commands only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MmcFrameKind {
    Command, // 0x06
    Response, // 0x07
}

/// The MMC command vocabulary VOID models. Codes are the real spec
/// numbers; commands VOID does not send decode as
/// [`SyncError::Mmc`] — never silently dropped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum MmcCommand {
    Stop,                 // 0x01
    Play,                 // 0x02
    DeferredPlay,         // 0x03
    FastForward,          // 0x04
    Rewind,               // 0x05
    RecordStrobe,         // 0x06
    RecordExit,           // 0x07
    RecordPause,          // 0x08
    Pause,                // 0x09
    Eject,                // 0x0A
    Chase,                // 0x0B
    CommandErrorReset,    // 0x0C
    MmcReset,             // 0x0D
    /// LOCATE [I/F] — command 0x44 with sub-command 0x01 (TARGET)
    /// carrying a 5-byte SMPTE time (hr mn sc fr ff).
    Locate { time: MtcTime, subframe: u8 },
    /// SHUTTLE — 0x47, direction/speed in a 3-byte sh st sl field.
    Shuttle { forward: bool, speed_permille: u16 },
}

fn command_code(c: &MmcCommand) -> u8 {
    match c {
        MmcCommand::Stop => 0x01,
        MmcCommand::Play => 0x02,
        MmcCommand::DeferredPlay => 0x03,
        MmcCommand::FastForward => 0x04,
        MmcCommand::Rewind => 0x05,
        MmcCommand::RecordStrobe => 0x06,
        MmcCommand::RecordExit => 0x07,
        MmcCommand::RecordPause => 0x08,
        MmcCommand::Pause => 0x09,
        MmcCommand::Eject => 0x0A,
        MmcCommand::Chase => 0x0B,
        MmcCommand::CommandErrorReset => 0x0C,
        MmcCommand::MmcReset => 0x0D,
        MmcCommand::Locate { .. } => 0x44,
        MmcCommand::Shuttle { .. } => 0x47,
    }
}

/// MMC 5-byte standard time field: `hr mn sc fr ff` where `hr` packs
/// the rate code in bits 6–5 exactly like MTC.
fn encode_mmc_time(time: &MtcTime, subframe: u8) -> [u8; 5] {
    [
        (time.rate.code() << 5) | (time.hours & 0x1F),
        time.minutes & 0x7F,
        time.seconds & 0x7F,
        time.frames & 0x7F,
        subframe & 0x7F,
    ]
}

fn decode_mmc_time(b: &[u8]) -> Result<(MtcTime, u8), SyncError> {
    if b.len() != 5 {
        return Err(SyncError::Mmc(format!(
            "MMC time field must be 5 bytes, got {}",
            b.len()
        )));
    }
    for (i, &x) in b.iter().enumerate() {
        if x & 0x80 != 0 {
            return Err(SyncError::Mmc(format!(
                "MMC time byte {i} has high bit set ({x:02X})"
            )));
        }
    }
    let rate = MtcFrameRate::from_code((b[0] >> 5) & 0x03)?;
    let t = MtcTime::new(b[0] & 0x1F, b[1], b[2], b[3], rate)?;
    Ok((t, b[4]))
}

/// Maximum wire size for a supported MMC command (Locate's 5-byte
/// time + headers).
pub const MAX_MMC_MESSAGE_LEN: usize = 16;

/// Encode `command` for `device_id` (0x00–0x7F; 0x7F = all devices).
pub fn encode(device_id: u8, command: &MmcCommand) -> Result<Vec<u8>, SyncError> {
    if device_id > 0x7F {
        return Err(SyncError::Mmc(format!(
            "device id {device_id} > 0x7F"
        )));
    }
    let mut out = vec![0xF0, 0x7F, device_id, 0x06];
    match command {
        MmcCommand::Locate { time, subframe } => {
            out.push(0x44);
            // data count byte + TARGET sub-command (0x01) + 5-byte time
            out.push(0x06);
            out.push(0x01);
            out.extend_from_slice(&encode_mmc_time(time, *subframe));
        }
        MmcCommand::Shuttle {
            forward,
            speed_permille,
        } => {
            if *speed_permille > 0x3FFF {
                return Err(SyncError::Mmc(format!(
                    "shuttle speed {speed_permille}‰ out of range (max 16383‰)"
                )));
            }
            out.push(0x47);
            out.push(0x03); // data count
            let sp = *speed_permille;
            // VOID integral packing: 14-bit permille value split
            // sh/st/sl as (dir:bit6|sp13..8) (sp7..2) (sp1..0). This is
            // honest integral packing — VOID does not claim the
            // fractional-standard shuttle variant.
            out.push(if *forward { 0x40 } else { 0x00 } | ((sp >> 8) & 0x3F) as u8);
            out.push(((sp >> 2) & 0x3F) as u8);
            out.push((sp & 0x03) as u8);
        }
        c => out.push(command_code(c)),
    }
    out.push(0xF7);
    Ok(out)
}

/// Decode one complete MMC frame (`F0 .. F7` slice). Returns the
/// addressed device id plus the command. Unknown command codes are a
/// typed error — never silently ignored.
pub fn decode(bytes: &[u8]) -> Result<(u8, MmcCommand), SyncError> {
    if bytes.len() < 5 || bytes.len() > MAX_MMC_MESSAGE_LEN {
        return Err(SyncError::Mmc(format!(
            "frame length {} out of 5..={MAX_MMC_MESSAGE_LEN}",
            bytes.len()
        )));
    }
    if bytes[0] != 0xF0 || *bytes.last().unwrap() != 0xF7 {
        return Err(SyncError::Mmc("missing F0/F7 SysEx frame".into()));
    }
    if bytes[1] != 0x7F {
        return Err(SyncError::Mmc(format!(
            "expected system channel 7F, got {:02X}",
            bytes[1]
        )));
    }
    let device_id = bytes[2];
    if device_id > 0x7F {
        return Err(SyncError::Mmc(format!("device id {device_id} > 0x7F")));
    }
    if bytes[3] != 0x06 {
        return Err(SyncError::Mmc(format!(
            "expected command frame 06, got {:02X}",
            bytes[3]
        )));
    }
    let body = &bytes[4..bytes.len() - 1];
    let cmd = match body[0] {
        0x01 => MmcCommand::Stop,
        0x02 => MmcCommand::Play,
        0x03 => MmcCommand::DeferredPlay,
        0x04 => MmcCommand::FastForward,
        0x05 => MmcCommand::Rewind,
        0x06 => MmcCommand::RecordStrobe,
        0x07 => MmcCommand::RecordExit,
        0x08 => MmcCommand::RecordPause,
        0x09 => MmcCommand::Pause,
        0x0A => MmcCommand::Eject,
        0x0B => MmcCommand::Chase,
        0x0C => MmcCommand::CommandErrorReset,
        0x0D => MmcCommand::MmcReset,
        0x44 => {
            // LOCATE [I/F]: 44 <count=06> <sub=01 TARGET> hr mn sc fr ff
            if body.len() != 8 || body[1] != 0x06 || body[2] != 0x01 {
                return Err(SyncError::Mmc(
                    "LOCATE requires 44 06 01 + 5-byte time".into(),
                ));
            }
            let (time, subframe) = decode_mmc_time(&body[3..8])?;
            MmcCommand::Locate { time, subframe }
        }
        0x47 => {
            // SHUTTLE: 47 03 sh st sl (VOID integral packing, see encode).
            if body.len() != 5 || body[1] != 0x03 {
                return Err(SyncError::Mmc("SHUTTLE requires 47 03 sh st sl".into()));
            }
            let sh = body[2];
            if sh & 0x80 != 0 || body[3] & 0x80 != 0 || body[4] & 0x80 != 0 {
                return Err(SyncError::Mmc("SHUTTLE data byte high bit set".into()));
            }
            let forward = sh & 0x40 != 0;
            let speed =
                (((sh & 0x3F) as u16) << 8) | (((body[3] & 0x3F) as u16) << 2) | (body[4] & 0x03) as u16;
            MmcCommand::Shuttle {
                forward,
                speed_permille: speed,
            }
        }
        other => {
            return Err(SyncError::Mmc(format!(
                "unsupported MMC command {other:02X} — not silently dropped"
            )));
        }
    };
    Ok((device_id, cmd))
}
