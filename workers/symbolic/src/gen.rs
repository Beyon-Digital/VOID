//! Symbolic continuation model — deterministic interval-Markov generator.
//!
//! Trains a first-order Markov chain on the input region's pitch
//! intervals plus marginal duration/IOI distributions, estimates the key
//! with Krumhansl-Schmuckler profiles, then samples continuations with a
//! seeded xorshift64* stream. Same input + same seed ⇒ byte-identical
//! proposals.
//!
//! Honesty rules baked in:
//! - scores are the model's own sample likelihoods (mean log-prob
//!   exponentiated), never a fabricated confidence;
//! - rationale strings are rendered from measured facts only;
//! - a region with < 2 notes falls back to scale-walk generation and
//!   says so (`fallback` flag + warning), never pretends to have
//!   learned transitions it didn't observe.

use std::collections::BTreeMap;

/// Ticks per quarter note (CONTRACTS.md §1).
pub const TICKS_PER_QUARTER: i64 = 960_000;
/// Duration/IOI quantization classes: whole, half, quarter, 8th, 16th, 32nd, 64th.
pub const GRID_CLASSES: [i64; 7] = [
    4 * TICKS_PER_QUARTER,
    2 * TICKS_PER_QUARTER,
    TICKS_PER_QUARTER,
    TICKS_PER_QUARTER / 2,
    TICKS_PER_QUARTER / 4,
    TICKS_PER_QUARTER / 8,
    TICKS_PER_QUARTER / 16,
];

const INTERVAL_CLAMP: i32 = 24;
const SMOOTH: f64 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteEvent {
    pub pitch: i32,
    pub velocity: i32,
    pub onset: i64,
    pub length: i64,
}

/// Deterministic RNG — xorshift64* seeded via splitmix64.
pub struct Rng(u64);
impl Rng {
    pub fn new(seed: u64) -> Self {
        // splitmix64 finalizer — good diffusion even for seed=0.
        let mut z = seed.wrapping_add(0x9E37_79B9_7F4A_7C15);
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        Self(z ^ (z >> 31))
    }
    fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// Uniform in [0, n) via multiply-shift (Lemire) — no modulo bias
    /// that matters here; deterministic across platforms.
    pub fn below(&mut self, n: u64) -> u64 {
        if n <= 1 {
            return 0;
        }
        ((self.next_u64() as u128 * n as u128) >> 64) as u64
    }
    /// Uniform integer in [lo, hi] inclusive.
    pub fn range(&mut self, lo: i64, hi: i64) -> i64 {
        lo + self.below((hi - lo + 1) as u64) as i64
    }
}

fn nearest_grid(v: i64) -> i64 {
    let mut best = GRID_CLASSES[GRID_CLASSES.len() - 1];
    let mut best_d = i64::MAX;
    for &g in GRID_CLASSES.iter() {
        let d = (v - g).abs();
        // Ties resolve to the coarser class — documented, deterministic.
        if d < best_d {
            best_d = d;
            best = g;
        }
    }
    best
}

/// Krumhansl-Schmuckler key profiles (major, minor).
const KS_MAJOR: [f64; 12] = [
    6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88,
];
const KS_MINOR: [f64; 12] = [
    6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17,
];

fn pearson(a: &[f64; 12], b: &[f64; 12]) -> f64 {
    let ma = a.iter().sum::<f64>() / 12.0;
    let mb = b.iter().sum::<f64>() / 12.0;
    let mut num = 0.0;
    let mut da = 0.0;
    let mut db = 0.0;
    for i in 0..12 {
        let x = a[i] - ma;
        let y = b[i] - mb;
        num += x * y;
        da += x * x;
        db += y * y;
    }
    if da == 0.0 || db == 0.0 {
        return 0.0;
    }
    num / (da.sqrt() * db.sqrt())
}

/// Duration-weighted pitch-class histogram → best key (root 0..11, minor?, correlation).
pub fn estimate_key(notes: &[NoteEvent]) -> (i32, bool, f64) {
    let mut hist = [0.0f64; 12];
    let mut total = 0.0;
    for n in notes {
        let w = (n.length.max(1) as f64).min(4.0 * TICKS_PER_QUARTER as f64);
        hist[((n.pitch % 12) + 12) as usize % 12] += w;
        total += w;
    }
    if total <= 0.0 {
        return (0, false, 0.0);
    }
    for h in hist.iter_mut() {
        *h /= total;
    }
    let mut best = (0, false, f64::NEG_INFINITY);
    for root in 0..12 {
        for minor in [false, true] {
            let profile = if minor { KS_MINOR } else { KS_MAJOR };
            // Rotate the profile so index = pitch-class relative to root.
            let mut rot = [0.0f64; 12];
            for i in 0..12 {
                rot[(i + root as usize) % 12] = profile[i];
            }
            let r = pearson(&hist, &rot);
            if r > best.2 {
                best = (root, minor, r);
            }
        }
    }
    best
}

const MAJOR_STEPS: [i32; 7] = [0, 2, 4, 5, 7, 9, 11];
const MINOR_STEPS: [i32; 7] = [0, 2, 3, 5, 7, 8, 10];

fn in_scale(pitch: i32, root: i32, minor: bool) -> bool {
    let pc = ((pitch - root) % 12 + 12) % 12;
    let steps = if minor { MINOR_STEPS } else { MAJOR_STEPS };
    steps.contains(&pc)
}

fn snap_to_scale(pitch: i32, root: i32, minor: bool, down_first: bool) -> (i32, bool) {
    if in_scale(pitch, root, minor) {
        return (pitch, false);
    }
    for d in 1..6 {
        let up = pitch + d;
        let dn = pitch - d;
        let (first, second) = if down_first { (dn, up) } else { (up, dn) };
        if (0..=127).contains(&first) && in_scale(first, root, minor) {
            return (first, true);
        }
        if (0..=127).contains(&second) && in_scale(second, root, minor) {
            return (second, true);
        }
    }
    (pitch.clamp(0, 127), true)
}

/// The trained model: transition table + marginals + key + summary facts.
pub struct Model {
    /// prev interval → (next interval → count)
    trans: BTreeMap<i32, BTreeMap<i32, f64>>,
    /// union of plausible next intervals (observed ∪ scale/chord steps)
    support: Vec<i32>,
    dur_marginal: BTreeMap<i64, f64>,
    ioi_marginal: BTreeMap<i64, f64>,
    pub grid: i64,
    pub key_root: i32,
    pub key_minor: bool,
    pub key_corr: f64,
    pub dominant_interval: i32,
    pub median_velocity: i32,
    pub last_pitch: i32,
    pub last_interval: i32,
    pub note_count: usize,
    pub fallback: bool,
}

impl Model {
    /// Train on the region's notes (any order; sorted internally).
    /// `hint` supplies a key override ("A minor", "F# major") when the
    /// caller already knows — treated as data, validated, never executed.
    pub fn train(notes: &[NoteEvent], key_hint: Option<&str>) -> Self {
        let mut sorted: Vec<NoteEvent> = notes.to_vec();
        sorted.sort_by(|a, b| (a.onset, a.pitch).cmp(&(b.onset, b.pitch)));
        sorted.dedup();

        let (mut root, mut minor, mut corr) = estimate_key(&sorted);
        if let Some(h) = key_hint {
            if let Some((r, m)) = parse_key_hint(h) {
                root = r;
                minor = m;
                corr = 1.0; // caller-supplied fact, not a measurement
            }
        }

        // Interval sequence over successive notes, then first-order
        // transitions interval[i-1] → interval[i]. Generation continues
        // the chain from `last_interval` — i.e. the model genuinely
        // continues the region rather than restarting its own prefix.
        let intervals: Vec<i32> = sorted
            .windows(2)
            .map(|w| (w[1].pitch - w[0].pitch).clamp(-INTERVAL_CLAMP, INTERVAL_CLAMP))
            .collect();

        let mut trans: BTreeMap<i32, BTreeMap<i32, f64>> = BTreeMap::new();
        let mut dur_marginal: BTreeMap<i64, f64> = BTreeMap::new();
        let mut ioi_marginal: BTreeMap<i64, f64> = BTreeMap::new();
        let mut interval_counts: BTreeMap<i32, u64> = BTreeMap::new();
        let mut grid_counts: BTreeMap<i64, u64> = BTreeMap::new();

        for w in intervals.windows(2) {
            *trans.entry(w[0]).or_default().entry(w[1]).or_insert(0.0) += 1.0;
        }
        for &iv in &intervals {
            *interval_counts.entry(iv).or_insert(0) += 1;
        }
        for w in sorted.windows(2) {
            let ioi = w[1].onset - w[0].onset;
            if ioi > 0 {
                let g = nearest_grid(ioi);
                *ioi_marginal.entry(g).or_insert(0.0) += 1.0;
                *grid_counts.entry(g).or_insert(0) += 1;
            }
        }
        for n in &sorted {
            let g = nearest_grid(n.length.max(1));
            *dur_marginal.entry(g).or_insert(0.0) += 1.0;
            *grid_counts.entry(g).or_insert(0) += 1;
        }

        // Support: observed intervals ∪ diatonic steps ∪ chord tones —
        // smoothed so unseen-but-plausible moves remain reachable.
        let mut support: Vec<i32> = interval_counts.keys().cloned().collect();
        for extra in [-7, -5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 5, 7, 12, -12] {
            if !support.contains(&extra) {
                support.push(extra);
            }
        }
        support.sort_unstable();

        let dominant_interval = interval_counts
            .iter()
            .max_by(|a, b| a.1.cmp(b.1).then(a.0.cmp(b.0)))
            .map(|(k, _)| *k)
            .unwrap_or(0);

        let grid = grid_counts
            .iter()
            .min_by(|a, b| b.1.cmp(a.1).then(b.0.cmp(a.0)))
            .map(|(g, _)| *g)
            .unwrap_or(TICKS_PER_QUARTER / 4);

        let mut velocities: Vec<i32> = sorted.iter().map(|n| n.velocity).collect();
        velocities.sort_unstable();
        let median_velocity = velocities
            .get(velocities.len() / 2)
            .copied()
            .unwrap_or(90)
            .clamp(1, 127);

        let last_pitch = sorted.last().map(|n| n.pitch).unwrap_or(60);
        let last_interval = if sorted.len() >= 2 {
            (sorted[sorted.len() - 1].pitch - sorted[sorted.len() - 2].pitch)
                .clamp(-INTERVAL_CLAMP, INTERVAL_CLAMP)
        } else {
            0
        };

        // Defaults for sparse data: uniform-ish marginals on the detected
        // grid (fallback path also uses these).
        if dur_marginal.is_empty() {
            for g in [TICKS_PER_QUARTER / 2, TICKS_PER_QUARTER] {
                dur_marginal.insert(g, 1.0);
            }
        }
        if ioi_marginal.is_empty() {
            for g in [TICKS_PER_QUARTER / 2, TICKS_PER_QUARTER] {
                ioi_marginal.insert(g, 1.0);
            }
        }

        Self {
            trans,
            support,
            dur_marginal,
            ioi_marginal,
            grid,
            key_root: root,
            key_minor: minor,
            key_corr: corr,
            dominant_interval,
            median_velocity,
            last_pitch,
            last_interval,
            note_count: sorted.len(),
            fallback: sorted.len() < 2,
        }
    }

    /// P(next interval | prev) with add-alpha smoothing over `support`.
    fn interval_prob(&self, prev: i32, next: i32) -> f64 {
        let row = self.trans.get(&prev.clamp(-INTERVAL_CLAMP, INTERVAL_CLAMP));
        let obs = row.and_then(|r| r.get(&next)).copied().unwrap_or(0.0);
        let total: f64 = row.map(|r| r.values().sum()).unwrap_or(0.0);
        (obs + SMOOTH) / (total + SMOOTH * self.support.len() as f64)
    }

    fn marginal_prob(map: &BTreeMap<i64, f64>, key: i64) -> f64 {
        let total: f64 = map.values().sum();
        if total <= 0.0 {
            return 1.0 / map.len().max(1) as f64;
        }
        let obs = map.get(&key).copied().unwrap_or(0.0);
        (obs + SMOOTH) / (total + SMOOTH * (map.len() + 1) as f64)
    }

    fn sample_weighted(rng: &mut Rng, probs: &[(i64, f64)]) -> i64 {
        let total: f64 = probs.iter().map(|(_, p)| p).sum();
        if total <= 0.0 {
            return probs[0].0;
        }
        let mut pick = (rng.below(1_000_000) as f64 / 1_000_000.0) * total;
        for (v, p) in probs {
            pick -= p;
            if pick <= 0.0 {
                return *v;
            }
        }
        probs[probs.len() - 1].0
    }

    /// One continuation: notes inside [start, start+span), score +
    /// measured facts for the rationale. Deterministic for a given rng.
    pub fn generate(
        &self,
        rng: &mut Rng,
        start: i64,
        span: i64,
        max_notes: usize,
    ) -> (Vec<NoteEvent>, f64, GenFacts) {
        let end = start + span;
        let mut notes = Vec::new();
        let mut log_prob = 0.0;
        let mut choices = 0u32;
        let mut snapped = 0u32;
        let mut pos = start;
        let mut prev_iv = self.last_interval;
        let mut pitch = self.last_pitch;

        while pos < end && notes.len() < max_notes {
            // interval → raw next pitch
            let iv = self.sample_interval(rng, prev_iv);
            log_prob += self.interval_prob(prev_iv, iv).ln();
            choices += 1;
            let raw = pitch + iv;
            let down_first = rng.below(2) == 0;
            let (mut p, was_snapped) =
                snap_to_scale(raw, self.key_root, self.key_minor, down_first);
            if !(0..=127).contains(&p) {
                p = p.clamp(0, 127);
            }
            if was_snapped {
                snapped += 1;
                log_prob += 0.85f64.ln();
            }
            pitch = p;
            prev_iv = iv.clamp(-INTERVAL_CLAMP, INTERVAL_CLAMP);

            let dur = Self::sample_weighted(rng, &self.dur_probs());
            log_prob += Self::marginal_prob(&self.dur_marginal, dur).ln();
            choices += 1;

            let vel = (self.median_velocity as i64 + rng.range(-8, 8)).clamp(1, 127) as i32;
            let length = dur.min(end - pos).max(1);
            notes.push(NoteEvent {
                pitch,
                velocity: vel,
                onset: pos,
                length,
            });

            let ioi = Self::sample_weighted(rng, &self.ioi_probs());
            log_prob += Self::marginal_prob(&self.ioi_marginal, ioi).ln();
            choices += 1;
            pos += ioi.max(self.grid / 4);
        }

        let score = if choices == 0 {
            0.0
        } else {
            (log_prob / choices as f64).exp()
        };
        (
            notes,
            score,
            GenFacts {
                snapped,
                choices,
                fallback: self.fallback,
            },
        )
    }

    /// Scale-walk fallback for < 2 notes — still seeded/deterministic,
    /// honestly labeled.
    pub fn generate_fallback(
        &self,
        rng: &mut Rng,
        start: i64,
        span: i64,
        max_notes: usize,
    ) -> (Vec<NoteEvent>, f64, GenFacts) {
        let end = start + span;
        let steps = if self.key_minor {
            MINOR_STEPS
        } else {
            MAJOR_STEPS
        };
        let base = self.last_pitch.clamp(24, 96);
        let mut notes = Vec::new();
        let mut pos = start;
        let mut degree: i64 = rng.below(7) as i64;
        let mut direction: i64 = if rng.below(2) == 0 { 1 } else { -1 };
        let mut choices = 0u32;
        let mut log_prob = 0.0;
        while pos < end && notes.len() < max_notes {
            if rng.below(4) == 0 {
                direction = -direction;
                log_prob += 0.25f64.ln();
            } else {
                log_prob += 0.75f64.ln();
            }
            choices += 1;
            degree = (degree + direction).rem_euclid(7);
            let octave = (base as i64) / 12 + degree / 7;
            let pitch = (self.key_root as i64 + steps[degree as usize % 7] as i64 + 12 * octave)
                .clamp(0, 127) as i32;
            let dur = Self::sample_weighted(rng, &self.dur_probs());
            log_prob += Self::marginal_prob(&self.dur_marginal, dur).ln();
            choices += 1;
            notes.push(NoteEvent {
                pitch,
                velocity: self.median_velocity,
                onset: pos,
                length: dur.min(end - pos).max(1),
            });
            let ioi = Self::sample_weighted(rng, &self.ioi_probs());
            log_prob += Self::marginal_prob(&self.ioi_marginal, ioi).ln();
            choices += 1;
            pos += ioi.max(self.grid / 4);
        }
        let score = if choices == 0 {
            0.0
        } else {
            (log_prob / choices as f64).exp()
        };
        (
            notes,
            score * 0.5, // honest: fallback ranks below trained continuations
            GenFacts {
                snapped: 0,
                choices,
                fallback: true,
            },
        )
    }

    fn sample_interval(&self, rng: &mut Rng, prev: i32) -> i32 {
        let probs: Vec<(i64, f64)> = self
            .support
            .iter()
            .map(|&iv| (iv as i64, self.interval_prob(prev, iv)))
            .collect();
        Self::sample_weighted(rng, &probs) as i32
    }

    fn dur_probs(&self) -> Vec<(i64, f64)> {
        self.dur_marginal
            .iter()
            .map(|(k, _)| (*k, Self::marginal_prob(&self.dur_marginal, *k)))
            .collect()
    }

    fn ioi_probs(&self) -> Vec<(i64, f64)> {
        self.ioi_marginal
            .iter()
            .map(|(k, _)| (*k, Self::marginal_prob(&self.ioi_marginal, *k)))
            .collect()
    }
}

pub struct GenFacts {
    pub snapped: u32,
    pub choices: u32,
    pub fallback: bool,
}

/// "C", "F# minor", "Bb major" — tolerant parse, inert data.
pub fn parse_key_hint(h: &str) -> Option<(i32, bool)> {
    let t = h.trim().to_ascii_lowercase();
    if t.is_empty() {
        return None;
    }
    let mut it = t.split_whitespace();
    let name = it.next()?;
    let root = match name.as_bytes() {
        [b'c'] => 0,
        [b'c', b'#'] | [b'd', b'b'] => 1,
        [b'd'] => 2,
        [b'd', b'#'] | [b'e', b'b'] => 3,
        [b'e'] => 4,
        [b'f'] => 5,
        [b'f', b'#'] | [b'g', b'b'] => 6,
        [b'g'] => 7,
        [b'g', b'#'] | [b'a', b'b'] => 8,
        [b'a'] => 9,
        [b'a', b'#'] | [b'b', b'b'] => 10,
        [b'b'] => 11,
        _ => return None,
    };
    let minor = it.next().map(|w| w.starts_with('m')).unwrap_or(false);
    Some((root, minor))
}

pub const KEY_NAMES: [&str; 12] = [
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];
