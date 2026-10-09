//! Musical model for the producer lane: scales, chords, grooves and the
//! generated-note domain type. Tick quantization is the protocol's
//! signed 960_000 ticks per quarter (CONTRACTS.md §1); DTO strings ride
//! `void_proposals` NoteEvent/TickRange shapes — no second note DTO.

use crate::error::{ProducerError, Result};
use serde::{Deserialize, Serialize};

/// 960,000 ticks per quarter note (CONTRACTS.md §1).
pub const TICKS_PER_QUARTER: i64 = 960_000;

/// Half-open tick range [start, end).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Range {
    pub start: i64,
    pub length: i64,
}

impl Range {
    pub fn new(start: i64, length: i64) -> Result<Self> {
        if length <= 0 {
            return Err(ProducerError::InvalidSpec(format!(
                "range length must be > 0, got {length}"
            )));
        }
        Ok(Self { start, length })
    }
    pub fn end(&self) -> i64 {
        self.start + self.length
    }
    pub fn contains_tick(&self, t: i64) -> bool {
        t >= self.start && t < self.end()
    }
    /// True when [s, e) intersects this range.
    pub fn overlaps(&self, s: i64, e: i64) -> bool {
        self.start < e && s < self.end()
    }
    /// Distance from tick to the range edge (0 when inside).
    pub fn clamp_inside(&self, t: i64) -> i64 {
        t.clamp(self.start, self.end().saturating_sub(1))
    }
}

/// One generated note — integer domain. `to_dto` produces the wire
/// `NoteEvent` (decimal strings) the proposals/store layers speak.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GenNote {
    pub pitch: i32,
    pub velocity: i32,
    pub onset: i64,
    pub length: i64,
}

impl GenNote {
    pub fn new(pitch: i32, velocity: i32, onset: i64, length: i64) -> Result<Self> {
        if !(0..=127).contains(&pitch) {
            return Err(ProducerError::InvalidSpec(format!("pitch {pitch}")));
        }
        if !(1..=127).contains(&velocity) {
            return Err(ProducerError::InvalidSpec(format!("velocity {velocity}")));
        }
        if length <= 0 {
            return Err(ProducerError::InvalidSpec(format!("length {length}")));
        }
        Ok(Self {
            pitch,
            velocity,
            onset,
            length,
        })
    }
    pub fn end(&self) -> i64 {
        self.onset + self.length
    }
    pub fn overlaps(&self, s: i64, e: i64) -> bool {
        self.onset < e && s < self.end()
    }
    pub fn to_dto(&self) -> void_proposals::NoteEvent {
        void_proposals::NoteEvent {
            pitch: self.pitch,
            velocity: self.velocity,
            onset_ticks: self.onset.to_string(),
            length_ticks: self.length.to_string(),
        }
    }
    pub fn from_dto(n: &void_proposals::NoteEvent) -> Result<Self> {
        Self::new(
            n.pitch,
            n.velocity,
            n.onset_ticks
                .parse()
                .map_err(|_| ProducerError::MalformedDocument("note onset".into()))?,
            n.length_ticks
                .parse()
                .map_err(|_| ProducerError::MalformedDocument("note length".into()))?,
        )
    }
}

/// Canonical little-endian serialization used for the byte-identity
/// locked-region assertion (T78/INTEL-01). Deterministic field order —
/// equal note lists hash identically.
pub fn serialize_notes(notes: &[GenNote]) -> Vec<u8> {
    let mut v = notes.to_vec();
    v.sort_by_key(|n| (n.onset, n.length, n.pitch, n.velocity));
    let mut out = Vec::with_capacity(v.len() * 24);
    for n in &v {
        out.extend_from_slice(&n.onset.to_le_bytes());
        out.extend_from_slice(&n.length.to_le_bytes());
        out.extend_from_slice(&n.pitch.to_le_bytes());
        out.extend_from_slice(&n.velocity.to_le_bytes());
    }
    out
}

/// Bytes of exactly the notes intersecting `locked` ranges — compared
/// before/after an apply to prove protected material is untouched.
pub fn locked_region_bytes(notes: &[GenNote], locked: &[Range]) -> Vec<u8> {
    let inside: Vec<GenNote> = notes
        .iter()
        .filter(|n| locked.iter().any(|r| r.overlaps(n.onset, n.end())))
        .cloned()
        .collect();
    serialize_notes(&inside)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScaleKind {
    Major,
    NaturalMinor,
    Dorian,
    MinorPentatonic,
    MajorPentatonic,
}

impl ScaleKind {
    /// Semitone offsets within one octave, ascending from the root.
    pub fn degrees(&self) -> &'static [u8] {
        match self {
            Self::Major => &[0, 2, 4, 5, 7, 9, 11],
            Self::NaturalMinor => &[0, 2, 3, 5, 7, 8, 10],
            Self::Dorian => &[0, 2, 3, 5, 7, 9, 10],
            Self::MinorPentatonic => &[0, 3, 5, 7, 10],
            Self::MajorPentatonic => &[0, 2, 4, 7, 9],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Scale {
    /// Pitch class of the tonic, 0-11 (C..B).
    pub root_pc: u8,
    pub kind: ScaleKind,
}

impl Scale {
    pub fn new(root_pc: u8, kind: ScaleKind) -> Result<Self> {
        if root_pc > 11 {
            return Err(ProducerError::InvalidSpec(format!("root pc {root_pc}")));
        }
        Ok(Self { root_pc, kind })
    }
    /// Absolute MIDI pitch-classes this scale contains.
    pub fn pitch_classes(&self) -> Vec<u8> {
        self.kind
            .degrees()
            .iter()
            .map(|d| (self.root_pc + d) % 12)
            .collect()
    }
    pub fn contains(&self, pitch: i32) -> bool {
        let pc = pitch.rem_euclid(12) as u8;
        self.pitch_classes().contains(&pc)
    }
    /// MIDI pitch for `degree` in `octave` (scientific octave; 4 = around
    /// middle C when root is C).
    pub fn degree_pitch(&self, octave: i32, degree: usize) -> i32 {
        let deg = self.kind.degrees();
        let d = deg[degree % deg.len()] as i32;
        let extra_oct = (degree / deg.len()) as i32;
        (octave + extra_oct + 1) * 12 + self.root_pc as i32 + d
    }
    /// Degree index whose pitch is closest to `pitch` (downward tie).
    pub fn nearest_degree(&self, pitch: i32) -> usize {
        let deg = self.kind.degrees().len();
        let mut best = 0usize;
        let mut best_d = i32::MAX;
        for oct in pitch.div_euclid(12) - 2..=pitch.div_euclid(12) {
            for d in 0..deg {
                let p = self.degree_pitch(oct, d);
                let dist = (p - pitch).abs();
                if dist < best_d {
                    best_d = dist;
                    best = d;
                }
            }
        }
        best
    }
    /// Nearest in-scale pitch to `pitch` (ties go down).
    pub fn nearest_pitch(&self, pitch: i32) -> i32 {
        for d in 0..12 {
            for cand in [pitch - d, pitch + d] {
                if self.contains(cand) {
                    return cand;
                }
            }
        }
        pitch
    }
    /// Parse a key hint like "c", "c#", "db", "F#m" (inert text input —
    /// labels are data, never instructions; parse is strict).
    pub fn parse_key(hint: &str, kind: ScaleKind) -> Result<Scale> {
        let h = hint.trim().to_ascii_lowercase();
        let h = h.strip_suffix('m').unwrap_or(&h);
        let pc = match h {
            "c" => 0,
            "c#" | "db" => 1,
            "d" => 2,
            "d#" | "eb" => 3,
            "e" => 4,
            "f" => 5,
            "f#" | "gb" => 6,
            "g" => 7,
            "g#" | "ab" => 8,
            "a" => 9,
            "a#" | "bb" => 10,
            "b" | "cb" => 11,
            _ => return Err(ProducerError::InvalidSpec(format!("key hint {hint:?}"))),
        };
        Self::new(pc, kind)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChordQuality {
    Major,
    Minor,
    Dominant7,
    Major7,
    Minor7,
    Sus4,
}

impl ChordQuality {
    /// Semitone intervals from the chord root.
    pub fn intervals(&self) -> &'static [i32] {
        match self {
            Self::Major => &[0, 4, 7],
            Self::Minor => &[0, 3, 7],
            Self::Dominant7 => &[0, 4, 7, 10],
            Self::Major7 => &[0, 4, 7, 11],
            Self::Minor7 => &[0, 3, 7, 10],
            Self::Sus4 => &[0, 5, 7],
        }
    }
    pub fn symbol(&self) -> &'static str {
        match self {
            Self::Major => "",
            Self::Minor => "m",
            Self::Dominant7 => "7",
            Self::Major7 => "maj7",
            Self::Minor7 => "m7",
            Self::Sus4 => "sus4",
        }
    }
}

/// Piecewise chord map: `at_ticks` sorted ascending; each chord holds
/// until the next event. Chords are literal quality intervals (not
/// scale-snapped) — the caller declares the harmony.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChordEvent {
    pub at_ticks: i64,
    pub root_pc: u8,
    pub quality: ChordQuality,
}

impl ChordEvent {
    /// Pitch-class set of this chord (0-11).
    pub fn pitch_classes(&self) -> Vec<u8> {
        self.quality
            .intervals()
            .iter()
            .map(|i| ((self.root_pc as i32 + i) as u8) % 12)
            .collect()
    }
}

/// Chord active at `tick` (last event at or before it).
pub fn chord_at(chords: &[ChordEvent], tick: i64) -> Option<&ChordEvent> {
    chords.iter().rfind(|c| c.at_ticks <= tick)
}

/// Swing/groove template: a fixed per-step table of timing offsets
/// (ticks) and velocity accents applied on a quantization grid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrooveTemplate {
    pub name: String,
    /// Grid division: steps per beat (quarter). 4 = 16ths, 2 = 8ths.
    pub steps_per_beat: u32,
    /// Positive pushes that step late, by step-index modulo `len`.
    pub timing: Vec<i64>,
    /// Velocity delta applied at each step-index modulo `len`.
    pub accent: Vec<i32>,
}

impl GrooveTemplate {
    /// Straight feel — no timing shifts, mild downbeat accent table.
    pub fn straight() -> Self {
        Self {
            name: "straight".into(),
            steps_per_beat: 4,
            timing: vec![0; 16],
            accent: vec![8, 0, -4, 0, 4, 0, -4, 0, 8, 0, -4, 0, 4, 0, -4, 0],
        }
    }
    /// Classic swing: every second step in a pair is delayed by
    /// `swing_ppm` of a step interval (333_333 ≈ triplet swing).
    pub fn swing(swing_ppm: u32, steps_per_beat: u32) -> Result<Self> {
        if !(1..=8).contains(&steps_per_beat) || swing_ppm > 500_000 {
            return Err(ProducerError::InvalidSpec(
                "swing_ppm/steps_per_beat out of range".into(),
            ));
        }
        let step = TICKS_PER_QUARTER / steps_per_beat as i64;
        let push = (step as u128 * swing_ppm as u128 / 1_000_000) as i64;
        let n = (steps_per_beat * 4) as usize; // one bar
        let mut timing = vec![0i64; n];
        for (i, t) in timing.iter_mut().enumerate() {
            if i % 2 == 1 {
                *t = push;
            }
        }
        let mut accent = vec![0i32; n];
        for i in (0..n).step_by(steps_per_beat as usize) {
            accent[i] = 6;
        }
        Ok(Self {
            name: format!("swing-{swing_ppm}"),
            steps_per_beat,
            timing,
            accent,
        })
    }
    pub fn step_ticks(&self) -> i64 {
        TICKS_PER_QUARTER / self.steps_per_beat as i64
    }
    /// Timing offset (ticks) for the `step_index`-th grid step since 0.
    pub fn offset_at(&self, step_index: u64) -> i64 {
        if self.timing.is_empty() {
            return 0;
        }
        self.timing[step_index as usize % self.timing.len()]
    }
    pub fn accent_at(&self, step_index: u64) -> i32 {
        if self.accent.is_empty() {
            return 0;
        }
        self.accent[step_index as usize % self.accent.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scale_membership_and_snap() {
        let c = Scale::new(0, ScaleKind::Major).unwrap();
        assert!(c.contains(60));
        assert!(!c.contains(61));
        assert_eq!(c.nearest_pitch(61), 60); // tie goes down
        assert_eq!(c.nearest_pitch(62), 62);
        let f_min = Scale::new(5, ScaleKind::NaturalMinor).unwrap();
        assert_eq!(f_min.pitch_classes(), vec![5, 7, 8, 10, 0, 1, 3]);
        assert_eq!(Scale::parse_key("Eb", ScaleKind::Major).unwrap().root_pc, 3);
        assert!(Scale::parse_key("H", ScaleKind::Major).is_err());
    }

    #[test]
    fn degree_pitch_octaves() {
        let c = Scale::new(0, ScaleKind::Major).unwrap();
        assert_eq!(c.degree_pitch(4, 0), 60);
        assert_eq!(c.degree_pitch(4, 2), 64);
        assert_eq!(c.degree_pitch(4, 7), 72); // degree wraps to next octave
    }

    #[test]
    fn chord_map_piecewise() {
        let chords = vec![
            ChordEvent {
                at_ticks: 0,
                root_pc: 0,
                quality: ChordQuality::Major,
            },
            ChordEvent {
                at_ticks: 3_840_000,
                root_pc: 7,
                quality: ChordQuality::Dominant7,
            },
        ];
        assert_eq!(chord_at(&chords, 0).unwrap().root_pc, 0);
        assert_eq!(chord_at(&chords, 3_839_999).unwrap().root_pc, 0);
        assert_eq!(chord_at(&chords, 3_840_000).unwrap().root_pc, 7);
        assert_eq!(
            chord_at(&chords, 3_840_000).unwrap().pitch_classes(),
            vec![7, 11, 2, 5]
        );
    }

    #[test]
    fn swing_pushes_every_other_step() {
        let g = GrooveTemplate::swing(333_333, 4).unwrap();
        assert_eq!(g.offset_at(0), 0);
        assert_eq!(g.offset_at(1), 79_999); // 240_000·0.333333, truncated
        assert_eq!(g.offset_at(2), 0);
        assert_eq!(g.offset_at(3), 79_999);
    }

    #[test]
    fn locked_bytes_only_inside() {
        let notes = vec![
            GenNote::new(60, 90, 0, 240_000).unwrap(),
            GenNote::new(62, 90, 3_840_000, 240_000).unwrap(),
        ];
        let locked = [Range::new(0, 1_000_000).unwrap()];
        let b = locked_region_bytes(&notes, &locked);
        assert_eq!(b.len(), 24); // only the first note
    }
}
