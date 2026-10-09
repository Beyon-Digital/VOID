# VOID Notation (W25 — Lane V)

Score model + interchange for the score editor. Covers the NON-NATIVE
halves of T90 (editing over stable ids) and T91 (interchange + anchors).
Engraving/rendering is out of this lane — see `NEEDS.md`.

## Layout

- `crates/void-notation/` — the model crate (pure Rust, no audio/GUI):
  - `model.rs` — `Score`/`Part`/`Measure`/`Element`. Every entity
    (note, rest, chord, tuplet, beam, slur, articulation, lyric, tab)
    is an `Element` with a stable `ElementId`. Timed elements carry a
    `Position` (measure + offset/duration/voice/staff); attachments
    (slur/articulation/lyric/tab, and tuplet/beam groupings) address
    their hosts by id. `semantic_eq` defines interchange equality;
    `canonical_json` is the byte-stable form.
  - `ops.rs` — `ScoreOp` enum applied via `Score::apply` /
    `apply_transaction` (all-or-nothing, rollback on mid-plan failure)
    with exact `UndoToken`s. Ops: insert/delete/move/transpose/
    setDuration/setPitch/assignLyric/bindTab/attachArticulation/
    createSlur/groupTuplet/beam/setTempoMap/upsertAnchor/removeAnchor.
    Cascade rules on delete: referencing slurs are removed, tuplet/beam
    groups shrink (beam < 2 members → removed), attached leaves go with
    their host. `extract_part` derives a standalone single-part score
    with ids unchanged.
  - `musicxml.rs` — real MusicXML 4.0 `score-partwise` import + export.
    Measures/attributes (key/time/clefs/staves)/notes (pitch, duration,
    dots, ties, accidentals, voice, staff), `<backup>`/`<forward>`
    multi-voice, directions (words/metronome/dynamics/wedges/segno/
    coda/rehearsal + `<sound tempo>` → score tempo map), notations
    (slurs, tuplets — nested — articulations, fermata, technical
    string/fret), lyrics (multi-verse + syllabic), whole-measure rests,
    part-groups. Stable-id contract: `<note id>` preserved verbatim;
    everything else gets a deterministic `derive_id` (uuid-v5 over its
    canonical descriptor), so re-import of our own export reproduces
    identical ids. Anything unmodelled lands in the `LossReport`
    (`void-loss/1`, same contract as void-exchange) — grace notes,
    ornaments, harmony, barline markup, print/layout, instrument
    assignment, unpitched/cue notes, non-tempo sound attrs, and
    inexact `divisions`→ticks (approximated). Proprietary formats stay
    blocked: the importer hard-errors on non-XML/garbage input.
  - `anchors.rs` — movie-scoring anchors. `Anchor.at_seconds` is an
    exact rational — the ONLY stored position; tick position is derived
    via `TempoMap::ticks_at`, so `setTempoMap` cannot move an anchor in
    time (the T91 invariant the tests assert). `TimecodeMode` covers
    24/25/30 NDF, 29.97 NDF + DF, 59.94 DF with real `num/den` rates
    (30000/1001) and SMPTE drop-label math; `format_timecode` renders
    `HH:MM:SS:FF`/`;FF`.
- `packages/void-studio/src/notation/` — the view layer:
  - `types.ts` — wire DTOs mirroring the crate serde (string-int64
    ticks, `op`/`params` op envelopes, flattened element kinds).
  - `ops.ts` — gesture→op-plan builders (`insertNotePlan`,
    `deleteSelectionPlan`, `moveSelectionPlan`, `nudgeSelectionPlan`,
    `transposePlan`, `setDurationPlan`, `assignLyricPlan`,
    `bindTabPlan`, `attachArticulationPlan`, `createSlurPlan`,
    `groupTupletPlan`, `beamPlan`, `setTempoMapPlan`, anchor ops).
    Builders validate shapes/tick literals only; the crate owns model
    truth and rolls back failures.
  - `store.ts` — zustand view-state store: selection by stable id +
    staged plan + loss-report surface. No score content — see
    `assertViewStateOnly` coverage in `notation.test.ts`.
  - `capabilities.ts` — honest flags (`ENGRAVING_AVAILABLE = false`,
    `SCORE_OPS_WIRE_AVAILABLE = false`, `AAF/FINALCUT/LOGICX = false`,
    `MUSICXML_INTERCHANGE_AVAILABLE`/`SCORE_ANCHORS_AVAILABLE = true`).
- `tests/notation/` — detached suite with real `.musicxml` fixtures
  (etude, two-part voices/tuplet/chord/beam/slur, guitar tab/lossy
  markup, odd divisions). Asserts import shape, semantic-equality
  round-trips, stable ids through re-import AND through edit ops,
  transaction rollback, cascade delete + undo, lyric/tab upsert id
  stability, part extraction, anchor-survives-tempo, timecode labels,
  loss-report determinism, canonical JSON + string-int64 wire shape.

## Honest asymmetries (by design)

- A `Direction` element of kind `TempoMark` re-imports as a tempo-map
  point, not a direction — `<sound tempo>` has no direction-type twin;
  import puts it in the map. Fixtures avoid mixing the two.
- Chord members are ONE element (id = head note's); a `<chord/>` note's
  own `id` attr is absorbed — reported `approximated` when present.
- `pitch.octave` holds sounding octave — a `clef-octave-change` on the
  clef is carried, transpose/spelling follows the key signature.
