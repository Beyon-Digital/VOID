# Notation NEEDS — Lane V (W25)

NEEDS-style entries for every piece of T90/T91 that this Linux lane
cannot deliver — each lists what EXISTS here (the real model/op/policy
layer) vs what still needs the native engine / GUI / vendor path.

## NEEDS-notation-01 — Engraved score rendering (Verovio)

- **Blocked by:** native engine + GUI surface. Decision VIS-04 is
  Verovio — an engraving backend that renders the model to
  SVG/screen; nothing renderable exists on a headless Linux lane.
- **What exists here:** the complete score model (`crates/void-notation`)
  is the engraving input — stable ids, positions, all notations; the
  MusicXML exporter is exactly the interchange Verovio consumes (it
  reads MusicXML natively). `ENGRAVING_AVAILABLE = false` in
  `packages/void-studio/src/notation/capabilities.ts`.
- **Needed:** a native/GUI task that feeds `Score` (or its MusicXML
  export) into Verovio inside the tauri app and returns page/glyph
  geometry for hit-testing. Element ids map to Verovio `xml:id` so
  selection stays id-stable end-to-end.
- **Acceptance when done:** rendered page, click→stable element id
  selection round-trip, T90 UI half.

## NEEDS-notation-02 — Protocol wire for score ops

- **Blocked by:** protocol major.1 has no score op variant; PersistentOp
  has no `Score`/`Notation` member, and no ReadItem carries measures/
  elements.
- **What exists here:** `ScoreOp` is already serde-tagged
  (`{op, params}`, camelCase, string-int64) — the exact DTO
  `packages/void-studio/src/notation/types.ts` mirrors; `apply_transaction`
  + `UndoToken` are the apply path. Studio `store.ts` stages plans.
  `SCORE_OPS_WIRE_AVAILABLE = false` flags the honest gap.
- **Needed:** a protocol task adding `ApplyScoreOps` (op plan + score
  root) and a `SCORE_VIEW` ReadItem carrying the canonical score JSON;
  a coordinator mapping view-store staging → that op.
- **Acceptance when done:** staged plan → coordinator → model mutated,
  undo token returned, undo restores.

## NEEDS-notation-03 — AAF / Final Cut Pro XML interchange

- **Blocked by:** proprietary interchange — no licensed converter on
  this lane; CONTRACTS bars claiming support.
- **What exists here:** nothing pretends to work —
  `AAF_INTERCHANGE_AVAILABLE` / `FINALCUT_INTERCHANGE_AVAILABLE` are
  `false`; the model + anchor layer are the eventual target schema
  (markers/cues map to `Anchor`, picture cuts to hit anchors).
- **Needed:** an integration decision (e.g. via `pyaaf2`-style native
  adapter or vendor SDK on the macOS/Windows lanes), then an
  importer mapping AAF metadata/tracks to score parts + anchors.
- **Acceptance when done:** real .aaf fixture imports with loss report;
  export emits a file a DAW/video suite opens.

## NEEDS-notation-04 — .logicx (Logic Pro) import

- **Blocked by:** Apple's closed format; blocked intentionally
  (T91: "unsupported proprietary conversion stays blocked").
- **What exists here:** `LOGICX_INTERCHANGE_AVAILABLE = false`; non-XML
  input to `import_musicxml` hard-errors — nothing silently passes.
- **Needed:** none for T91 — the requirement IS the block staying
  honest. Reopen only if a legitimate licensed path appears.

## NEEDS-notation-05 — Sheet-music playback/MIDI render of a Score

- **Blocked by:** the audio engine (native/void-engine) — playback needs
  the JUCE worker, not this lane.
- **What exists here:** the model carries everything a renderer needs
  (pitches, durations, tempo map, voices); `tempo_map` +
  `measure_len_ticks` give absolute tick→beat mapping.
- **Needed:** an engine task mapping `Element` timed positions →
  engine note events under the tempo map (same conversion the anchor
  layer proves: ticks_at/time_at_ticks).
