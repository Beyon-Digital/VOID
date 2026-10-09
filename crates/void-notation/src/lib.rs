//! void-notation — W25 "Notation, scoring and advanced interchange"
//! non-native implementation for VOID.
//!
//! Four subsystems, all real and all Linux-verifiable:
//!
//! - **Score model** (`model`): parts/measures of [`Element`]s with
//!   stable [`ElementId`]s — notes, rests, chords, tuplets, beams,
//!   slurs, articulations, lyrics, tab bindings, directions — on the
//!   VOID 960,000-tick quarter grid. `semantic_eq` + `canonical_json`
//!   define interchange equality; `validate` collects every violation.
//! - **Edit ops** (`ops`): every mutation is a typed [`ScoreOp`] inside
//!   a single transaction boundary (`apply_transaction`), returning an
//!   [`UndoToken`] that rewinds exactly — insert/delete/move/transpose/
//!   duration/part-extraction/lyric/tab/slur/tuplet/beam. Deletes
//!   cascade to attached leaves and shrink/remove containers, all
//!   captured for undo.
//! - **MusicXML 4.0** (`musicxml`): score-partwise import/export with
//!   deterministic [`LossReport`]s — unsupported constructs are dropped
//!   or approximated with entries, never silently. `<note id>` carries
//!   stable identity across round-trips.
//! - **Movie-scoring anchors** (`anchors`): absolute-time [`Anchor`]s
//!   whose tick position is *derived* through the tempo map — tempo-map
//!   edits cannot move an anchor's seconds by construction. Rational
//!   [`Rat`] arithmetic is exact; [`TimecodeMode`] covers declared
//!   SMPTE modes including 29.97/59.94 drop-frame relabeling.
//!
//! Engraving (Verovio) is deliberately out of scope — it is a
//! native/GUI NEEDS entry in `docs/notation/`.

pub mod anchors;
pub mod error;
pub mod model;
pub mod musicxml;
pub mod ops;
pub mod pitch;

pub use anchors::{
    format_timecode, frame_seconds, timecode_frame, timecode_label, Anchor, AnchorKind, DropFrame,
    Rat, TempoMap, TimecodeMode,
};
pub use error::{NotationError, Result};
pub use model::*;
pub use musicxml::{export_musicxml, import_musicxml};
pub use ops::{ScoreOp, TransposeScope, UndoToken};
pub use pitch::{Accidental, Pitch, Step};
pub use void_exchange::{ExchangeDirection, LossEntry, LossKind, LossReport};
