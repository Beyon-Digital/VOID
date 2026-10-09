//! Rational frame-rate math (CONTRACTS.md §7, T87).
//!
//! Video frame rates are exact rationals — NTSC video is 30000/1001
//! (~29.97), film on NTSC is 24000/1001 (~23.976), PAL is 25/1.
//! All frame↔sample conversions here use u128 integer arithmetic over
//! `(frame, rate_num, rate_den, sample_rate)` so no floating-point
//! rounding ever sneaks into scheduling. The rounding policy used is an
//! explicit field on the spec/plan, not an ambient default.

use serde::{Deserialize, Serialize};

use crate::error::{AvError, Result};

/// Rational video frame rate: `num/den` frames per second. Stored
/// non-normalized (30000/1001 stays 30000/1001) — the declared numbers
/// are part of the configuration identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameRate {
    pub num: u64,
    pub den: u64,
}

pub const NTSC_30: FrameRate = FrameRate {
    num: 30000,
    den: 1001,
};
pub const NTSC_24: FrameRate = FrameRate {
    num: 24000,
    den: 1001,
};

/// Explicit rounding policy for fractional sample↔frame boundaries.
/// Required on every spec (CONTRACTS.md §7: "frame rounding/tail
/// duration must be explicit").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum FrameRounding {
    /// Nearest integer; exact halves round away from zero. Matches the
    /// project crate's sample-domain rounding convention.
    #[default]
    NearestTiesAway,
    Floor,
    Ceil,
}

/// `num/den` → integer under the given rounding policy (num,den ≥ 0).
pub fn rational_round(num: u128, den: u128, policy: FrameRounding) -> u64 {
    debug_assert!(den > 0);
    let q = num / den;
    let r = num % den;
    let v = match policy {
        FrameRounding::Floor => q,
        FrameRounding::Ceil => {
            if r == 0 {
                q
            } else {
                q + 1
            }
        }
        // ties away from zero: 2r >= den rounds up
        FrameRounding::NearestTiesAway => {
            if 2 * r >= den {
                q + 1
            } else {
                q
            }
        }
    };
    u64::try_from(v).expect("rational result fits u64")
}

impl FrameRate {
    pub fn new(num: u64, den: u64) -> Result<Self> {
        if num == 0 || den == 0 {
            return Err(AvError::InvalidSpec(
                "frame rate num/den must be non-zero".into(),
            ));
        }
        // Sanity bound: refuses absurd rates rather than overflowing
        // later arithmetic. u32 ceiling keeps spec JSON simple.
        if num > u32::MAX as u64 || den > u32::MAX as u64 || num / den > 1000 {
            return Err(AvError::InvalidSpec(format!(
                "frame rate {num}/{den} outside supported range"
            )));
        }
        Ok(Self { num, den })
    }

    /// The sampling interval of one video frame, in seconds, as an
    /// exact rational `den/num`. (NTSC 30: 1001/30000 s ≈ 33.3667 ms.)
    pub fn frame_interval(&self) -> (u64, u64) {
        (self.den, self.num)
    }

    /// Audio samples covered by `frames` video frames at `sample_rate`,
    /// rounded per `policy`: `frames * rate_den * sample_rate / rate_num`.
    pub fn samples_for_frames(&self, frames: u64, sample_rate: u32, policy: FrameRounding) -> u64 {
        let num = frames as u128 * self.den as u128 * sample_rate as u128;
        rational_round(num, self.num as u128, policy)
    }

    /// Audio sample index at which video `frame_index` *starts* —
    /// i.e. the first audio sample whose timestamp lies within the
    /// frame, using `policy` on `frame_index * den * sr / num`.
    pub fn frame_start_sample(
        &self,
        frame_index: u64,
        sample_rate: u32,
        policy: FrameRounding,
    ) -> u64 {
        self.samples_for_frames(frame_index, sample_rate, policy)
    }

    /// The video frame containing audio `sample`. Frame f owns
    /// `[start(f), start(f+1))` where `start` is a rational floor, so
    /// the inverse is `floor(((sample+1)*num - 1) / (sr*den))` — NOT
    /// `floor(sample*num/(sr*den))`: a frame-start sample whose ideal
    /// position is fractional (NTSC-30 frame 1 opens at floor(1601.6)
    /// = 1601 while 1601*30000/(48000*1001) < 1) belongs to the frame
    /// it opens, not the previous one.
    pub fn frame_index_at_sample(&self, sample: u64, sample_rate: u32) -> u64 {
        let num = (sample as u128 + 1) * self.num as u128;
        let den = sample_rate as u128 * self.den as u128;
        u64::try_from((num - 1) / den).expect("frame index fits u64")
    }

    /// Number of video frames needed to fully cover `samples` audio
    /// samples: `ceil(samples * num / (sr * den))` — a fractional frame
    /// requires the whole frame to be emitted. For NTSC 30 at 48 kHz,
    /// exactly 60 s of audio needs 1799 frames (not 1800): 1800 frames
    /// of 30000/1001 is 60.06 s.
    pub fn frames_covering_samples(&self, samples: u64, sample_rate: u32) -> u64 {
        let num = samples as u128 * self.num as u128;
        let den = sample_rate as u128 * self.den as u128;
        u64::try_from(num.div_ceil(den)).expect("frame count fits u64")
    }
}

/// Where a cue (marker/onset timestamp) lands relative to video frame
/// boundaries — used for the ≤1-frame cue-tolerance evidence (T87).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CuePlacement {
    /// Video frame index containing the cue's sample position.
    pub frame_index: u64,
    /// Audio sample at which that frame starts (per spec rounding).
    pub frame_start_sample: u64,
    /// Samples the cue is into its frame: `at_sample - frame_start`.
    /// 0..=one-frame-sample-length. This is the measured drift recorded
    /// into provenance.
    pub offset_samples: u64,
    /// True when the cue sits within one frame of a frame boundary —
    /// i.e. `offset_samples` ≤ the cue tolerance for this rate.
    pub within_cue_tolerance: bool,
}

/// Declared cue tolerance: one video frame (T87 "declared one-frame cue
/// tolerance"). A cue is on-tolerance when its offset inside the frame
/// does not exceed one full frame interval.
pub fn place_cue(
    at_sample: u64,
    rate: &FrameRate,
    sample_rate: u32,
    rounding: FrameRounding,
) -> CuePlacement {
    let frame_index = rate.frame_index_at_sample(at_sample, sample_rate);
    let start = rate.frame_start_sample(frame_index, sample_rate, rounding);
    let offset = at_sample.saturating_sub(start);
    let one_frame = rate.samples_for_frames(1, sample_rate, FrameRounding::Ceil);
    CuePlacement {
        frame_index,
        frame_start_sample: start,
        offset_samples: offset,
        // offset can exceed one frame only if rounding moved the start
        // past the cue sample — report that honestly as out of tolerance.
        within_cue_tolerance: offset <= one_frame,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: u32 = 48_000;

    #[test]
    fn ntsc30_frame_interval_is_not_integer_samples() {
        // 48000 * 1001 / 30000 = 1601.6 samples per frame.
        assert_eq!(
            NTSC_30.samples_for_frames(1, SR, FrameRounding::Floor),
            1601
        );
        assert_eq!(NTSC_30.samples_for_frames(1, SR, FrameRounding::Ceil), 1602);
        assert_eq!(
            NTSC_30.samples_for_frames(1, SR, FrameRounding::NearestTiesAway),
            1602
        );
    }

    #[test]
    fn ntsc30_frame_30_starts_at_exact_sample_48048() {
        // 30 * 48000 * 1001 / 30000 = 48048 exactly.
        assert_eq!(
            NTSC_30.frame_start_sample(30, SR, FrameRounding::Floor),
            48048
        );
        assert_eq!(NTSC_30.frame_index_at_sample(48048, SR), 30);
        assert_eq!(NTSC_30.frame_index_at_sample(48047, SR), 29);
        // Fractional-boundary starts map to the frame they open:
        // frame 1's first sample is floor(1601.6) = 1601.
        assert_eq!(NTSC_30.frame_index_at_sample(1601, SR), 1);
        assert_eq!(NTSC_30.frame_index_at_sample(1600, SR), 0);
        assert_eq!(NTSC_30.frame_index_at_sample(3203, SR), 2);
        assert_eq!(NTSC_30.frame_index_at_sample(3202, SR), 1);
    }

    #[test]
    fn one_minute_of_48k_audio_needs_1799_ntsc30_frames() {
        // ceil(2_880_000 * 30000 / (48000*1001)) = ceil(1798.20…) = 1799.
        assert_eq!(NTSC_30.frames_covering_samples(60 * SR as u64, SR), 1799);
    }

    #[test]
    fn integer_rates_round_trip() {
        let pal = FrameRate::new(25, 1).unwrap();
        assert_eq!(
            pal.samples_for_frames(100, SR, FrameRounding::Floor),
            192_000
        );
        assert_eq!(pal.frame_index_at_sample(191_999, SR), 99);
        assert_eq!(pal.frame_index_at_sample(192_000, SR), 100);
    }

    #[test]
    fn cue_on_frame_boundary_has_zero_drift() {
        let c = place_cue(48048, &NTSC_30, SR, FrameRounding::NearestTiesAway);
        assert_eq!(c.frame_index, 30);
        assert_eq!(c.offset_samples, 0);
        assert!(c.within_cue_tolerance);
    }

    #[test]
    fn zero_den_rejected() {
        assert!(FrameRate::new(30, 0).is_err());
        assert!(FrameRate::new(0, 1).is_err());
    }
}
