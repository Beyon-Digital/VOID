//! Movie-scoring anchors — absolute-time marks that survive tempo-map
//! changes (PRO-02, T91).
//!
//! An [`Anchor`] stores an absolute position in seconds as an exact
//! [`Rat`]. Its *tick* position is never stored — it is derived through
//! the score's tempo map, so editing the tempo map moves an anchor's
//! musical position while its picture-locked time stays fixed. Both
//! directions are exact rational arithmetic: `ticks_at` then
//! `time_at_ticks` round-trips to the identical rational, and
//! `round-trip` tests assert position invariance to zero error.
//!
//! [`TimecodeMode`] declares the project's frame rate (rational
//! numerator/denominator, e.g. 30000/1001) and drop-frame rule.
//! `format_timecode` produces the SMPTE label; `timecode_frame` returns
//! the *real* frame index (labels are a display concern — an anchor's
//! truth is seconds).

use serde::{Deserialize, Serialize};

use crate::model::TempoPoint;
use crate::model::TICKS_PER_QUARTER;

/// Minimal exact rational (`num`/`den`, `den > 0`, always reduced).
/// i128 numerator keeps products of tick×bpm intermediates in range for
/// any realistic score. Serde: canonical `"num/den"` string — a bare
/// integer serializes as `"n"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rat {
    num: i128,
    den: u64,
}

fn gcd_i128(mut a: i128, mut b: i128) -> i128 {
    a = a.abs();
    b = b.abs();
    while b != 0 {
        let t = a % b;
        a = b;
        b = t;
    }
    a.max(1)
}

impl Rat {
    pub fn new(num: i128, den: i128) -> Self {
        assert!(den != 0, "Rat denominator 0");
        let (num, den) = if den < 0 { (-num, -den) } else { (num, den) };
        let g = gcd_i128(num, den);
        Self {
            num: num / g,
            den: (den / g) as u64,
        }
    }
    pub fn i64(v: i64) -> Self {
        Self::new(v as i128, 1)
    }
    pub fn num(&self) -> i128 {
        self.num
    }
    pub fn den(&self) -> u64 {
        self.den
    }
    /// Every finite f64 is a dyadic rational; this conversion is exact
    /// (never introduces representation error), which is what makes the
    /// anchor tick↔seconds round-trip lossless even for decimal bpm.
    pub fn from_f64(v: f64) -> Self {
        assert!(v.is_finite(), "Rat::from_f64 non-finite");
        let bits = v.to_bits();
        let sign = if bits >> 63 == 1 { -1i128 } else { 1i128 };
        let exp = ((bits >> 52) & 0x7ff) as i64;
        let frac = (bits & 0xf_ffff_ffff_ffff) as i128;
        let (mantissa, e) = if exp == 0 {
            (frac, -1074) // subnormal
        } else {
            (frac | 0x10_0000_0000_0000, exp - 1075)
        };
        let mut num = sign * mantissa;
        let mut den = 1i128;
        if e >= 0 {
            num <<= e;
        } else {
            den <<= -e;
        }
        Rat::new(num, den)
    }
    pub fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
    }
    /// Round to the nearest integer, ties away from zero — the
    /// CONTRACTS §1 rounding rule applied at sample/tick boundaries.
    pub fn round_away(self) -> i64 {
        let n = self.num;
        let d = self.den as i128;
        let q = n / d;
        let r = (n % d).abs();
        if r * 2 >= d {
            if n >= 0 {
                (q + 1) as i64
            } else {
                (q - 1) as i64
            }
        } else {
            q as i64
        }
    }
}

// Exact rational ordering — cross-multiplied, NOT field order (a
// derived Ord would compare num then den, which misorders e.g.
// 899899/15000 vs 60/1).
impl Ord for Rat {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        (self.num * o.den as i128).cmp(&(o.num * self.den as i128))
    }
}
impl PartialOrd for Rat {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}

impl std::ops::Add for Rat {
    type Output = Rat;
    fn add(self, o: Rat) -> Rat {
        Rat::new(
            self.num * o.den as i128 + o.num * self.den as i128,
            self.den as i128 * o.den as i128,
        )
    }
}
impl std::ops::Sub for Rat {
    type Output = Rat;
    fn sub(self, o: Rat) -> Rat {
        Rat::new(
            self.num * o.den as i128 - o.num * self.den as i128,
            self.den as i128 * o.den as i128,
        )
    }
}
impl std::ops::Mul for Rat {
    type Output = Rat;
    fn mul(self, o: Rat) -> Rat {
        Rat::new(self.num * o.num, self.den as i128 * o.den as i128)
    }
}
impl std::ops::Div for Rat {
    type Output = Rat;
    fn div(self, o: Rat) -> Rat {
        Rat::new(self.num * o.den as i128, self.den as i128 * o.num)
    }
}

impl Serialize for Rat {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        if self.den == 1 {
            s.serialize_str(&self.num.to_string())
        } else {
            s.serialize_str(&format!("{}/{}", self.num, self.den))
        }
    }
}

impl<'de> Deserialize<'de> for Rat {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum V {
            S(String),
            I(i128),
            F(f64),
        }
        match V::deserialize(d)? {
            V::S(s) => {
                let s = s.trim();
                if let Some((a, b)) = s.split_once('/') {
                    let n: i128 = a.trim().parse().map_err(serde::de::Error::custom)?;
                    let de: i128 = b.trim().parse().map_err(serde::de::Error::custom)?;
                    if de <= 0 {
                        return Err(serde::de::Error::custom("rat denominator <= 0"));
                    }
                    Ok(Rat::new(n, de))
                } else {
                    s.parse::<i128>()
                        .map(Rat::i64_from_i128)
                        .map_err(serde::de::Error::custom)
                }
            }
            V::I(n) => Ok(Rat::i64_from_i128(n)),
            V::F(f) => Ok(Rat::from_f64(f)),
        }
    }
}

impl Rat {
    fn i64_from_i128(n: i128) -> Self {
        Rat::new(n, 1)
    }
}

/// Piecewise-constant tempo map, ticks→seconds and back. Exactly the
/// anchor boundary: the score's `tempo_map` feeds these converters.
#[derive(Debug, Clone, PartialEq)]
pub struct TempoMap {
    /// Sorted by `at_ticks`, strictly increasing, all bpm > 0.
    /// An empty map means "unspecified" — callers supply `default_bpm`.
    pub points: Vec<TempoPoint>,
}

impl TempoMap {
    /// Seconds per tick at tempo `bpm` (exact): 60 / (bpm × TPQ).
    fn sec_per_tick(bpm: f64) -> Rat {
        Rat::i64(60) / (Rat::from_f64(bpm) * Rat::i64(TICKS_PER_QUARTER))
    }

    /// Effective points: the stored map plus an implicit first point at
    /// tick 0 with `default_bpm` when the map doesn't start at 0 (or is
    /// empty).
    fn effective(&self, default_bpm: f64) -> Vec<TempoPoint> {
        match self.points.first() {
            Some(p) if p.at_ticks == 0 => self.points.clone(),
            Some(p) if p.at_ticks > 0 => {
                let mut v = vec![TempoPoint {
                    at_ticks: 0,
                    bpm: default_bpm,
                }];
                v.extend(self.points.iter().cloned());
                v
            }
            Some(_) => self.points.clone(), // negative first point is legal
            None => vec![TempoPoint {
                at_ticks: 0,
                bpm: default_bpm,
            }],
        }
    }

    /// Absolute time in seconds at `ticks` (exact rational). Ticks
    /// before the first tempo point integrate backwards at that point's
    /// bpm (pickup measures stay honest, not clamped to zero).
    pub fn time_at_ticks(&self, ticks: i64, default_bpm: f64) -> Rat {
        let pts = self.effective(default_bpm);
        let first = &pts[0];
        if ticks <= first.at_ticks {
            return Rat::i64(ticks - first.at_ticks) * Self::sec_per_tick(first.bpm);
        }
        let mut seconds = Rat::i64(0);
        for (i, p) in pts.iter().enumerate() {
            let next = pts.get(i + 1).map(|q| q.at_ticks).unwrap_or(i64::MAX);
            let seg_end = ticks.min(next);
            if seg_end > p.at_ticks {
                seconds = seconds + Rat::i64(seg_end - p.at_ticks) * Self::sec_per_tick(p.bpm);
            }
            if ticks <= next {
                break;
            }
        }
        seconds
    }

    /// Tick position at absolute `seconds` (exact inverse, then nearest
    /// tick, ties away from zero per CONTRACTS).
    pub fn ticks_at(&self, seconds: Rat, default_bpm: f64) -> i64 {
        let pts = self.effective(default_bpm);
        let mut cum = Rat::i64(0);
        for (i, p) in pts.iter().enumerate() {
            let next = pts.get(i + 1).map(|q| q.at_ticks).unwrap_or(i64::MAX);
            let span_ticks = if next == i64::MAX {
                i64::MAX - 1
            } else {
                next - p.at_ticks
            };
            let seg_seconds = Rat::i64(span_ticks) * Self::sec_per_tick(p.bpm);
            if seconds < cum + seg_seconds {
                let within = (seconds - cum) / Self::sec_per_tick(p.bpm);
                return (Rat::i64(p.at_ticks) + within).round_away();
            }
            cum = cum + seg_seconds;
        }
        // Past every segment — extend at final bpm.
        let last = pts.last().unwrap();
        let within = (seconds - cum) / Self::sec_per_tick(last.bpm);
        (Rat::i64(last.at_ticks) + within).round_away()
    }
}

/// What an anchor marks — vocabulary only; all kinds behave identically
/// in the timing math.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum AnchorKind {
    /// Picture event the music must hit.
    Hit,
    /// Free cue label.
    Marker,
    /// Session/film start mark.
    Start,
    End,
    /// Bar/measure lock — resolves like a hit but semantically claims a
    /// barline should land on this time.
    Barline,
    Custom {
        name: String,
    },
}

/// An absolute-time score anchor. `at_seconds` is the only stored
/// position — tick position is derived via [`Score::anchor_ticks`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Anchor {
    /// Stable opaque id (uuid or derived).
    pub id: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub kind: Option<AnchorKind>,
    /// Absolute position, seconds from score zero. Exact rational.
    pub at_seconds: Rat,
    /// Optional pre-computed SMPTE label recorded at authoring time —
    /// informational only; `format_timecode` is authoritative.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timecode_hint: Option<String>,
}

/// Drop-frame rule. `Drop { per_minute }` skips `per_minute` *labels* at
/// the top of every minute not divisible by 10 (2 for 29.97 DF, 4 for
/// 59.94 DF). Frames themselves are never dropped — it's a numbering
/// convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum DropFrame {
    #[default]
    NonDrop,
    Drop {
        per_minute: u32,
    },
}

/// Declared project timecode mode (PRO-02: "declared timecode modes").
/// `num`/`den` is the *real* frame rate as a rational — 30000/1001 for
/// NTSC 29.97 — not a rounded integer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimecodeMode {
    /// Frame-rate numerator/denominator, e.g. (24,1), (30000,1001).
    pub num: u64,
    pub den: u64,
    #[serde(default)]
    pub drop: DropFrame,
    /// Session start offset in frames applied to displayed labels.
    /// Seconds-zero of the score labels as `start_frames + 0`.
    #[serde(default)]
    pub start_frames: u64,
}

impl TimecodeMode {
    /// 24 fps film.
    pub const FPS_24: Self = Self {
        num: 24,
        den: 1,
        drop: DropFrame::NonDrop,
        start_frames: 0,
    };
    /// 25 fps PAL/EBU.
    pub const FPS_25: Self = Self {
        num: 25,
        den: 1,
        drop: DropFrame::NonDrop,
        start_frames: 0,
    };
    /// 30 fps non-drop.
    pub const FPS_30: Self = Self {
        num: 30,
        den: 1,
        drop: DropFrame::NonDrop,
        start_frames: 0,
    };
    /// 29.97 fps non-drop ("30 NDF" at NTSC rate).
    pub const FPS_2997_NDF: Self = Self {
        num: 30000,
        den: 1001,
        drop: DropFrame::NonDrop,
        start_frames: 0,
    };
    /// 29.97 fps drop-frame — labels skip 2 frame numbers per minute
    /// except every 10th minute.
    pub const FPS_2997_DF: Self = Self {
        num: 30000,
        den: 1001,
        drop: DropFrame::Drop { per_minute: 2 },
        start_frames: 0,
    };
    /// 59.94 fps drop-frame.
    pub const FPS_5994_DF: Self = Self {
        num: 60000,
        den: 1001,
        drop: DropFrame::Drop { per_minute: 4 },
        start_frames: 0,
    };

    /// Exact frame rate as rational fps.
    pub fn fps(&self) -> Rat {
        Rat::new(self.num as i128, self.den as i128)
    }
    /// Nominal (label) frame rate — the rounded integer the SMPTE label
    /// counts in (29.97fps labels count 30 frames/second).
    pub fn nominal_fps(&self) -> u64 {
        self.fps().round_away() as u64
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.num == 0 || self.den == 0 {
            return Err("timecode num/den must be non-zero".into());
        }
        let nominal = self.nominal_fps();
        if nominal == 0 || nominal > 240 {
            return Err(format!("nominal fps {nominal} out of range"));
        }
        if let DropFrame::Drop { per_minute } = self.drop {
            if per_minute == 0 || per_minute >= nominal as u32 {
                return Err(format!(
                    "drop {per_minute} invalid for nominal fps {nominal}"
                ));
            }
            // SMPTE drop is only defined for rates where nominal*1001/1000
            // equals the declared num/den (29.97, 59.94, ...). Enforce it:
            // reject invented drop modes like 24 DF.
            let real = self.num as f64 / self.den as f64;
            let expected = nominal as f64 * 1000.0 / 1001.0;
            if (real - expected).abs() / expected > 1e-9 {
                return Err(format!(
                    "drop-frame only defined at nominal×1000/1001 rates; declared {}/{} nominal {nominal}",
                    self.num, self.den
                ));
            }
        }
        Ok(())
    }
}

/// Real frame index at `seconds` (round nearest, ties away — the same
/// rounding rule as tick conversion).
pub fn timecode_frame(seconds: Rat, mode: &TimecodeMode) -> i64 {
    (seconds * mode.fps()).round_away() + mode.start_frames as i64
}

/// Seconds of a real frame index (exact inverse of `timecode_frame`).
pub fn frame_seconds(frame: i64, mode: &TimecodeMode) -> Rat {
    Rat::i64(frame - mode.start_frames as i64) / mode.fps()
}

/// SMPTE label `HH:MM:SS:FF` (`;FF` for drop-frame). `frame_index` is a
/// *real* frame number — for drop modes the label renumbers it per the
/// SMPTE convention.
pub fn format_timecode(frame_index: i64, mode: &TimecodeMode) -> String {
    let nominal = mode.nominal_fps() as i64;
    let (label_frames, sep) = match mode.drop {
        DropFrame::NonDrop => (frame_index, ':'),
        DropFrame::Drop { per_minute } => {
            let drop = per_minute as i64;
            let frames_per_min = nominal * 60 - drop;
            let frames_per_10min = nominal * 600 - drop * 9;
            let d = frame_index / frames_per_10min;
            let m = frame_index % frames_per_10min;
            let extra = if m >= drop {
                (m - drop) / frames_per_min
            } else {
                0
            };
            (frame_index + drop * 9 * d + drop * extra, ';')
        }
    };
    let ff = label_frames % nominal;
    let total_seconds = label_frames / nominal;
    let ss = total_seconds % 60;
    let mm = (total_seconds / 60) % 60;
    let hh = total_seconds / 3600;
    format!("{hh:02}:{mm:02}:{ss:02}{sep}{ff:02}")
}

/// Convenience: label for a time position.
pub fn timecode_label(seconds: Rat, mode: &TimecodeMode) -> String {
    format_timecode(timecode_frame(seconds, mode), mode)
}
