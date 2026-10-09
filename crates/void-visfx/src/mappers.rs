//! Audio-feature → scene-parameter mappers (T85/VIS-03 reactive side).
//! Analyzers run over rendered PCM frames (facts, never live audio on
//! this box); mappers normalize to 0..1 deterministically — same
//! fixture PCM + descriptors ⇒ identical outputs, bit-for-bit.

use crate::error::{Result, VisFxError};
use serde::{Deserialize, Serialize};

/// Analyzer descriptor set — one entry per feature to extract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalyzerSet {
    /// Analysis frame length in samples (per channel summed mono).
    pub frame_size: usize,
    /// Hop between frames in samples (≤ frame_size).
    pub hop: usize,
    pub sample_rate: u32,
    #[serde(default)]
    pub features: Vec<FeatureKind>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureKind {
    Rms,
    Peak,
    Onset,
    /// Band energy for `band` of `bands` equal-width linear bands.
    BandEnergy {
        band: u32,
        bands: u32,
    },
}

impl AnalyzerSet {
    pub fn validate(&self) -> Result<()> {
        if self.frame_size == 0 || self.hop == 0 || self.hop > self.frame_size {
            return Err(VisFxError::InvalidSpec(
                "frame/hop must satisfy 0<hop<=frame_size".into(),
            ));
        }
        if self.sample_rate == 0 {
            return Err(VisFxError::InvalidSpec("sample_rate=0".into()));
        }
        for f in &self.features {
            if let FeatureKind::BandEnergy { band, bands } = f {
                if *bands == 0 || *band >= *bands || *bands > 64 {
                    return Err(VisFxError::InvalidSpec(format!(
                        "band {band}/{bands} out of range"
                    )));
                }
            }
        }
        Ok(())
    }
}

/// Per-frame feature output (deterministic — floats through a
/// documented fixed pipeline, no platform-dependent math).
#[derive(Debug, Clone, Default)]
pub struct FeatureFrames {
    /// rms normalized to 0..1 by full-scale.
    pub rms: Vec<f32>,
    /// peak |x| per frame.
    pub peak: Vec<f32>,
    /// Onset strength per frame (spectral-flux-free: energy-delta
    /// rectified, normalized by max so outputs are stable 0..1).
    pub onset: Vec<f32>,
    /// (band, bands) → per-frame band energy normalized to frame energy.
    pub band_energy: Vec<(u32, u32, Vec<f32>)>,
}

/// Interleaved PCM (any channel count) → mono f32 frame slices.
fn frames<'a>(pcm: &'a [f32], set: &'a AnalyzerSet) -> impl Iterator<Item = &'a [f32]> + 'a {
    let mut i = 0usize;
    std::iter::from_fn(move || {
        if i + set.frame_size > pcm.len() {
            None
        } else {
            let f = &pcm[i..i + set.frame_size];
            i += set.hop;
            Some(f)
        }
    })
}

pub fn analyze(pcm: &[f32], set: &AnalyzerSet) -> Result<FeatureFrames> {
    set.validate()?;
    let mut out = FeatureFrames::default();
    let want = |k: &FeatureKind| set.features.contains(k);
    let want_onset = set.features.iter().any(|f| matches!(f, FeatureKind::Onset));
    let mut prev_energy = 0f32;
    let mut energies = Vec::new();
    for f in frames(pcm, set) {
        let mut sum_sq = 0f64;
        let mut peak = 0f32;
        for &x in f {
            sum_sq += (x as f64) * (x as f64);
            let a = x.abs();
            if a > peak {
                peak = a;
            }
        }
        let n = f.len() as f64;
        let rms = (sum_sq / n).sqrt() as f32;
        if want(&FeatureKind::Rms) {
            out.rms.push(rms.clamp(0.0, 1.0));
        }
        if want(&FeatureKind::Peak) {
            out.peak.push(peak.clamp(0.0, 1.0));
        }
        let e = sum_sq as f32;
        energies.push(e);
        if want_onset {
            // frame 0 has no prior context — convention: no onset at
            // t=0, so a cold start can't dominate the feature.
            out.onset.push(if energies.len() == 1 {
                0.0
            } else {
                (e - prev_energy).max(0.0)
            });
        }
        prev_energy = e;
        for fk in &set.features {
            if let FeatureKind::BandEnergy { band, bands } = fk {
                let (band, bands) = (*band as usize, *bands as usize);
                let per = f.len() / bands.max(1);
                let mut be = 0f64;
                if per > 0 {
                    for &x in &f[band * per..((band + 1) * per).min(f.len())] {
                        be += (x as f64) * (x as f64);
                    }
                }
                let norm = if sum_sq > 0.0 {
                    (be / sum_sq) as f32
                } else {
                    0.0
                };
                let v = norm.clamp(0.0, 1.0);
                if let Some((_, _, vec)) = out
                    .band_energy
                    .iter_mut()
                    .find(|(b, bs, _)| *b == band as u32 && *bs == bands as u32)
                {
                    vec.push(v);
                } else {
                    out.band_energy.push((band as u32, bands as u32, vec![v]));
                }
            }
        }
    }
    // onset normalization: scale so max onset = 1 (0 when all silent)
    if want_onset {
        let max = out.onset.iter().copied().fold(0f32, f32::max);
        if max > 0.0 {
            for v in &mut out.onset {
                *v /= max;
            }
        }
    }
    let _ = energies;
    Ok(out)
}

// ---------------------------------------------------------------------------
// Normalized mappers — feature value → scene param value (0..1)
// ---------------------------------------------------------------------------

/// Mapper kind declared on an AudioBinding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapperKind {
    Linear,
    /// sqrt(x) — expands low end (quiet material still moves).
    Sqrt,
    /// db floor: 0 at -60dBFS, 1 at 0dBFS, linear in between.
    DbFloor,
}

impl MapperKind {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "linear" => Ok(Self::Linear),
            "sqrt" => Ok(Self::Sqrt),
            "db_floor" => Ok(Self::DbFloor),
            other => Err(VisFxError::InvalidSpec(format!("mapper {other:?}"))),
        }
    }
}

/// stateful mapper: attack/release smoothing (independent constants
/// for rise/fall — the standard reactive-vis feel) + gain.
#[derive(Debug, Clone)]
pub struct ParamMapper {
    pub kind: MapperKind,
    pub gain: f32,
    pub attack: f32,
    pub release: f32,
    state: f32,
}

impl ParamMapper {
    pub fn new(kind: MapperKind, gain: f32, attack: f32, release: f32) -> Result<Self> {
        for (n, v) in [("gain", gain), ("attack", attack), ("release", release)] {
            if !v.is_finite() {
                return Err(VisFxError::InvalidSpec(format!("mapper {n} not finite")));
            }
        }
        if !(0.0..=1.0).contains(&attack) || !(0.0..=1.0).contains(&release) {
            return Err(VisFxError::InvalidSpec(
                "attack/release must be 0..1".into(),
            ));
        }
        Ok(Self {
            kind,
            gain,
            attack,
            release,
            state: 0.0,
        })
    }

    /// Deterministic per-frame step. Output stays 0..1.
    pub fn step(&mut self, x: f32) -> f32 {
        let x = x.clamp(0.0, 1.0);
        let shaped = match self.kind {
            MapperKind::Linear => x,
            MapperKind::Sqrt => x.sqrt(),
            MapperKind::DbFloor => {
                if x <= 0.0 {
                    0.0
                } else {
                    // 20*log10(x): 0dB→1, -60dB→0
                    ((20.0 * x.log10() + 60.0) / 60.0).clamp(0.0, 1.0)
                }
            }
        };
        let target = (shaped * self.gain).clamp(0.0, 1.0);
        let k = if target > self.state {
            self.attack
        } else {
            self.release
        };
        self.state += k * (target - self.state);
        self.state.clamp(0.0, 1.0)
    }

    /// Stateless convenience: map a whole feature vector.
    pub fn map_series(&mut self, xs: &[f32]) -> Vec<f32> {
        xs.iter().map(|&x| self.step(x)).collect()
    }

    pub fn reset(&mut self) {
        self.state = 0.0;
    }
}
