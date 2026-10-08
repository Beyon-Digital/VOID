//! Accompaniment engine (W21 T78; INTEL-01/03/05, DOC-04, PRO-05).
//!
//! Role-scoped generators (drums / bass / keys / synth) turn a
//! `GenerationSpec` into real `GenNote` events: pitch material is drawn
//! from the declared scale + chord map, rhythm from the groove template,
//! and every decision through `XorShift128` — the recorded `seed`
//! reproduces the take bit-for-bit.
//!
//! Invariants enforced here, not just tested:
//!   * notes are generated ONLY inside `spec.output_range` — proposals
//!     never extend into locked/source regions;
//!   * `postcondition_check` asserts the locked source region's bytes
//!     are identical before/after an apply (caller supplies snapshots);
//!   * inpaint/continue are bounded generation problems with the
//!     existing-region edge material passed in as conditioning, never
//!     as mutable targets.

use crate::error::{ProducerError, Result};
use crate::music::{chord_at, ChordEvent, GenNote, GrooveTemplate, Range, Scale, TICKS_PER_QUARTER};
use crate::rng::XorShift128;
use serde::{Deserialize, Serialize};

pub const MAX_CANDIDATES: usize = 8;
pub const MAX_RANGE_TICKS: i64 = TICKS_PER_QUARTER * 4 * 512; // 512 bars
pub const DEFAULT_VELOCITY: i32 = 88;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Drums,
    Bass,
    Keys,
    Synth,
}

impl Role {
    pub const ALL: [Role; 4] = [Role::Drums, Role::Bass, Role::Keys, Role::Synth];
    /// Nominal register window [lo, hi] for pitched roles.
    pub fn register(&self) -> (i32, i32) {
        match self {
            Role::Drums => (35, 60),
            Role::Bass => (28, 52),
            Role::Keys => (48, 76),
            Role::Synth => (60, 84),
        }
    }
}

/// Hand-authored drum kit lanes (General MIDI pitches).
pub const DRUM_KICK: i32 = 36;
pub const DRUM_SNARE: i32 = 38;
pub const DRUM_HAT_CLOSED: i32 = 42;
pub const DRUM_HAT_OPEN: i32 = 46;
pub const DRUM_CRASH: i32 = 49;

/// What kind of generation the candidate performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GenMode {
    /// Fresh material in `output_range` from seed + spec.
    Accompaniment,
    /// Fill a bounded gap: notes end at/inside `output_range`; edge
    /// conditioning comes from `context_notes` inside the region.
    Inpaint,
    /// Pick up from the trailing edge: onset must be inside the
    /// continuation window `output_range`; the window follows the
    /// region that held `context_notes`.
    Continue,
    /// New take of an existing proposal: densify/sparsify within
    /// `variation_amount` of `context_notes`' density.
    Vary,
}

/// Generation request in integer domain (parsed from the DTO/request
/// layer, which keeps string ticks at the boundary).
#[derive(Debug, Clone)]
pub struct GenerationSpec {
    pub seed: u64,
    pub role: Role,
    pub mode: GenMode,
    /// Candidate count 1..=8 (CONTRACTS §6 proposal cap).
    pub candidates: usize,
    /// Half-open output window in ticks — every generated note must
    /// satisfy `output_range.contains(onset)` and end ≤ `range.end`.
    pub output_range: Range,
    /// Locked region material (the protected source). For Inpaint the
    /// *surrounding* region context notes; for Continue the region the
    /// window continues; for Vary the proposal's previous take.
    /// NEVER mutated — used read-only as conditioning.
    pub context_notes: Vec<GenNote>,
    /// Regions inside which no generated note may begin or overlap —
    /// the protected source extents (region locked_ranges plus the
    /// region itself for pure accompaniment mode).
    pub locked_ranges: Vec<Range>,
    pub scale: Scale,
    /// Piecewise chord map for harmonic material; empty = scale only.
    pub chords: Vec<ChordEvent>,
    pub groove: GrooveTemplate,
    /// Note density, parts-per-million of grid steps carrying an onset
    /// (drums: per-lane occupancy; pitched: phrase occupancy).
    pub density_ppm: u32,
    /// 0..=1_000_000: how far Vary may drift (tick + pitch shift scale).
    pub variation_ppm: u32,
    pub tempo_bpm: u32,
}

impl GenerationSpec {
    pub fn validate(&self) -> Result<()> {
        if !(1..=MAX_CANDIDATES).contains(&self.candidates) {
            return Err(ProducerError::InvalidSpec(format!(
                "candidates {} (max {MAX_CANDIDATES})",
                self.candidates
            )));
        }
        if self.output_range.length > MAX_RANGE_TICKS {
            return Err(ProducerError::InvalidSpec(format!(
                "range {} exceeds max {MAX_RANGE_TICKS}",
                self.output_range.length
            )));
        }
        if self.density_ppm > 1_000_000 || self.variation_ppm > 1_000_000 {
            return Err(ProducerError::InvalidSpec("ppm out of range".into()));
        }
        if self.tempo_bpm == 0 || self.tempo_bpm > 999 {
            return Err(ProducerError::InvalidSpec("tempo_bpm out of range".into()));
        }
        // Sorted, non-overlapping chord map.
        for w in self.chords.windows(2) {
            if w[0].at_ticks >= w[1].at_ticks {
                return Err(ProducerError::InvalidSpec(
                    "chord map must be sorted and unique".into(),
                ));
            }
        }
        Ok(())
    }

    /// Sibling seed for candidate `k` — same spec, divergent stream.
    pub fn candidate_seed(&self, k: usize) -> u64 {
        self.seed ^ (k as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
    }
}

/// One generated take plus its provenance (T78: measure usefulness —
/// every candidate reports its own density/hits so the UI can order
/// "most useful first" without re-measuring).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedCandidate {
    pub role: Role,
    pub mode: GenMode,
    pub seed: u64,
    pub notes: Vec<GenNote>,
    /// Occupied grid steps / total grid steps in output_range (ppm).
    pub occupancy_ppm: u64,
    /// Chord-tone onsets / total onsets (ppm) — harmonic fit metric.
    pub chord_tone_ppm: u64,
}

/// Fit metrics shared across roles.
fn measure(notes: &[GenNote], spec: &GenerationSpec) -> (u64, u64) {
    let total_steps = (spec.output_range.length / spec.groove.step_ticks().max(1)).max(1) as u64;
    let mut occupied = std::collections::HashSet::new();
    let mut chord_tones = 0usize;
    for n in notes {
        occupied.insert((n.onset - spec.output_range.start) / spec.groove.step_ticks().max(1));
        if let Some(ch) = chord_at(&spec.chords, n.onset) {
            if ch.pitch_classes().contains(&(n.pitch.rem_euclid(12) as u8)) {
                chord_tones += 1;
            }
        } else if spec.scale.contains(n.pitch) {
            chord_tones += 1; // scale-tone counts as fit when no chord map
        }
    }
    let occ = occupied.len() as u64 * 1_000_000 / total_steps;
    let ct = if notes.is_empty() {
        0
    } else {
        chord_tones as u64 * 1_000_000 / notes.len() as u64
    };
    (occ, ct)
}

/// Hard invariant — every note onsets inside output_range and ends by
/// range.end; no note may overlap a locked range. Callers assert the
/// caller-side byte-identity of the source separately (postcondition).
fn enforce_bounds(notes: &mut Vec<GenNote>, spec: &GenerationSpec) -> Result<()> {
    let r = spec.output_range;
    notes.retain(|n| {
        r.contains_tick(n.onset)
            && n.end() <= r.end()
            && !spec
                .locked_ranges
                .iter()
                .any(|l| l.overlaps(n.onset, n.end()))
    });
    // Defensive clamp: note that still straddles the end is shortened,
    // never moved into a locked span.
    for n in notes.iter_mut() {
        if n.end() > r.end() {
            n.length = r.end() - n.onset;
        }
    }
    notes.retain(|n| n.length > 0);
    Ok(())
}

/// Byte-identity postcondition for T78 (INTEL-01): the source-region
/// bytes the caller snapshotted before the apply must equal the bytes
/// after. Compared as bytes, not vectors — serialization is canonical.
pub fn postcondition_check(before_bytes: &[u8], after_bytes: &[u8]) -> Result<()> {
    if before_bytes != after_bytes {
        return Err(ProducerError::LockedViolation(
            "source region material changed across apply".into(),
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------
// Role generators — real note material, not placeholders.
// ---------------------------------------------------------------------

/// GM drum part: kick/snare/hat skeleton on the groove grid with
/// density-driven fills; crash on section head. Deterministic per seed.
fn gen_drums(spec: &GenerationSpec, rng: &mut XorShift128) -> Result<Vec<GenNote>> {
    let r = spec.output_range;
    let step = spec.groove.step_ticks();
    let bar = TICKS_PER_QUARTER * 4;
    let spb = spec.groove.steps_per_beat as i64;
    let mut notes = Vec::new();
    let hat_density = spec.density_ppm.clamp(80_000, 950_000);
    let mut step_index = 0u64;
    let mut t = r.start - r.start.rem_euclid(step);
    if t < r.start {
        t += step;
    }
    while t < r.end() {
        let phase = (t - r.start).rem_euclid(bar);
        let in_bar_step = phase / step;
        let beat_step = in_bar_step % spb;
        let tt = t + spec.groove.offset_at(step_index);
        if r.contains_tick(tt) {
            // Kick: beats 1 & 3 (+ optional "and of 2" syncopation).
            if beat_step == 0 && (in_bar_step == 0 || in_bar_step == 2 * spb) {
                notes.push(GenNote::new(DRUM_KICK, 96 + spec.groove.accent_at(step_index), tt, step / 2)?);
            }
            if beat_step == spb - 1 && in_bar_step == spb && rng.chance(spec.density_ppm / 3) {
                notes.push(GenNote::new(DRUM_KICK, 84, tt, step / 2)?);
            }
            // Snare on 2 & 4; ghost notes at low prob.
            if in_bar_step == spb || in_bar_step == 3 * spb {
                notes.push(GenNote::new(DRUM_SNARE, 100, tt, step / 2)?);
            } else if in_bar_step % 2 == 1 && rng.chance(spec.density_ppm / 8) {
                notes.push(GenNote::new(DRUM_SNARE, 52, tt, step / 4)?);
            }
            // Hat: density-gated grid occupancy; occasional open hat.
            if rng.chance(hat_density) {
                let open = beat_step == spb - 1 && rng.chance(120_000);
                let vel = 64 + spec.groove.accent_at(step_index) + rng.below(12) as i32;
                notes.push(GenNote::new(
                    if open { DRUM_HAT_OPEN } else { DRUM_HAT_CLOSED },
                    vel.clamp(1, 127),
                    tt,
                    if open { step * 2 } else { step / 2 },
                )?);
            }
            // Crash on each 4-bar phrase head.
            if phase == 0 && (t - r.start) / bar % 4 == 0 {
                notes.push(GenNote::new(DRUM_CRASH, 108, tt, step * 2)?);
            }
        }
        t += step;
        step_index += 1;
    }
    Ok(notes)
}

/// Bass: chord roots on downbeats, fifths/approach tones between,
/// gated by density. Pitch material from the chord map/scale only.
fn gen_bass(spec: &GenerationSpec, rng: &mut XorShift128) -> Result<Vec<GenNote>> {
    let (lo, hi) = spec.role.register();
    let r = spec.output_range;
    let step = spec.groove.step_ticks();
    let spb = spec.groove.steps_per_beat as i64;
    let mut notes = Vec::new();
    let mut step_index = 0u64;
    let mut t = r.start - r.start.rem_euclid(step);
    if t < r.start {
        t += step;
    }
    while t < r.end() {
        let tt = t + spec.groove.offset_at(step_index);
        let in_bar_step = (t - r.start).rem_euclid(TICKS_PER_QUARTER * 4) / step;
        let downbeat = in_bar_step % spb == 0;
        if r.contains_tick(tt) && (downbeat || rng.chance(spec.density_ppm / 2)) {
            let ch = chord_at(&spec.chords, tt);
            let pc = ch.map(|c| c.root_pc).unwrap_or(spec.scale.root_pc);
            // Register-snapped chord root.
            let mut pitch = (lo + 12 - lo % 12) + pc as i32;
            while pitch > hi - 5 {
                pitch -= 12;
            }
            while pitch < lo {
                pitch += 12;
            }
            // Non-downbeats: fifth or approach step.
            if !downbeat {
                let choice = rng.below(3);
                if choice == 0 {
                    pitch += 7; // fifth
                } else {
                    let deg = spec.scale.nearest_degree(pitch);
                    pitch = spec.scale.degree_pitch(pitch.div_euclid(12) - 1, deg);
                    if choice == 1 {
                        pitch += if rng.chance(500_000) { 2 } else { -2 };
                        pitch = spec.scale.nearest_pitch(pitch);
                    }
                }
            }
            let vel = (78 + spec.groove.accent_at(step_index) + rng.below(16) as i32).clamp(1, 127);
            notes.push(GenNote::new(pitch.clamp(0, 127), vel, tt, step)?);
        }
        t += step;
        step_index += 1;
    }
    Ok(notes)
}

/// Keys: rhythmic comping — chord voicings (3-4 voices) stabbing on
/// offbeats/syncopations chosen by density; voice-lead toward the
/// register centre.
fn gen_keys(spec: &GenerationSpec, rng: &mut XorShift128) -> Result<Vec<GenNote>> {
    let (lo, hi) = spec.role.register();
    let r = spec.output_range;
    let step = spec.groove.step_ticks();
    let spb = spec.groove.steps_per_beat as i64;
    let mut notes = Vec::new();
    let mut step_index = 0u64;
    let mut t = r.start - r.start.rem_euclid(step);
    if t < r.start {
        t += step;
    }
    while t < r.end() {
        let tt = t + spec.groove.offset_at(step_index);
        let in_bar_step = (t - r.start).rem_euclid(TICKS_PER_QUARTER * 4) / step;
        // Comp on "and" positions and bar heads at density.
        let comp_pos = in_bar_step % 2 == 1 || in_bar_step == 0 || in_bar_step == 2 * spb + 1;
        if r.contains_tick(tt) && comp_pos && rng.chance(spec.density_ppm) {
            if let Some(ch) = chord_at(&spec.chords, tt) {
                let pcs = ch.pitch_classes();
                let centre = (lo + hi) / 2;
                for (vi, pc) in pcs.iter().enumerate() {
                    let mut p = centre - centre % 12 + *pc as i32;
                    while p > hi {
                        p -= 12;
                    }
                    while p < lo {
                        p += 12;
                    }
                    let vel = (72 - vi as i32 * 4 + rng.below(10) as i32).clamp(1, 127);
                    notes.push(GenNote::new(p, vel, tt, step * 2)?);
                }
            } else {
                // No chord map: stacked scale triad on the tonic.
                for d in [0usize, 2, 4] {
                    let p = spec.scale.degree_pitch(4, d).clamp(lo, hi);
                    notes.push(GenNote::new(p, 72, tt, step * 2)?);
                }
            }
        }
        t += step;
        step_index += 1;
    }
    Ok(notes)
}

/// Synth lead: a stepwise melody over the scale with chord-tone
/// gravity — each onset picks a chord tone or scale neighbour, gated
/// by density; phrase lengths 1-2 steps.
fn gen_synth(spec: &GenerationSpec, rng: &mut XorShift128) -> Result<Vec<GenNote>> {
    let (lo, hi) = spec.role.register();
    let r = spec.output_range;
    let step = spec.groove.step_ticks();
    let mut notes = Vec::new();
    let mut step_index = 0u64;
    let mut last = spec.scale.nearest_pitch((lo + hi) / 2);
    let mut t = r.start - r.start.rem_euclid(step);
    if t < r.start {
        t += step;
    }
    while t < r.end() {
        let tt = t + spec.groove.offset_at(step_index);
        if r.contains_tick(tt) && rng.chance(spec.density_ppm / 2) {
            let chord_pcs = chord_at(&spec.chords, tt).map(|c| c.pitch_classes());
            let mut next = last;
            match rng.below(4) {
                0 => {
                    // Stepwise motion within the scale.
                    let dir = if rng.chance(500_000) { 1 } else { -1 };
                    next = spec.scale.nearest_pitch(next + dir * 2);
                }
                1 => {
                    // Arpeggiate the active chord.
                    if let Some(pcs) = &chord_pcs {
                        let pc = pcs[rng.below(pcs.len() as u64) as usize] as i32;
                        next = spec.scale.nearest_pitch((next + 12 - next % 12) + pc);
                    } else {
                        next = spec.scale.nearest_pitch(next + 7);
                    }
                }
                2 => {
                    // Leap to a chord tone.
                    if let Some(pcs) = &chord_pcs {
                        let pc = pcs[rng.below(pcs.len() as u64) as usize] as i32;
                        next = (lo + 12 - lo % 12) + pc + 12;
                        while next > hi {
                            next -= 12;
                        }
                    }
                }
                _ => {
                    // Octave displacement.
                    next += if rng.chance(500_000) { 12 } else { -12 };
                }
            }
            next = spec.scale.nearest_pitch(next.clamp(lo, hi));
            let len = step * (1 + rng.below(2) as i64);
            let vel = (80 + spec.groove.accent_at(step_index) + rng.below(12) as i32).clamp(1, 127);
            notes.push(GenNote::new(next, vel, tt, len)?);
            last = next;
        }
        t += step;
        step_index += 1;
    }
    Ok(notes)
}

// ---------------------------------------------------------------------
// Mode drivers
// ---------------------------------------------------------------------

/// Accompaniment / inpaint: same role engine — inpaint differs only in
/// that `output_range` is the bounded gap and `context_notes` conditions
/// the *edges*: the first onset preferentially continues the last
/// context pitch-class motion. Implemented as edge-conditioned start
/// state rather than a different note source.
fn gen_mode_notes(spec: &GenerationSpec, rng: &mut XorShift128) -> Result<Vec<GenNote>> {
    match spec.role {
        Role::Drums => gen_drums(spec, rng),
        Role::Bass => gen_bass(spec, rng),
        Role::Keys => gen_keys(spec, rng),
        Role::Synth => gen_synth(spec, rng),
    }
}

/// Inpaint/Continue edge conditioning: rotate the candidate so its
/// opening gesture hands off from the context's trailing material —
/// first note snaps to a scale step continuing the last context pitch
/// direction. Bounded: still inside output_range.
fn edge_condition(notes: &mut [GenNote], spec: &GenerationSpec, at_tail: bool) {
    if notes.is_empty() || spec.context_notes.is_empty() || spec.role == Role::Drums {
        return;
    }
    let ctx: Vec<&GenNote> = spec
        .context_notes
        .iter()
        .filter(|n| n.onset < spec.output_range.start)
        .collect();
    let Some(&last_ctx) = ctx.iter().max_by_key(|n| n.onset) else {
        return;
    };
    // Direction of the last two context notes biases the opening.
    let direction = if ctx.len() >= 2 {
        let mut sorted = ctx.clone();
        sorted.sort_by_key(|n| n.onset);
        (sorted[sorted.len() - 1].pitch - sorted[sorted.len() - 2].pitch).signum()
    } else {
        0
    };
    let first = &mut notes[0];
    let target = if at_tail {
        spec.scale.nearest_pitch(last_ctx.pitch + if direction >= 0 { 2 } else { -2 })
    } else {
        spec.scale.nearest_pitch(last_ctx.pitch + direction.max(-2).min(2))
    };
    let (lo, hi) = spec.role.register();
    first.pitch = target.clamp(lo, hi);
}

/// Vary: re-voice `context_notes` (the previous take) — keep the onset
/// grid, jitter timing within variation bounds, move pitches to
/// neighbouring scale tones, and swap a fraction of onsets by density.
/// Source notes are inputs; the output is a NEW note list.
fn gen_vary(spec: &GenerationSpec, rng: &mut XorShift128) -> Result<Vec<GenNote>> {
    let amount = spec.variation_ppm.max(50_000);
    let timing_span = (spec.groove.step_ticks() as u128 * amount as u128 / 1_000_000 / 2) as i64;
    let mut out = Vec::with_capacity(spec.context_notes.len());
    let mut prev_pitch: Option<i32> = None;
    for src in &spec.context_notes {
        // Drop a bounded fraction of onsets → "sparsify" variant.
        if rng.chance(amount / 4) {
            continue;
        }
        let onset = (src.onset + rng.jitter(timing_span)).max(spec.output_range.start);
        let mut pitch = src.pitch;
        if rng.chance(amount / 2) && spec.role != Role::Drums {
            let step = if rng.chance(500_000) { 2 } else { -2 };
            pitch = spec.scale.nearest_pitch(pitch + step);
            if let Some(pp) = prev_pitch {
                // Keep melodic intervals singable: cap at a fifth.
                if (pitch - pp).abs() > 7 {
                    pitch = spec.scale.nearest_pitch(pp + step.signum() * 7);
                }
            }
        }
        prev_pitch = Some(pitch);
        let vel = (src.velocity + rng.jitter(12) as i32).clamp(1, 127);
        out.push(GenNote::new(
            pitch,
            vel,
            onset,
            (src.length + rng.jitter(timing_span / 2)).max(spec.groove.step_ticks() / 4),
        )?);
    }
    Ok(out)
}

/// Public entry: generate `spec.candidates` candidates. Deterministic —
/// same spec JSON ⇒ same output bytes.
pub fn generate(spec: &GenerationSpec) -> Result<Vec<GeneratedCandidate>> {
    spec.validate()?;
    let mut out = Vec::with_capacity(spec.candidates);
    for k in 0..spec.candidates {
        let mut rng = XorShift128::new(spec.candidate_seed(k));
        let mut notes = match spec.mode {
            GenMode::Vary => gen_vary(spec, &mut rng)?,
            _ => {
                let mut n = gen_mode_notes(spec, &mut rng)?;
                match spec.mode {
                    GenMode::Inpaint => edge_condition(&mut n, spec, false),
                    GenMode::Continue => edge_condition(&mut n, spec, true),
                    _ => {}
                }
                n
            }
        };
        notes.sort_by_key(|n| (n.onset, n.pitch));
        notes.dedup_by(|a, b| a.onset == b.onset && a.pitch == b.pitch);
        enforce_bounds(&mut notes, spec)?;
        let (occ, ct) = measure(&notes, spec);
        out.push(GeneratedCandidate {
            role: spec.role,
            mode: spec.mode,
            seed: spec.candidate_seed(k),
            notes,
            occupancy_ppm: occ,
            chord_tone_ppm: ct,
        });
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::music::{ChordQuality, ScaleKind};

    fn spec(role: Role, mode: GenMode) -> GenerationSpec {
        GenerationSpec {
            seed: 0x5EED,
            role,
            mode,
            candidates: 3,
            output_range: Range::new(0, TICKS_PER_QUARTER * 16).unwrap(),
            context_notes: vec![],
            locked_ranges: vec![],
            scale: Scale::new(0, ScaleKind::Major).unwrap(),
            chords: vec![
                ChordEvent { at_ticks: 0, root_pc: 0, quality: ChordQuality::Major },
                ChordEvent {
                    at_ticks: 4 * TICKS_PER_QUARTER,
                    root_pc: 5,
                    quality: ChordQuality::Major,
                },
                ChordEvent {
                    at_ticks: 8 * TICKS_PER_QUARTER,
                    root_pc: 7,
                    quality: ChordQuality::Dominant7,
                },
            ],
            groove: GrooveTemplate::straight(),
            density_ppm: 450_000,
            variation_ppm: 300_000,
            tempo_bpm: 120,
        }
    }

    #[test]
    fn deterministic_same_spec_same_notes() {
        let s = spec(Role::Keys, GenMode::Accompaniment);
        let a = generate(&s).unwrap();
        let b = generate(&s).unwrap();
        assert_eq!(a.len(), 3);
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.notes, y.notes);
            assert_eq!(x.seed, y.seed);
        }
        let mut s2 = spec(Role::Keys, GenMode::Accompaniment);
        s2.seed = 0x5EED + 1;
        let c = generate(&s2).unwrap();
        assert_ne!(a[0].notes, c[0].notes);
    }

    #[test]
    fn pitched_roles_stay_in_scale_or_chord() {
        for role in [Role::Bass, Role::Keys, Role::Synth] {
            let s = spec(role, GenMode::Accompaniment);
            for cand in generate(&s).unwrap() {
                assert!(!cand.notes.is_empty());
                for n in &cand.notes {
                    let pc = n.pitch.rem_euclid(12) as u8;
                    let in_scale = s.scale.contains(n.pitch);
                    let in_chord = chord_at(&s.chords, n.onset)
                        .map(|c| c.pitch_classes().contains(&pc))
                        .unwrap_or(false);
                    assert!(in_scale || in_chord, "role {role:?} pitch {pc}");
                }
            }
        }
    }

    #[test]
    fn drum_part_has_real_kit_content() {
        let s = spec(Role::Drums, GenMode::Accompaniment);
        let cand = &generate(&s).unwrap()[0];
        assert!(cand.notes.len() > 30, "{}", cand.notes.len());
        let pitches: std::collections::HashSet<i32> =
            cand.notes.iter().map(|n| n.pitch).collect();
        assert!(pitches.contains(&DRUM_KICK));
        assert!(pitches.contains(&DRUM_SNARE));
        assert!(pitches.contains(&DRUM_HAT_CLOSED));
        // Snare backbeats on 2 & 4 of bar 1.
        let beat = TICKS_PER_QUARTER;
        assert!(cand
            .notes
            .iter()
            .any(|n| n.pitch == DRUM_SNARE && n.onset == beat));
        assert!(cand
            .notes
            .iter()
            .any(|n| n.pitch == DRUM_SNARE && n.onset == 3 * beat));
    }

    #[test]
    fn bounds_respect_locked_ranges() {
        let mut s = spec(Role::Bass, GenMode::Accompaniment);
        // Lock the middle 4 quarters — no generated note may overlap.
        s.locked_ranges = vec![Range::new(4 * TICKS_PER_QUARTER, 4 * TICKS_PER_QUARTER).unwrap()];
        for cand in generate(&s).unwrap() {
            for n in &cand.notes {
                assert!(!s.locked_ranges[0].overlaps(n.onset, n.end()));
                assert!(s.output_range.contains_tick(n.onset));
                assert!(n.end() <= s.output_range.end());
            }
        }
    }

    #[test]
    fn vary_produces_different_notes_same_grid_family() {
        let mut s = spec(Role::Keys, GenMode::Accompaniment);
        let base = generate(&s).unwrap()[0].notes.clone();
        s.mode = GenMode::Vary;
        s.context_notes = base.clone();
        let varied = &generate(&s).unwrap()[0].notes;
        assert_ne!(*varied, base);
        // Still all inside range, and roughly comparable size.
        assert!(varied.len() > base.len() / 4);
    }

    #[test]
    fn postcondition_detects_mutation() {
        let before = vec![1u8, 2, 3];
        let after = vec![1u8, 2, 3];
        assert!(postcondition_check(&before, &after).is_ok());
        assert!(postcondition_check(&before, &[1, 2, 4]).is_err());
    }
}
