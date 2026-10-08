//! Tempo map and tick<->sample conversion.
//!
//! The tempo map is engine-owned musical state mirrored to the visual
//! lane via `VisualClockSyncRequest`. Conversions are piecewise-linear
//! over the map; rounding is nearest, ties away from zero
//! (CONTRACTS.md §1). Segment accumulation uses f64; at 48 kHz / 960 000
//! tpq a full day of timeline keeps double error below 1e-4 samples, far
//! inside the half-sample rounding band.

use serde::{Deserialize, Serialize};

use crate::types::TICKS_PER_QUARTER;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TempoPoint {
    pub at_ticks: i64,
    pub bpm: f64,
    pub numerator: u8,
    pub denominator: u8,
}

impl Default for TempoPoint {
    fn default() -> Self {
        Self {
            at_ticks: 0,
            bpm: 120.0,
            numerator: 4,
            denominator: 4,
        }
    }
}

/// Ordered tempo map. Invariant: sorted by `at_ticks`, first point may be
/// at any tick (positions before the first point extend that tempo
/// backwards to -inf).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TempoMap {
    pub points: Vec<TempoPoint>,
    pub sample_rate: u32,
}

impl Default for TempoMap {
    fn default() -> Self {
        Self {
            points: vec![TempoPoint::default()],
            sample_rate: 48_000,
        }
    }
}

impl TempoMap {
    pub fn new(points: Vec<TempoPoint>, sample_rate: u32) -> Self {
        let mut points = points;
        points.sort_by_key(|p| p.at_ticks);
        if points.is_empty() {
            points.push(TempoPoint::default());
        }
        Self {
            points,
            sample_rate,
        }
    }

    /// Round to nearest integer, ties away from zero (CONTRACTS.md §1).
    fn round_ties_away(v: f64) -> i64 {
        // f64::round is exactly "half away from zero".
        v.round() as i64
    }

    /// Samples spanned by `ticks` at `bpm` and this map's sample rate.
    fn span_samples(&self, ticks: f64, bpm: f64) -> f64 {
        // seconds = ticks / (tpq * quarters_per_sec); quarters/s = bpm/60
        ticks * 60.0 * (self.sample_rate as f64) / (bpm * TICKS_PER_QUARTER as f64)
    }

    fn point_index_at(&self, tick: i64) -> usize {
        match self.points.iter().rposition(|p| p.at_ticks <= tick) {
            Some(i) => i,
            None => 0,
        }
    }

    /// Cumulative sample position of `tick` (0 at tick 0).
    pub fn sample_at_tick(&self, tick: i64) -> i64 {
        // Walk segments from tick 0. For negative ticks we integrate
        // backwards through the first tempo segment.
        let sign = if tick >= 0 { 1.0f64 } else { -1.0 };
        let target = tick.abs() as f64;
        let mut acc = 0.0f64;
        // Establish segment starts on the + axis: points <= 0 start at 0.
        let mut seg_start = 0.0f64;
        let mut seg_bpm = self.tempo_at(0).bpm;
        let mut boundaries: Vec<(f64, f64)> = Vec::new();
        for p in &self.points {
            if (p.at_ticks as f64) > seg_start {
                boundaries.push((seg_start, seg_bpm));
                seg_start = p.at_ticks as f64;
                seg_bpm = p.bpm;
            }
        }
        boundaries.push((seg_start, seg_bpm));
        for (i, (start, bpm)) in boundaries.iter().enumerate() {
            if target <= *start {
                break;
            }
            let end = boundaries
                .get(i + 1)
                .map(|(s, _)| (*s).min(target))
                .unwrap_or(target);
            acc += self.span_samples(end - *start, *bpm);
        }
        if tick < 0 {
            // Integrate the negative axis with the first tempo segment
            // (points before/at 0 all collapse to the active tempo at 0).
            let bpm = self.tempo_at(0).bpm;
            acc = self.span_samples(target, bpm);
        }
        Self::round_ties_away(sign * acc)
    }

    /// Tick position of `sample` (inverse of `sample_at_tick`).
    pub fn tick_at_sample(&self, sample: i64) -> i64 {
        if sample <= 0 {
            // Negative/0 axis: first segment only.
            let bpm = self.tempo_at(0).bpm;
            let ticks = (sample as f64) * bpm * TICKS_PER_QUARTER as f64
                / (60.0 * self.sample_rate as f64);
            return Self::round_ties_away(ticks);
        }
        let mut acc_ticks = 0.0f64;
        let mut acc_samples = 0.0f64;
        let mut seg_start = 0i64;
        let mut seg_bpm = self.tempo_at(0).bpm;
        let mut segs: Vec<(i64, f64)> = Vec::new();
        for p in &self.points {
            if p.at_ticks > seg_start {
                segs.push((seg_start, seg_bpm));
                seg_start = p.at_ticks;
                seg_bpm = p.bpm;
            }
        }
        segs.push((seg_start, seg_bpm));
        for (i, (start, bpm)) in segs.iter().enumerate() {
            let end_ticks = if i + 1 < segs.len() {
                segs[i + 1].0
            } else {
                i64::MAX
            };
            let seg_samples = self.span_samples((end_ticks.min(i64::MAX / 2) - start).max(0) as f64, *bpm);
            if acc_samples + seg_samples >= sample as f64 {
                let remaining = sample as f64 - acc_samples;
                let ticks = remaining * bpm * TICKS_PER_QUARTER as f64
                    / (60.0 * self.sample_rate as f64);
                return Self::round_ties_away(*start as f64 + ticks);
            }
            acc_samples += seg_samples;
            acc_ticks += (end_ticks - start) as f64;
        }
        Self::round_ties_away(acc_ticks)
    }

    /// Tempo point active at `tick`.
    pub fn tempo_at(&self, tick: i64) -> TempoPoint {
        self.points[self.point_index_at(tick)]
    }

    /// Musical quarter-note index containing `tick`.
    pub fn beat_of_tick(tick: i64) -> i64 {
        tick.div_euclid(TICKS_PER_QUARTER)
    }

    /// Next quarter-note boundary strictly after `tick` (or at `tick`
    /// when it is exactly on a boundary — the boundary still counts).
    pub fn next_beat_after(&self, tick: i64) -> i64 {
        (Self::beat_of_tick(tick) + 1) * TICKS_PER_QUARTER
    }

    /// Next bar boundary at-or-after `tick` under the meter in effect
    /// there (meter = the tempo point governing `tick`).
    pub fn next_bar_after(&self, tick: i64) -> i64 {
        let meter = self.tempo_at(tick);
        let bar_ticks = TICKS_PER_QUARTER * i64::from(meter.numerator);
        // Bar boundaries align to musical 0 on the tick axis.
        let bar = tick.div_euclid(bar_ticks);
        let boundary = bar * bar_ticks;
        if boundary <= tick {
            boundary + bar_ticks
        } else {
            boundary
        }
    }

    /// Anchor position resolved to ticks. SAMPLE anchors convert through
    /// this map's sample rate; TIMECODE anchors convert ns -> samples ->
    /// ticks (absolute-time anchors hold wall-time position, so their
    /// *tick* position shifts with the tempo map — intended).
    pub fn anchor_ticks(&self, kind: crate::types::AnchorKind, ticks: i64, sample: i64, timecode_ns: i64) -> i64 {
        match kind {
            crate::types::AnchorKind::BeatTick => ticks,
            crate::types::AnchorKind::Sample => self.tick_at_sample(sample),
            crate::types::AnchorKind::Timecode => {
                let sample = Self::round_ties_away(
                    (timecode_ns as f64) * (self.sample_rate as f64) / 1.0e9,
                );
                self.tick_at_sample(sample)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_tempo_round_trip() {
        let m = TempoMap::default(); // 120bpm, 48k
        // One quarter at 120bpm = 0.5s = 24000 samples.
        assert_eq!(m.sample_at_tick(TICKS_PER_QUARTER), 24_000);
        assert_eq!(m.tick_at_sample(24_000), TICKS_PER_QUARTER);
        // One bar of 4/4 = 4 quarters.
        assert_eq!(m.sample_at_tick(4 * TICKS_PER_QUARTER), 96_000);
    }

    #[test]
    fn tempo_change_piecewise() {
        // 0..1bar at 120bpm, then 60bpm.
        let m = TempoMap::new(
            vec![
                TempoPoint::default(),
                TempoPoint {
                    at_ticks: 4 * TICKS_PER_QUARTER,
                    bpm: 60.0,
                    ..Default::default()
                },
            ],
            48_000,
        );
        // At the boundary the sample position is 96_000; one quarter
        // further at 60bpm = 1s = 48_000 more.
        assert_eq!(m.sample_at_tick(5 * TICKS_PER_QUARTER), 144_000);
        assert_eq!(m.tick_at_sample(144_000), 5 * TICKS_PER_QUARTER);
    }

    #[test]
    fn beat_and_bar_boundaries() {
        let m = TempoMap::default();
        assert_eq!(m.next_beat_after(0), TICKS_PER_QUARTER);
        assert_eq!(m.next_beat_after(TICKS_PER_QUARTER - 1), TICKS_PER_QUARTER);
        assert_eq!(m.next_bar_after(0), 4 * TICKS_PER_QUARTER);
        assert_eq!(m.next_bar_after(4 * TICKS_PER_QUARTER - 1), 4 * TICKS_PER_QUARTER);
        assert_eq!(m.next_bar_after(4 * TICKS_PER_QUARTER), 8 * TICKS_PER_QUARTER);
    }

    #[test]
    fn ties_round_away_from_zero() {
        assert_eq!(TempoMap::round_ties_away(0.5), 1);
        assert_eq!(TempoMap::round_ties_away(-0.5), -1);
        assert_eq!(TempoMap::round_ties_away(2.5), 3);
    }
}
