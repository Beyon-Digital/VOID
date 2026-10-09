//! DMX universe/channel descriptors (T95/T96 side).
//!
//! Models the DMX512-A rules at the descriptor layer: a universe is
//! exactly 512 channels, addresses are 1..=512, values 0..=255, and a
//! fixture's channel footprint may not overlap another's — overlaps
//! are a typed conflict, not merged state. VOID sends nothing until a
//! real output path exists (see NEEDS); this is the *model* the gate
//! and show control validate against.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::error::SyncError;

/// DMX512-A fixed universe size.
pub const DMX_UNIVERSE_CHANNELS: usize = 512;
/// Max strobe rate VOID will generate — seizure-safety bound
/// (~10 Hz conservative; the visible-risk band is ~3–30 Hz).
pub const MAX_STROBE_HZ: f32 = 10.0;

/// A DMX universe: exactly 512 channels of 0..=255. The length
/// invariant is enforced by construction and re-checked on access —
/// a deserialized universe with the wrong channel count errors rather
/// than silently index-failing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DmxUniverse {
    /// Universe index (0-based within the rig).
    pub index: u16,
    /// 1..=512 → 0..=255.
    channels: Vec<u8>,
}

fn check_len(u: &DmxUniverse) -> Result<(), SyncError> {
    if u.channels.len() != DMX_UNIVERSE_CHANNELS {
        return Err(SyncError::Dmx(format!(
            "universe {} carries {} channels != {DMX_UNIVERSE_CHANNELS}",
            u.index,
            u.channels.len()
        )));
    }
    Ok(())
}

impl DmxUniverse {
    pub fn new(index: u16) -> Self {
        DmxUniverse {
            index,
            channels: vec![0; DMX_UNIVERSE_CHANNELS],
        }
    }

    /// Raw channel view (len is the invariant-checked 512).
    pub fn channels(&self) -> &[u8] {
        &self.channels
    }

    /// Integrity check — a universe that isn't exactly 512 channels
    /// is corrupt data, surfaced as an error not a panic.
    pub fn validate(&self) -> Result<(), SyncError> {
        check_len(self)
    }

    pub fn channel(&self, address: u16) -> Result<u8, SyncError> {
        check_len(self)?;
        if !(1..=DMX_UNIVERSE_CHANNELS as u16).contains(&address) {
            return Err(SyncError::Dmx(format!(
                "address {address} out of 1..={DMX_UNIVERSE_CHANNELS}"
            )));
        }
        Ok(self.channels[(address - 1) as usize])
    }

    pub fn set_channel(&mut self, address: u16, value: u8) -> Result<(), SyncError> {
        check_len(self)?;
        if !(1..=DMX_UNIVERSE_CHANNELS as u16).contains(&address) {
            return Err(SyncError::Dmx(format!(
                "address {address} out of 1..={DMX_UNIVERSE_CHANNELS}"
            )));
        }
        self.channels[(address - 1) as usize] = value;
        Ok(())
    }

    /// Blackout: every channel to zero — the safe disconnected state.
    /// PANIC and disconnect both end here.
    pub fn blackout(&mut self) {
        self.channels = vec![0; DMX_UNIVERSE_CHANNELS];
    }
}

/// One declared channel's role inside a fixture footprint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DmxChannelDescriptor {
    /// 1-based channel offset inside the footprint.
    pub offset: u16,
    /// e.g. "dimmer", "pan", "tilt", "strobe".
    pub function: String,
    /// For strobe channels: ceiling — VOID never emits above this.
    /// `None` = not a strobe channel.
    pub strobe_max_hz: Option<f32>,
}

/// A fixture occupying `[start_address, start_address + footprint)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DmxFixture {
    pub fixture_id: String,
    pub universe_index: u16,
    /// First DMX address (1..=512).
    pub start_address: u16,
    pub channels: Vec<DmxChannelDescriptor>,
}

impl DmxFixture {
    /// End address (inclusive), for overlap checks.
    pub fn end_address(&self) -> u16 {
        self.start_address + self.channels.len().max(1) as u16 - 1
    }

    pub fn validate(&self) -> Result<(), SyncError> {
        if self.channels.is_empty() {
            return Err(SyncError::Dmx(format!(
                "fixture {:?} has empty footprint",
                self.fixture_id
            )));
        }
        if self.start_address < 1
            || self.end_address() > DMX_UNIVERSE_CHANNELS as u16
        {
            return Err(SyncError::Dmx(format!(
                "fixture {:?} footprint {}..{} exceeds universe",
                self.fixture_id,
                self.start_address,
                self.end_address()
            )));
        }
        for (i, ch) in self.channels.iter().enumerate() {
            if ch.function.trim().is_empty() {
                return Err(SyncError::Dmx(format!(
                    "fixture {:?} channel {i} has empty function",
                    self.fixture_id
                )));
            }
            if let Some(hz) = ch.strobe_max_hz {
                if !(hz.is_finite() && hz > 0.0 && hz <= MAX_STROBE_HZ) {
                    return Err(SyncError::Dmx(format!(
                        "fixture {:?} channel {i} strobe_max_hz {hz} out of (0, {MAX_STROBE_HZ}]",
                        self.fixture_id
                    )));
                }
            }
        }
        Ok(())
    }
}

/// The rig: universes + non-overlapping fixture footprints.
/// Overlap is a typed error — fixtures never silently share channels.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DmxRig {
    pub universes: BTreeMap<u16, DmxUniverse>,
    pub fixtures: Vec<DmxFixture>,
}

impl DmxRig {
    pub fn validate(&self) -> Result<(), Vec<SyncError>> {
        let mut errors = Vec::new();
        for f in &self.fixtures {
            if let Err(e) = f.validate() {
                errors.push(e);
            }
            if !self.universes.contains_key(&f.universe_index) {
                errors.push(SyncError::Dmx(format!(
                    "fixture {:?} references missing universe {}",
                    f.fixture_id, f.universe_index
                )));
            }
        }
        // Pairwise overlap within each universe.
        for (i, a) in self.fixtures.iter().enumerate() {
            for b in &self.fixtures[i + 1..] {
                if a.universe_index == b.universe_index
                    && a.start_address <= b.end_address()
                    && b.start_address <= a.end_address()
                {
                    errors.push(SyncError::Dmx(format!(
                        "fixtures {:?} and {:?} overlap universe {} ({}..{} vs {}..{})",
                        a.fixture_id,
                        b.fixture_id,
                        a.universe_index,
                        a.start_address,
                        a.end_address(),
                        b.start_address,
                        b.end_address()
                    )));
                }
            }
        }
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
