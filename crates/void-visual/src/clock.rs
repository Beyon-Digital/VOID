//! Audio-clock tracking (CONTRACTS.md §1, §7).
//!
//! The audio engine's `ClockSnapshot` telemetry is the ONLY timing source
//! for visuals — there is deliberately no wall-clock fallback for musical
//! scheduling. This module mirrors the `ClockSnapshot` wire shape and the
//! acceptance rules: snapshots are per-epoch monotonic (`sequence`), stale
//! epochs/sequences are discarded, and timeline discontinuities (seek,
//! loop wrap, restart) are detected rather than interpolated across.

use serde::{Deserialize, Serialize};

/// Mirror of `voidproto::ClockSnapshot` (protocol major 1, minor 0) — kept
/// as a plain struct so the visual lane can be fed by any transport that
/// already decoded the control-schema telemetry, and by tests.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClockSnapshot {
    pub project_id: String,
    pub engine_epoch: u64,
    /// Position on the (possibly looping) timeline, in samples.
    pub timeline_sample: i64,
    /// Monotonic device counter — never wraps on loops.
    pub device_sample_counter: i64,
    pub sample_rate: u32,
    /// Mirrors `voidproto::TransportState` numeric values.
    pub transport_state: u8,
    pub loop_start_ticks: i64,
    pub loop_end_ticks: i64,
    pub tempo_map_revision: u64,
    /// Per-epoch monotonic snapshot sequence.
    pub sequence: u64,
    /// Monotonic host clock, nanoseconds (diagnostics only).
    pub host_clock_ns: u64,
}

pub const TRANSPORT_STOPPED: u8 = 0;
pub const TRANSPORT_PLAYING: u8 = 1;
pub const TRANSPORT_RECORDING: u8 = 2;
pub const TRANSPORT_PAUSED: u8 = 3;

/// Outcome of pushing a snapshot into the tracker.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockAccept {
    /// First snapshot on this epoch.
    Established,
    /// Fresh snapshot accepted.
    Advanced,
    /// Same position/sequence family — accepted, no new info.
    Repeated,
    /// Timeline jump/loop/seek discontinuity — do not interpolate across.
    Discontinuity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockReject {
    /// Snapshot carries an older engine epoch.
    StaleEpoch,
    /// Sequence is not greater than the last accepted (reorder/replay).
    StaleSequence,
}

/// Acceptance rules for engine clock snapshots.
#[derive(Debug)]
pub struct ClockTracker {
    pub engine_epoch: u64,
    last: Option<ClockSnapshot>,
    /// Snapshots rejected for stale epoch (diagnostic counter).
    pub rejected_stale_epoch: u64,
    /// Snapshots rejected for non-advancing sequence.
    pub rejected_stale_sequence: u64,
    /// Snapshots skipped while newer ones were pending (drop accounting —
    /// incremented by the runtime's latest-slot channel, mirrored into
    /// `VisualFrameTelemetry.dropped_clocks`).
    pub dropped_clocks: u64,
}

impl ClockTracker {
    pub fn new(engine_epoch: u64) -> Self {
        Self {
            engine_epoch,
            last: None,
            rejected_stale_epoch: 0,
            rejected_stale_sequence: 0,
            dropped_clocks: 0,
        }
    }

    /// Epoch changes invalidate every outstanding snapshot.
    pub fn set_epoch(&mut self, epoch: u64) {
        if epoch != self.engine_epoch {
            self.engine_epoch = epoch;
            self.last = None;
        }
    }

    pub fn current(&self) -> Option<&ClockSnapshot> {
        self.last.as_ref()
    }

    /// Accept or reject a snapshot. Stale epochs and non-advancing
    /// sequences are discarded and counted, never interpolated.
    pub fn push(
        &mut self,
        snap: ClockSnapshot,
    ) -> std::result::Result<ClockAccept, ClockReject> {
        if snap.engine_epoch != self.engine_epoch {
            self.rejected_stale_epoch += 1;
            return Err(ClockReject::StaleEpoch);
        }
        match &self.last {
            None => {
                self.last = Some(snap);
                Ok(ClockAccept::Established)
            }
            Some(prev) => {
                if snap.sequence <= prev.sequence {
                    self.rejected_stale_sequence += 1;
                    return Err(ClockReject::StaleSequence);
                }
                let accept = if Self::is_discontinuity(prev, &snap) {
                    ClockAccept::Discontinuity
                } else if snap.timeline_sample == prev.timeline_sample
                    && snap.device_sample_counter == prev.device_sample_counter
                {
                    ClockAccept::Repeated
                } else {
                    ClockAccept::Advanced
                };
                self.last = Some(snap);
                Ok(accept)
            }
        }
    }

    /// A discontinuity is any case where the straight-line interval
    /// between snapshots is not real elapsed playback: loop wrap
    /// (timeline moves backwards while the device counter advances),
    /// seek/restart (timeline jumps by more than the elapsed device
    /// samples could explain), or transport leaving/entering a rolling
    /// state. Interpolation across these is forbidden.
    fn is_discontinuity(prev: &ClockSnapshot, snap: &ClockSnapshot) -> bool {
        if prev.transport_state != snap.transport_state {
            return true;
        }
        let dev_delta = snap.device_sample_counter - prev.device_sample_counter;
        let tl_delta = snap.timeline_sample - prev.timeline_sample;
        if tl_delta < 0 {
            return true; // loop wrap or rewind
        }
        // Allow small jitter (device and timeline counters drift by up to
        // one render quantum in either direction); anything larger is a
        // jump the transport did not play through.
        if dev_delta >= 0 && (tl_delta - dev_delta).abs() > 4096 {
            return true;
        }
        if dev_delta < 0 {
            return true; // device counter went backwards — epoch-class event
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(seq: u64, tl: i64, dev: i64) -> ClockSnapshot {
        ClockSnapshot {
            project_id: "p".into(),
            engine_epoch: 7,
            timeline_sample: tl,
            device_sample_counter: dev,
            sample_rate: 48_000,
            transport_state: TRANSPORT_PLAYING,
            loop_start_ticks: 0,
            loop_end_ticks: 0,
            tempo_map_revision: 1,
            sequence: seq,
            host_clock_ns: seq * 1_000_000,
        }
    }

    #[test]
    fn monotonic_accept_and_stale_reject() {
        let mut t = ClockTracker::new(7);
        assert_eq!(t.push(snap(1, 0, 0)), Ok(ClockAccept::Established));
        assert_eq!(t.push(snap(2, 480, 480)), Ok(ClockAccept::Advanced));
        assert_eq!(
            t.push(snap(2, 480, 480)),
            Err(ClockReject::StaleSequence)
        );
        let mut old = snap(9, 10, 10);
        old.engine_epoch = 6;
        assert_eq!(t.push(old), Err(ClockReject::StaleEpoch));
        assert_eq!(t.rejected_stale_epoch, 1);
        assert_eq!(t.rejected_stale_sequence, 1);
    }

    #[test]
    fn loop_wrap_and_seek_are_discontinuities() {
        let mut t = ClockTracker::new(7);
        t.push(snap(1, 0, 0)).unwrap();
        t.push(snap(2, 4800, 4800)).unwrap();
        // Seek backwards on the timeline while device continues.
        assert_eq!(
            t.push(snap(3, 100, 9600)),
            Ok(ClockAccept::Discontinuity)
        );
        // Forward jump far ahead of device progress is also a seek.
        assert_eq!(
            t.push(snap(4, 1_000_000, 10_000)),
            Ok(ClockAccept::Discontinuity)
        );
    }

    #[test]
    fn epoch_change_resets() {
        let mut t = ClockTracker::new(7);
        t.push(snap(1, 0, 0)).unwrap();
        t.set_epoch(8);
        let mut s = snap(1, 0, 0);
        s.engine_epoch = 8;
        assert_eq!(t.push(s), Ok(ClockAccept::Established));
    }
}
