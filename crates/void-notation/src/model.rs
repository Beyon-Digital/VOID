//! The score model — stable-id score semantics over the VOID tick grid.
//!
//! Every addressable entity in the score is an [`Element`] with a stable
//! opaque [`ElementId`]. Identity is *not* position: moving an element
//! between measures changes its `position`, never its id, and the same
//! is true through interchange — `note/@id` is emitted on MusicXML
//! export and re-read on import, while attached entities (slurs, tuplets,
//! beams, articulations, lyrics, tab) derive deterministic ids from
//! their hosts, so an unchanged structure re-derives identical ids.
//!
//! Duration/offset units are VOID protocol ticks (960,000 per quarter
//! note, `TICKS_PER_QUARTER` — same domain as `void-exchange` documents).
//! MusicXML `divisions` is a file-level detail handled at the boundary,
//! never stored on the model.
//!
//! Serde contract: camelCase; every i64 tick field serializes as a
//! decimal string (`ticks_serde`, same rule as `void-exchange`).

use crate::pitch::{Accidental, Pitch};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// `void-exchange` re-exports the protocol constant; keep one source.
pub use void_exchange::TICKS_PER_QUARTER;

/// Decimal-string i64 serde — identical contract to
/// `void_exchange::document::ticks_serde`.
pub mod ticks_serde {
    pub use void_exchange::document::ticks_serde::opt;
    pub use void_exchange::document::ticks_serde::{deserialize, serialize};
}

/// Stable opaque element identity. Imported MusicXML notes keep their
/// `id` attribute; elements without one get a deterministic derived id
/// (`derive_id`), so a byte-identical re-import lands the same ids.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ElementId(pub String);

impl ElementId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ElementId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for ElementId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for ElementId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Deterministic element-id derivation (uuid v5 over a canonical
/// descriptor). Used when a source has no own identity (MusicXML
/// elements without `id`, op-created elements without an explicit id).
pub fn derive_id(descriptor: &str) -> ElementId {
    const NS: uuid::Uuid = uuid::Uuid::from_u128(0x7617_4c27_5984_5f4b_9e8a_40e5_c7d2_a913);
    ElementId(format!(
        "e{}",
        uuid::Uuid::new_v5(&NS, descriptor.as_bytes()).simple()
    ))
}

/// Position of a timed element: offset within its measure plus the
/// notated duration. Attached elements (slur/articulation/lyric/tab)
/// carry `None` — their position follows their host(s).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    /// Offset from the measure start; >= 0 (anacrusis is a shorter
    /// first measure, never a negative offset).
    #[serde(with = "ticks_serde")]
    pub offset_ticks: i64,
    /// Notated duration; 0 for zero-time marks (directions).
    #[serde(with = "ticks_serde")]
    pub duration_ticks: i64,
    /// MusicXML voice number (1..=8 on this lane's bound).
    pub voice: u32,
    /// Staff number within the part, 1-based (grand staff = 1..2).
    pub staff: u32,
}

/// MusicXML note `<type>` — engraving hint only; `duration_ticks` is
/// authoritative and every duration is legal even when no type maps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NoteType {
    Longa,
    Breve,
    Whole,
    Half,
    Quarter,
    Eighth,
    #[serde(rename = "16th")]
    N16,
    #[serde(rename = "32nd")]
    N32,
    #[serde(rename = "64th")]
    N64,
    #[serde(rename = "128th")]
    N128,
    #[serde(rename = "256th")]
    N256,
    #[serde(rename = "512th")]
    N512,
    #[serde(rename = "1024th")]
    N1024,
}

impl NoteType {
    /// Undotted duration in ticks.
    pub fn base_ticks(self) -> i64 {
        match self {
            NoteType::Longa => TICKS_PER_QUARTER * 16,
            NoteType::Breve => TICKS_PER_QUARTER * 8,
            NoteType::Whole => TICKS_PER_QUARTER * 4,
            NoteType::Half => TICKS_PER_QUARTER * 2,
            NoteType::Quarter => TICKS_PER_QUARTER,
            NoteType::Eighth => TICKS_PER_QUARTER / 2,
            NoteType::N16 => TICKS_PER_QUARTER / 4,
            NoteType::N32 => TICKS_PER_QUARTER / 8,
            NoteType::N64 => TICKS_PER_QUARTER / 16,
            NoteType::N128 => TICKS_PER_QUARTER / 32,
            NoteType::N256 => TICKS_PER_QUARTER / 64,
            NoteType::N512 => TICKS_PER_QUARTER / 128,
            NoteType::N1024 => TICKS_PER_QUARTER / 256,
        }
    }
    /// MusicXML type token (also used for metronome beat-units).
    pub fn as_str(self) -> &'static str {
        match self {
            NoteType::Longa => "longa",
            NoteType::Breve => "breve",
            NoteType::Whole => "whole",
            NoteType::Half => "half",
            NoteType::Quarter => "quarter",
            NoteType::Eighth => "eighth",
            NoteType::N16 => "16th",
            NoteType::N32 => "32nd",
            NoteType::N64 => "64th",
            NoteType::N128 => "128th",
            NoteType::N256 => "256th",
            NoteType::N512 => "512th",
            NoteType::N1024 => "1024th",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "longa" => NoteType::Longa,
            "breve" => NoteType::Breve,
            "whole" => NoteType::Whole,
            "half" => NoteType::Half,
            "quarter" => NoteType::Quarter,
            "eighth" => NoteType::Eighth,
            "16th" => NoteType::N16,
            "32nd" => NoteType::N32,
            "64th" => NoteType::N64,
            "128th" => NoteType::N128,
            "256th" => NoteType::N256,
            "512th" => NoteType::N512,
            "1024th" => NoteType::N1024,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClefSign {
    G,
    F,
    C,
    Percussion,
    Tab,
    Jianpu,
    None,
}

impl ClefSign {
    pub fn as_str(self) -> &'static str {
        match self {
            ClefSign::G => "G",
            ClefSign::F => "F",
            ClefSign::C => "C",
            ClefSign::Percussion => "percussion",
            ClefSign::Tab => "TAB",
            ClefSign::Jianpu => "jianpu",
            ClefSign::None => "none",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "G" => ClefSign::G,
            "F" => ClefSign::F,
            "C" => ClefSign::C,
            "percussion" => ClefSign::Percussion,
            "TAB" => ClefSign::Tab,
            "jianpu" => ClefSign::Jianpu,
            "none" => ClefSign::None,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clef {
    pub sign: ClefSign,
    /// Staff line the sign sits on; omitted when the sign has a
    /// conventional default (G=2, F=4, C=3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// Octave displacement (MusicXML `clef-octave-change`), 0 or ±1/±2.
    #[serde(default)]
    pub octave_change: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeySignature {
    /// Circle-of-fifths count: 0=C, negative=flat keys, positive=sharp.
    pub fifths: i32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeasureTimeSignature {
    /// `beats` — the numerator as written (may be "3+2" style? no:
    /// VOID keeps the single integer form; composite meters arrive via
    /// MusicXML only as separate sig elements which this lane reports).
    pub beats: u32,
    /// Denominator as written (4 = quarter). Powers of two for the
    /// supported subset; others validate but MusicXML re-export warns.
    pub beat_type: u32,
}

/// `<measure><attributes>` — per-measure attribute block. Fields are
/// all optional: a measure may restate any subset. `staves` is the
/// declared staff count used to bound `Position.staff`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MeasureAttributes {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<KeySignature>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time: Option<MeasureTimeSignature>,
    /// Clefs, one per staff entry that carries one (MusicXML allows
    /// `number` per clef; VOID stores the ordered list).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub clefs: Vec<Clef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub staves: Option<u32>,
}

/// Tie endpoints on a note/chord head.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tie {
    #[serde(default)]
    pub start: bool,
    #[serde(default)]
    pub stop: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteData {
    pub pitch: Pitch,
    /// Recorded engraving hint; emitted verbatim on MusicXML export.
    /// When absent the exporter does not invent one — duration is
    /// authoritative and `<type>` stays off (MusicXML makes it optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note_type: Option<NoteType>,
    #[serde(default)]
    pub dots: u8,
    #[serde(default)]
    pub tie: Tie,
    /// Optional displayed accidental (print hint).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub accidental: Option<Accidental>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestData {
    /// `measure="yes"` whole-measure rest.
    #[serde(default)]
    pub measure_rest: bool,
}

/// A chord is ONE element: shared position/duration, ordered pitch set.
/// Chord member pitches are not separately addressable — editors and
/// the TAB/lyric lanes disambiguate members by `member` index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChordData {
    /// >= 2 pitches in sounding order (low to high as authored).
    pub pitches: Vec<Pitch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note_type: Option<NoteType>,
    #[serde(default)]
    pub dots: u8,
    #[serde(default)]
    pub tie: Tie,
}

/// Tuplet grouping: `actual` notes in the time of `normal`.
/// `members` are timed element ids (or nested tuplet ids — a nested
/// tuplet's span derives from its own members).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TupletData {
    pub actual: u32,
    pub normal: u32,
    /// Engraving hint from `normal-type`; derived on export when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normal_type: Option<NoteType>,
    pub members: Vec<ElementId>,
}

/// Beam membership at one beam number. Roles mirror MusicXML `<beam>`
/// values; `ForwardHook`/`BackwardHook` cover broken secondary beams.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BeamRole {
    Begin,
    Continue,
    End,
    ForwardHook,
    BackwardHook,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeamMembership {
    pub element: ElementId,
    pub role: BeamRole,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BeamData {
    /// Beam level (MusicXML `number`): 1 = primary eighth beam.
    pub number: u32,
    pub members: Vec<BeamMembership>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SlurData {
    /// Start/end host element ids — both must be timed elements in the
    /// same part.
    pub start: ElementId,
    pub end: ElementId,
    /// MusicXML slur `number` (concurrent slurs disambiguate).
    #[serde(default = "default_slur_number")]
    pub number: u32,
}

fn default_slur_number() -> u32 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Placement {
    Above,
    Below,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum ArticulationKind {
    Accent,
    StrongAccent,
    Staccato,
    Tenuto,
    DetachedLegato,
    Staccatissimo,
    Spiccato,
    Scoop,
    Plop,
    Doit,
    Falloff,
    BreathMark,
    Caesura,
    Stress,
    Unstress,
    /// `<fermata>` — a notation of its own in MusicXML; modelled here so
    /// it round-trips (placement carried on the data, not the kind).
    Fermata,
    /// MusicXML `<other-articulation>` value, preserved verbatim.
    Other {
        value: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArticulationData {
    pub host: ElementId,
    #[serde(flatten)]
    pub kind: ArticulationKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<Placement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Syllabic {
    Single,
    Begin,
    Middle,
    End,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricData {
    /// Host note or chord element.
    pub host: ElementId,
    /// MusicXML lyric `number`/`name` — the verse/line selector.
    #[serde(default)]
    pub number: String,
    pub syllabic: Syllabic,
    pub text: String,
}

/// Tablature string/fret binding for one note or one chord member
/// (`member` indexes `ChordData::pitches`; 0 for plain notes).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabData {
    pub host: ElementId,
    #[serde(default)]
    pub member: u32,
    /// 1-based string (1 = highest sounding, guitar convention).
    pub string: u32,
    pub fret: u32,
}

/// Score direction kinds (the `direction-type` subset VOID supports
/// semantically). Unsupported direction-types are import loss entries,
/// never silently skipped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum DirectionKind {
    /// Tempo/mood text ("Allegro", "molto rit.").
    Words {
        text: String,
    },
    /// `<sound tempo="…"/>` — a sounding tempo mark (bpm, finite>0).
    TempoMark {
        bpm: f64,
    },
    /// Metronome mark `<beat-unit>×n = per-minute`.
    Metronome {
        beat_unit: NoteType,
        /// Multi-beat-unit ("quarter + eighth") extra units, in order.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        extra_units: Vec<NoteType>,
        /// Dots on the *first* beat-unit.
        #[serde(default)]
        beat_unit_dots: u8,
        per_minute: f64,
    },
    /// `<dynamics>` mark — token like `"mf"`, `"sfz"`.
    Dynamics {
        mark: String,
    },
    /// Crescendo/diminuendo hairpin endpoints.
    Wedge {
        wedge: WedgeKind,
    },
    /// Rehearsal mark (`<rehearsal>`).
    Rehearsal {
        text: String,
    },
    /// `<segno>` / `<coda>` navigation symbols.
    Segno,
    Coda,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WedgeKind {
    Crescendo,
    Diminuendo,
    Stop,
    Continue,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectionData {
    pub kind: DirectionKind,
}

/// Every element kind. Timed kinds carry `Position`; attached kinds
/// follow their host ids.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "data")]
pub enum ElementKind {
    Note(NoteData),
    Rest(RestData),
    Chord(ChordData),
    Tuplet(TupletData),
    Beam(BeamData),
    Slur(SlurData),
    Articulation(ArticulationData),
    Lyric(LyricData),
    Tab(TabData),
    Direction(DirectionData),
}

impl ElementKind {
    /// Short classifier for messages and loss `aspect` values.
    pub fn kind_name(&self) -> &'static str {
        match self {
            ElementKind::Note(_) => "note",
            ElementKind::Rest(_) => "rest",
            ElementKind::Chord(_) => "chord",
            ElementKind::Tuplet(_) => "tuplet",
            ElementKind::Beam(_) => "beam",
            ElementKind::Slur(_) => "slur",
            ElementKind::Articulation(_) => "articulation",
            ElementKind::Lyric(_) => "lyric",
            ElementKind::Tab(_) => "tab",
            ElementKind::Direction(_) => "direction",
        }
    }
}

/// One element = one addressable entity. `position` is `Some` for timed
/// kinds (note/rest/chord/direction) and `None` for attachments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Element {
    pub id: ElementId,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    #[serde(flatten)]
    pub kind: ElementKind,
}

impl Element {
    pub fn timed(&self) -> bool {
        self.position.is_some()
    }
    /// Host element ids this element references (empty for timed).
    pub fn hosts(&self) -> Vec<&ElementId> {
        match &self.kind {
            ElementKind::Slur(s) => vec![&s.start, &s.end],
            ElementKind::Articulation(a) => vec![&a.host],
            ElementKind::Lyric(l) => vec![&l.host],
            ElementKind::Tab(t) => vec![&t.host],
            ElementKind::Tuplet(t) => t.members.iter().collect(),
            ElementKind::Beam(b) => b.members.iter().map(|m| &m.element).collect(),
            _ => Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Measure {
    /// MusicXML `number` attribute — preserved verbatim (may carry
    /// suffixes like "12a" in source files).
    #[serde(default)]
    pub number: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attributes: Option<MeasureAttributes>,
    /// Timed + attached elements. Order is canonical (offset, voice,
    /// then insertion sequence) — see `canonical_elements`.
    #[serde(default)]
    pub elements: Vec<Element>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Part {
    /// Stable part id (MusicXML `score-part/@id` preserved on import).
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub abbreviation: Option<String>,
    /// MusicXML `part-group` name when the source carried one —
    /// preserved honestly, re-emitted on export.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_name: Option<String>,
    #[serde(default)]
    pub measures: Vec<Measure>,
}

impl Part {
    pub fn find_element(&self, id: &ElementId) -> Option<(usize, &Element)> {
        for (mi, m) in self.measures.iter().enumerate() {
            if let Some(e) = m.elements.iter().find(|e| &e.id == id) {
                return Some((mi, e));
            }
        }
        None
    }
    pub fn find_element_mut(&mut self, id: &ElementId) -> Option<(usize, &mut Element)> {
        for (mi, m) in self.measures.iter_mut().enumerate() {
            if let Some(e) = m.elements.iter_mut().find(|e| &e.id == id) {
                return Some((mi, e));
            }
        }
        None
    }
    /// Key-signature fifths in effect at `measure_index` (nearest
    /// preceding attributes block carrying a key; 0 = C major).
    pub fn fifths_at(&self, measure_index: usize) -> i32 {
        let mut fifths = 0;
        for m in self.measures.iter().take(measure_index + 1) {
            if let Some(a) = &m.attributes {
                if let Some(k) = &a.key {
                    fifths = k.fifths;
                }
            }
        }
        fifths
    }
    /// Effective time signature at `measure_index`.
    pub fn time_at(&self, measure_index: usize) -> Option<MeasureTimeSignature> {
        let mut ts = None;
        for m in self.measures.iter().take(measure_index + 1) {
            if let Some(a) = &m.attributes {
                if let Some(t) = &a.time {
                    ts = Some(t.clone());
                }
            }
        }
        ts
    }
    /// Effective declared staff count at `measure_index`.
    pub fn staves_at(&self, measure_index: usize) -> u32 {
        let mut staves = 1;
        for m in self.measures.iter().take(measure_index + 1) {
            if let Some(a) = &m.attributes {
                if let Some(s) = a.staves {
                    staves = s;
                }
            }
        }
        staves
    }
    /// Notated measure length in ticks at `measure_index`
    /// (`beats * 4 * TPQ / beat_type`); `None` when no time signature
    /// has been declared yet.
    pub fn measure_len_ticks(&self, measure_index: usize) -> Option<i64> {
        self.time_at(measure_index)
            .map(|t| t.beats as i64 * TICKS_PER_QUARTER * 4 / t.beat_type.max(1) as i64)
    }
}

/// Piecewise-constant tempo map for anchor conversion (PRO-02). Same
/// semantics as `void_exchange::TempoPoint`; stored locally so the score
/// document carries everything anchors need.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TempoPoint {
    #[serde(with = "ticks_serde")]
    pub at_ticks: i64,
    /// Beats per minute; finite and > 0. Step changes only.
    pub bpm: f64,
}

pub use crate::anchors::{Anchor, AnchorKind, TimecodeMode};

pub const SCORE_FORMAT: &str = "void-notation/1";

/// The score document. Serde-canonical: serializing the same score
/// twice yields identical bytes; `semantic_eq` defines what two parses
/// must agree on for a round-trip to be lossless.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    /// `SCORE_FORMAT`; validated on parse.
    pub format: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub movement: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub composer: Option<String>,
    #[serde(default)]
    pub parts: Vec<Part>,
    /// Score-global tempo map used by the anchor layer (anchors are
    /// absolute-time; their tick position is *derived*, never stored).
    #[serde(default)]
    pub tempo_map: Vec<TempoPoint>,
    /// Declared timecode mode for movie scoring; `None` = undeclared.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timecode: Option<TimecodeMode>,
    /// Absolute-time anchors (hit points) — see `anchors.rs`.
    #[serde(default)]
    pub anchors: Vec<Anchor>,
}

impl Score {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            format: SCORE_FORMAT.to_string(),
            title: Some(title.into()),
            movement: None,
            composer: None,
            parts: Vec::new(),
            tempo_map: Vec::new(),
            timecode: None,
            anchors: Vec::new(),
        }
    }

    pub fn find_part(&self, part_id: &str) -> Option<usize> {
        self.parts.iter().position(|p| p.id == part_id)
    }

    /// Locate an element: (part index, measure index, element index).
    pub fn locate(&self, id: &ElementId) -> Option<(usize, usize)> {
        for (pi, p) in self.parts.iter().enumerate() {
            if let Some((mi, _)) = p.find_element(id) {
                return Some((pi, mi));
            }
        }
        None
    }

    pub fn element(&self, id: &ElementId) -> Option<&Element> {
        for p in &self.parts {
            if let Some((_, e)) = p.find_element(id) {
                return Some(e);
            }
        }
        None
    }

    /// Canonical element ordering within a measure: timed elements by
    /// (offset, voice, staff), then attachments sorted by id. The model
    /// never depends on `elements` insertion order for semantics.
    pub fn canonical_elements(measure: &Measure) -> Vec<&Element> {
        let mut timed: Vec<&Element> = Vec::new();
        let mut attached: Vec<&Element> = Vec::new();
        for e in &measure.elements {
            if e.timed() {
                timed.push(e);
            } else {
                attached.push(e);
            }
        }
        // Total order: (offset, voice, staff, id) — two parses of the
        // same document order elements identically no matter where the
        // source interleaved directions among notes.
        timed.sort_by(|a, b| {
            let p = a.position.unwrap();
            let q = b.position.unwrap();
            (p.offset_ticks, p.voice, p.staff, &a.id).cmp(&(
                q.offset_ticks,
                q.voice,
                q.staff,
                &b.id,
            ))
        });
        attached.sort_by(|a, b| a.id.cmp(&b.id));
        timed.extend(attached);
        timed
    }

    /// Put every measure's `elements` into canonical order (the model's
    /// documented invariant — callers, import and ops all rely on it).
    pub fn normalize(&mut self) {
        for p in &mut self.parts {
            for m in &mut p.measures {
                let canon = Score::canonical_elements(m).into_iter().cloned().collect();
                m.elements = canon;
            }
        }
    }

    /// Derived tick position of every anchor through `tempo_map`
    /// (anchors store absolute seconds — a tempo-map edit changes the
    /// derived ticks, never `at_seconds`; that is the
    /// survives-tempo-change invariant the tests assert).
    pub fn anchor_ticks(&self, default_bpm: f64) -> Vec<(String, i64)> {
        let map = crate::anchors::TempoMap {
            points: self.tempo_map.clone(),
        };
        self.anchors
            .iter()
            .map(|a| (a.id.clone(), map.ticks_at(a.at_seconds, default_bpm)))
            .collect()
    }

    /// Semantic equality for interchange round-trips (T91): compares
    /// every field the model claims to carry — element ids included,
    /// stream order canonicalized. Differs from `==` only in that
    /// `elements` insertion order is ignored.
    pub fn semantic_eq(&self, other: &Score) -> bool {
        if self.format != other.format
            || self.title != other.title
            || self.movement != other.movement
            || self.composer != other.composer
            || self.parts.len() != other.parts.len()
            || self.tempo_map != other.tempo_map
            || self.timecode != other.timecode
            || self.anchors != other.anchors
        {
            return false;
        }
        for (pa, pb) in self.parts.iter().zip(other.parts.iter()) {
            if pa.id != pb.id
                || pa.name != pb.name
                || pa.abbreviation != pb.abbreviation
                || pa.group_name != pb.group_name
                || pa.measures.len() != pb.measures.len()
            {
                return false;
            }
            for (ma, mb) in pa.measures.iter().zip(pb.measures.iter()) {
                if ma.number != mb.number || ma.attributes != mb.attributes {
                    return false;
                }
                let ca = Score::canonical_elements(ma);
                let cb = Score::canonical_elements(mb);
                if ca != cb {
                    return false;
                }
            }
        }
        true
    }

    /// Structural validation — returns *all* problems, mirroring
    /// `ExchangeDocument::validate`.
    pub fn validate(&self) -> std::result::Result<(), Vec<String>> {
        let mut errors = Vec::new();
        let mut bad = |m: String| errors.push(m);

        if self.format != SCORE_FORMAT {
            bad(format!(
                "format must be {SCORE_FORMAT:?}, got {:?}",
                self.format
            ));
        }
        let mut seen_parts: HashSet<&str> = HashSet::new();
        let mut all_ids: HashSet<&str> = HashSet::new();
        // First pass: collect every element id for reference checks.
        for p in &self.parts {
            for m in &p.measures {
                for e in &m.elements {
                    if !all_ids.insert(e.id.as_str()) {
                        bad(format!("duplicate element id {:?}", e.id));
                    }
                }
            }
        }
        for (pi, p) in self.parts.iter().enumerate() {
            if p.id.is_empty() {
                bad(format!("parts[{pi}] id is empty"));
            }
            if !seen_parts.insert(p.id.as_str()) {
                bad(format!("duplicate part id {:?}", p.id));
            }
            for (mi, m) in p.measures.iter().enumerate() {
                let capacity = p.measure_len_ticks(mi);
                let staves = p.staves_at(mi);
                for e in &m.elements {
                    let at = format!("parts[{pi}].measures[{mi}].{:?}", e.id);
                    match &e.kind {
                        ElementKind::Note(n) => {
                            if !(-3..=3).contains(&n.pitch.alter) {
                                bad(format!("{at} pitch alter out of range"));
                            }
                            if !(0..=9).contains(&n.pitch.octave) {
                                bad(format!("{at} pitch octave out of 0..=9"));
                            }
                        }
                        ElementKind::Chord(c) => {
                            if c.pitches.len() < 2 {
                                bad(format!("{at} chord with <2 pitches"));
                            }
                            for p_ in &c.pitches {
                                if !(-3..=3).contains(&p_.alter) || !(0..=9).contains(&p_.octave) {
                                    bad(format!("{at} chord pitch out of range"));
                                }
                            }
                        }
                        ElementKind::Rest(_) => {}
                        ElementKind::Direction(_) => {}
                        ElementKind::Tuplet(t) => {
                            if t.actual == 0 || t.normal == 0 {
                                bad(format!("{at} tuplet actual/normal must be > 0"));
                            }
                            if t.members.is_empty() {
                                bad(format!("{at} tuplet with no members"));
                            }
                            for mid in &t.members {
                                if !all_ids.contains(mid.as_str()) {
                                    bad(format!("{at} tuplet member {mid} missing"));
                                } else if mid == &e.id {
                                    bad(format!("{at} tuplet contains itself"));
                                }
                            }
                        }
                        ElementKind::Beam(b) => {
                            if b.number == 0 || b.number > 8 {
                                bad(format!("{at} beam number {} out of 1..8", b.number));
                            }
                            if b.members.is_empty() {
                                bad(format!("{at} beam with no members"));
                            }
                            for m_ in &b.members {
                                if !all_ids.contains(m_.element.as_str()) {
                                    bad(format!("{at} beam member {} missing", m_.element));
                                }
                            }
                        }
                        ElementKind::Slur(s) => {
                            for host in [&s.start, &s.end] {
                                if !all_ids.contains(host.as_str()) {
                                    bad(format!("{at} slur endpoint {host} missing"));
                                }
                            }
                        }
                        ElementKind::Articulation(a) => {
                            if !all_ids.contains(a.host.as_str()) {
                                bad(format!("{at} articulation host {} missing", a.host));
                            }
                        }
                        ElementKind::Lyric(l) => {
                            if !all_ids.contains(l.host.as_str()) {
                                bad(format!("{at} lyric host {} missing", l.host));
                            }
                        }
                        ElementKind::Tab(t) => {
                            if !all_ids.contains(t.host.as_str()) {
                                bad(format!("{at} tab host {} missing", t.host));
                            }
                            if t.string == 0 {
                                bad(format!("{at} tab string must be 1-based"));
                            }
                        }
                    }
                    if let Some(pos) = &e.position {
                        if pos.offset_ticks < 0 {
                            bad(format!("{at} negative offsetTicks"));
                        }
                        let is_timed = matches!(
                            e.kind,
                            ElementKind::Note(_) | ElementKind::Rest(_) | ElementKind::Chord(_)
                        );
                        if is_timed && pos.duration_ticks <= 0 {
                            bad(format!("{at} non-positive durationTicks"));
                        }
                        if pos.voice == 0 || pos.voice > 8 {
                            bad(format!("{at} voice {} out of 1..8", pos.voice));
                        }
                        if pos.staff == 0 || pos.staff > staves.max(1) {
                            bad(format!(
                                "{at} staff {} exceeds declared {staves}",
                                pos.staff
                            ));
                        }
                        if is_timed {
                            if let Some(cap) = capacity {
                                if pos.offset_ticks + pos.duration_ticks > cap {
                                    bad(format!("{at} overruns measure capacity {cap} ticks"));
                                }
                            }
                        }
                    } else if matches!(
                        e.kind,
                        ElementKind::Note(_)
                            | ElementKind::Rest(_)
                            | ElementKind::Chord(_)
                            | ElementKind::Direction(_)
                    ) {
                        bad(format!("{at} timed kind missing position"));
                    }
                }
            }
        }
        let mut prev = None;
        for (i, tp) in self.tempo_map.iter().enumerate() {
            if !tp.bpm.is_finite() || tp.bpm <= 0.0 {
                bad(format!("tempoMap[{i}] bpm {} not finite-positive", tp.bpm));
            }
            if let Some(p) = prev {
                if tp.at_ticks <= p {
                    bad(format!("tempoMap[{i}] not strictly increasing"));
                }
            }
            prev = Some(tp.at_ticks);
        }
        let mut seen_anchors: HashSet<&str> = HashSet::new();
        for a in &self.anchors {
            if !seen_anchors.insert(a.id.as_str()) {
                bad(format!("duplicate anchor id {:?}", a.id));
            }
            if a.at_seconds.to_f64() < 0.0 {
                bad(format!("anchor {:?} at negative time", a.id));
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Canonical JSON bytes — sorted elements, stable key order.
    pub fn canonical_json(&self) -> serde_json::Result<Vec<u8>> {
        let mut s = self.clone();
        for p in &mut s.parts {
            for m in &mut p.measures {
                let canon = Score::canonical_elements(m).into_iter().cloned().collect();
                m.elements = canon;
            }
        }
        serde_json::to_vec_pretty(&s)
    }
}
