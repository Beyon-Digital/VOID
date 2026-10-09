//! MIDI Clock — 24 pulses-per-quarter-note timing and the
//! start/stop/continue transport model (PRO-03, T94 model side).
//!
//! Tick math is exact rational arithmetic: song position is in
//! VOID ticks (960 000 ticks/quarter per CONTRACTS §1); a MIDI clock
//! pulse is 1/24 of a quarter. `ticks_to_pulses` returns the exact
//! pulse index *and* the leftover fractional pulse in ticks so
//! jitter/alignment is measurable rather than rounded away.

use serde::{Deserialize, Serialize};

use crate::error::SyncError;

/// MIDI clock resolution: 24 pulses per quarter note.
pub const MIDI_CLOCK_PPQN: u32 = 24;

/// Transport real-time messages (single-byte system real-time).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MidiClockMessage {
    /// 0xF8 — one 24ppqn pulse.
    Tick,
    /// 0xFA — start from song position 0.
    Start,
    /// 0xFB — continue from the held position.
    Continue,
    /// 0xFC — stop.
    Stop,
}

impl MidiClockMessage {
    pub fn status_byte(self) -> u8 {
        match self {
            MidiClockMessage::Tick => 0xF8,
            MidiClockMessage::Start => 0xFA,
            MidiClockMessage::Continue => 0xFB,
            MidiClockMessage::Stop => 0xFC,
        }
    }

    pub fn from_status_byte(b: u8) -> Option<Self> {
        match b {
            0xF8 => Some(MidiClockMessage::Tick),
            0xFA => Some(MidiClockMessage::Start),
            0xFB => Some(MidiClockMessage::Continue),
            0xFC => Some(MidiClockMessage::Stop),
            _ => None,
        }
    }
}

/// Exact tick↔pulse conversion. `whole` pulses plus a fractional
/// remainder expressed in ticks (never silently rounded).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TickPulse {
    /// Whole clock pulses elapsed.
    pub pulses: i64,
    /// Leftover ticks toward the next pulse (0 .. pulses_per_tick).
    pub remainder_ticks: i64,
}

/// Convert VOID ticks → MIDI clock pulses for a song tempo map whose
/// resolution is `song_ppq` ticks per quarter. All arithmetic is
/// checked — overflow is a typed error, never a wrapped value.
pub fn ticks_to_pulses(ticks: i64, song_ppq: u32) -> Result<TickPulse, SyncError> {
    if song_ppq == 0 {
        return Err(SyncError::Clock("song_ppq must be > 0".into()));
    }
    let ppq = song_ppq as i64;
    let num = ticks
        .checked_mul(MIDI_CLOCK_PPQN as i64)
        .ok_or_else(|| SyncError::Clock("tick*pulses overflow".into()))?;
    let pulses = num.div_euclid(ppq);
    let rem_ticks = num.rem_euclid(ppq) / (MIDI_CLOCK_PPQN as i64);
    Ok(TickPulse {
        pulses,
        remainder_ticks: rem_ticks,
    })
}

/// Inverse of [`ticks_to_pulses`]: pulses → ticks (exact when the
/// pulse lands on a tick boundary for the given ppq; the remainder
/// reports sub-tick residue honestly).
pub fn pulses_to_ticks(pulses: i64, song_ppq: u32) -> Result<(i64, i64), SyncError> {
    if song_ppq == 0 {
        return Err(SyncError::Clock("song_ppq must be > 0".into()));
    }
    // ticks = pulses * ppq / 24
    let num = pulses
        .checked_mul(song_ppq as i64)
        .ok_or_else(|| SyncError::Clock("pulse*ppq overflow".into()))?;
    let ticks = num.div_euclid(MIDI_CLOCK_PPQN as i64);
    let rem = num.rem_euclid(MIDI_CLOCK_PPQN as i64);
    Ok((ticks, rem))
}

/// Song-position → clock-pulse mapping under a running transport.
/// This is the *model*: it says which pulse index a tick position
/// lands on, so sync loss/jumps are detectable, not smoothed over.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockTransport {
    /// Song ticks-per-quarter (contract: 960 000).
    pub song_ppq: u32,
    state: ClockState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
enum ClockState {
    Stopped,
    /// Running: song position `song_ticks_at_start` corresponded to
    /// clock pulse `pulse_at_start`. New pulses advance both.
    Running {
        song_ticks_at_start: i64,
        pulse_at_start: i64,
    },
}

/// What a decoded transport byte means for a running clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockEvent {
    Started {
        song_ticks: i64,
    },
    Continued {
        song_ticks: i64,
    },
    Stopped,
    Tick,
    /// A byte that is not a clock message.
    NotClock,
}

impl ClockTransport {
    pub fn new(song_ppq: u32) -> Result<Self, SyncError> {
        if song_ppq == 0 {
            return Err(SyncError::Clock("song_ppq must be > 0".into()));
        }
        Ok(ClockTransport {
            song_ppq,
            state: ClockState::Stopped,
        })
    }

    pub fn is_running(&self) -> bool {
        matches!(self.state, ClockState::Running { .. })
    }

    /// 0xFA — start at song position 0, pulse counter resets.
    pub fn start(&mut self) -> ClockEvent {
        self.state = ClockState::Running {
            song_ticks_at_start: 0,
            pulse_at_start: 0,
        };
        ClockEvent::Started { song_ticks: 0 }
    }

    /// 0xFB — continue from the held position. Honest: continue does
    /// not reset anything; it resumes the mapping.
    pub fn continue_at(&mut self, held_song_ticks: i64, held_pulse: i64) -> ClockEvent {
        self.state = ClockState::Running {
            song_ticks_at_start: held_song_ticks,
            pulse_at_start: held_pulse,
        };
        ClockEvent::Continued {
            song_ticks: held_song_ticks,
        }
    }

    /// 0xFC — stop; the mapping is held for a later continue.
    pub fn stop(&mut self) -> ClockEvent {
        self.state = ClockState::Stopped;
        ClockEvent::Stopped
    }

    /// Which pulse should a song position land on (running only).
    pub fn pulse_at(&self, song_ticks: i64) -> Result<i64, SyncError> {
        match self.state {
            ClockState::Stopped => Err(SyncError::Clock("clock not running".into())),
            ClockState::Running {
                song_ticks_at_start,
                pulse_at_start,
            } => {
                let delta = song_ticks
                    .checked_sub(song_ticks_at_start)
                    .ok_or_else(|| SyncError::Clock("tick delta overflow".into()))?;
                let tp = ticks_to_pulses(delta, self.song_ppq)?;
                pulse_at_start
                    .checked_add(tp.pulses)
                    .ok_or_else(|| SyncError::Clock("pulse overflow".into()))
            }
        }
    }

    /// Reverse: which song position a pulse lands on (running only).
    pub fn song_ticks_at(&self, pulse: i64) -> Result<i64, SyncError> {
        match self.state {
            ClockState::Stopped => Err(SyncError::Clock("clock not running".into())),
            ClockState::Running {
                song_ticks_at_start,
                pulse_at_start,
            } => {
                let delta = pulse
                    .checked_sub(pulse_at_start)
                    .ok_or_else(|| SyncError::Clock("pulse delta overflow".into()))?;
                let (ticks, _rem) = pulses_to_ticks(delta, self.song_ppq)?;
                song_ticks_at_start
                    .checked_add(ticks)
                    .ok_or_else(|| SyncError::Clock("tick overflow".into()))
            }
        }
    }

    /// Decode a wire byte through the clock model.
    pub fn apply_status_byte(
        &mut self,
        b: u8,
        held_song_ticks: i64,
        held_pulse: i64,
    ) -> ClockEvent {
        match MidiClockMessage::from_status_byte(b) {
            Some(MidiClockMessage::Start) => self.start(),
            Some(MidiClockMessage::Continue) => self.continue_at(held_song_ticks, held_pulse),
            Some(MidiClockMessage::Stop) => self.stop(),
            Some(MidiClockMessage::Tick) => ClockEvent::Tick,
            None => ClockEvent::NotClock,
        }
    }
}
