//! Notated pitch — diatonic step + alter + octave.
//!
//! The model stores *spelled* pitches (what the score shows), not raw
//! MIDI numbers — `midi()` derives the sounding pitch. Spelling matters
//! for notation interchange: MusicXML carries step/alter/octave, and a
//! transpose must respell honestly, not leave `<alter>` contradicting
//! `<step>`.

use serde::{Deserialize, Serialize};

/// Diatonic step. `C4` is middle C → MIDI 60; octaves change between
/// B and C (B3→C4), matching the MusicXML convention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    C,
    D,
    E,
    F,
    G,
    A,
    B,
}

impl Step {
    /// Semitones of the natural step within the octave (C=0).
    pub fn natural_semitone(self) -> i32 {
        match self {
            Step::C => 0,
            Step::D => 2,
            Step::E => 4,
            Step::F => 5,
            Step::G => 7,
            Step::A => 9,
            Step::B => 11,
        }
    }
    pub fn letter(self) -> char {
        match self {
            Step::C => 'C',
            Step::D => 'D',
            Step::E => 'E',
            Step::F => 'F',
            Step::G => 'G',
            Step::A => 'A',
            Step::B => 'B',
        }
    }
    pub fn from_letter(c: char) -> Option<Self> {
        match c.to_ascii_uppercase() {
            'C' => Some(Step::C),
            'D' => Some(Step::D),
            'E' => Some(Step::E),
            'F' => Some(Step::F),
            'G' => Some(Step::G),
            'A' => Some(Step::A),
            'B' => Some(Step::B),
            _ => None,
        }
    }
    const ALL: [Step; 7] = [
        Step::C,
        Step::D,
        Step::E,
        Step::F,
        Step::G,
        Step::A,
        Step::B,
    ];
}

/// Spelled pitch. `alter` is semitone displacement from the natural
/// step: -2..=2 covers every practical spelling (Cbb..B##).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pitch {
    pub step: Step,
    /// -2..=2 in normal engraving; wider values validate but round-trip
    /// as-is.
    pub alter: i32,
    /// Scientific octave; middle C = 4. Legal 0..=9 (MusicXML bound).
    pub octave: i32,
}

impl Pitch {
    pub fn midi(&self) -> i32 {
        (self.octave + 1) * 12 + self.step.natural_semitone() + self.alter
    }

    /// Build from step/octave/alter with range validation for callers
    /// that want typed rejection rather than the permissive model bound.
    pub fn checked(step: Step, alter: i32, octave: i32) -> crate::error::Result<Self> {
        if !(-3..=3).contains(&alter) {
            return Err(crate::error::NotationError::Malformed(format!(
                "pitch alter {alter} out of range"
            )));
        }
        if !(0..=9).contains(&octave) {
            return Err(crate::error::NotationError::Malformed(format!(
                "pitch octave {octave} out of 0..=9"
            )));
        }
        Ok(Self {
            step,
            alter,
            octave,
        })
    }

    /// Spell a sounding pitch, preferring the accidental direction of
    /// the key signature (negative fifths → flats, positive → sharps)
    /// and always choosing the spelling with the smallest |alter|.
    ///
    /// Deterministic: identical (midi, fifths) always yields one
    /// spelling. `fifths` is the part's key signature fifths (0 = C
    /// major / none recorded).
    pub fn spell(midi: i32, fifths: i32) -> Self {
        let prefer_sharps = fifths > 0;
        let prefer_flats = fifths < 0;
        // Candidates: every (step, alter) with alter in -2..=2 that hits
        // the semitone, octave chosen so midi lands in-range.
        let mut best: Option<((i32, i32, i32), i32, Pitch)> = None;
        for octave in (0..=9).rev() {
            for step in Step::ALL {
                let base = (octave + 1) * 12 + step.natural_semitone();
                let alter = midi - base;
                if !(-2..=2).contains(&alter) {
                    continue;
                }
                let cand = Pitch {
                    step,
                    alter,
                    octave,
                };
                // Rank: fewer accidentals first, then key-direction
                // preference, then lower letter position for stability.
                let dir_bonus = if prefer_sharps && alter > 0 {
                    -1
                } else if prefer_flats && alter < 0 {
                    -1
                } else {
                    0
                };
                let rank = (alter.abs(), dir_bonus, step as i32);
                if best.is_none_or(|(b, _, _)| rank < b) {
                    best = Some((rank, octave, cand));
                }
            }
        }
        best.map(|(_, _, p)| p).unwrap_or(Pitch {
            step: Step::C,
            alter: 0,
            octave: 4,
        })
    }

    /// Semitone transpose preserving nothing but sound: computes the
    /// new sounding pitch then respells under `fifths`.
    pub fn transposed(&self, semitones: i32, fifths: i32) -> Self {
        Pitch::spell(self.midi() + semitones, fifths)
    }
}

/// MusicXML `<accidental>` display values — notation hint only; the
/// sounding pitch is always `Pitch`. Imported where present, re-emitted
/// unchanged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Accidental {
    Sharp,
    Natural,
    Flat,
    DoubleSharp,
    FlatFlat,
    NaturalSharp,
    NaturalFlat,
    SharpSharp,
    QuarterFlat,
    QuarterSharp,
    ThreeQuartersFlat,
    ThreeQuartersSharp,
    SharpDown,
    SharpUp,
    NaturalDown,
    NaturalUp,
    FlatDown,
    FlatUp,
    DoubleSharpDown,
    DoubleSharpUp,
    FlatFlatDown,
    FlatFlatUp,
    TripleSharp,
    TripleFlat,
}
