# UIP7 evidence — S24 Score / notation workspace (figma 4:2308, UI-T34)

Lane: `devin/void-lane-uip7` branched from `devin/void-ui` @ `44a3a5cce0781498d0793d5fa4ebdad0d898d47b`.
Screen: `apps/void-tauri/src/ui/studio/screens/score/` (hash route `#/score`).

## What is real

- **Derived score model** (`packages/void-studio/src/notation/derive.ts`) —
  engine notes → display score. `crates/void-notation` is the score-model
  authority, but protocol major.1 carries no score document
  (`SCORE_OPS_WIRE_AVAILABLE = false`), so the screen derives notation from
  the SAME engine notes the piano roll edits (NOTE_RANGE). Element identity
  IS the engine `note_id` — selection maps bidirectionally with the roll.
- **Display quantization (UI-T34)** — a view setting only: `layoutScore`
  snaps display offsets/durations to the chosen grid; the raw ticks that
  carry timing are never rewritten. Tests assert raw equality across grid
  settings (`derive.test.ts`, `model.test.ts`).
- **Parts rail** — TRACK_LIST items → real part rows; the active part's
  clips come from CLIP_LIST (`ClipEditor.loadTrackClips`, paged) and notes
  from NOTE_RANGE (`NoteEditor.loadClipNotes` — the same loader the roll
  uses, merged into the bounded view cache).
- **Internal score renderer** — honest SVG: staff lines, ledger lines,
  treble clef, barlines, measure numbers, whole-measure rests, elliptical
  noteheads + stems, hollow heads for ≥half durations, tie arcs across
  barlines, second-interval x-shift. `ENGRAVING_AVAILABLE = false` is stated
  in the inspector (NEEDS-notation-01) — no fake glyph pretense.
- **Selection ⇄ piano roll** — clicking a note calls the identical actions
  the roll calls (`editorStore.actions.selectNotes`/`toggleNote` +
  `studioStore.actions.selectNote`), so the roll and score share one
  `noteSelection`. Shift/Ctrl adds to the selection; Escape clears.
- **Editing** — keyboard gestures on the canvas go through
  `noteEditIntent` → `NoteEditor.applyIntent` (arrows move, Alt+arrows
  resize, +/- velocity, Delete removes); double-click in a bar inserts a
  real `InsertNoteOp` at the snapped onset when a MIDI clip covers it —
  or shows the honest reason when none does. One gesture = one
  `crypto.randomUUID()` transaction id; `sendWithStaleRetry` inside
  `NoteEditor` handles STALE_REVISION.
- **MusicXML export** — `exportMusicXml` serializes the part to MusicXML
  4.0 score-partwise: raw tick durations verbatim, engine note ids on
  `<note id>`, ties across barlines, chords by shared onset+duration,
  non-chord overlaps in numbered voices via `<backup>`, velocity in the
  note attribute, measure rests for empty bars. Download is a browser
  Blob; the crate-side writer stays coordinator-only (inspector says so).
- **Display controls** — meter (4/4 3/4 2/4 6/8 12/8) and spelling
  (sharps/flats) are view settings; Key shows `—` because the wire carries
  no key signature (NEEDS). Page/Scroll layouts both render.
- **Both themes** (token CSS vars only) and **compact ≤1280** (parts rail
  + inspector collapse to Escape-closeable drawers; canvas stays).

## Honest gaps (named in the UI, not worked around)

| Seam | Status | Where the UI says it |
|---|---|---|
| Verovio / engraving engine | `ENGRAVING_AVAILABLE = false` | inspector "Wire gaps" + `internal layout` tag on paper |
| Score-op wire (ScoreOpDto → engine) | `SCORE_OPS_WIRE_AVAILABLE = false` | inspector "Score-op wire is absent — edits use the shipped note ops" |
| Key signature | no key/meter view on the wire | `Key —` row |
| Notation playback | NEEDS-notation-05 | inspector |
| Score-model anchors/voices beyond the derived view | out of scope of this wire | export note |
| Audio clips in a part | not notatable | they are filtered (`isMidiClip`); a part with none shows the empty state |

## Files

- `packages/void-studio/src/notation/derive.ts` — notes→absolute,
  meter/quantize, spelling, staff positions, measure layout, MusicXML
- `packages/void-studio/src/notation/derive.test.ts` — 16 vitest cases
- `packages/void-studio/src/notation/index.ts` — added `./derive` export
- `packages/void-studio/src/index.ts` — added `./notation` barrel export
- `apps/void-tauri/src/ui/studio/screens/score/index.tsx` — screen
- `apps/void-tauri/src/ui/studio/screens/score/model.ts` — pure model
- `apps/void-tauri/src/ui/studio/screens/score/model.test.ts` — 10 vitest cases
- `docs/ui-handoff/tracking/evidence/UIP7-score-{dark,selection}.png`

## Wired data sources

| Surface | Source |
| --- | --- |
| Parts | `TRACK_LIST` items → `partsFromTrackItems` |
| Clips | `CLIP_LIST(track_id)` via `ClipEditor.loadTrackClips` |
| Notes | `NOTE_RANGE(track_id, clip window)` via `NoteEditor.loadClipNotes` → `parseNoteItem` |
| Project name | `PROJECT_SUMMARY` via `useProjectSummary` |
| Selection | `editorStore.noteSelection` + `studioStore.selection` (shared with the roll) |
| Edits | `InsertNoteOp`/`SetNoteOp`/`RemoveNoteOp` via `NoteEditor` |
| Export | `exportMusicXml` (derived part) → Blob download |

## Gates

| Gate | Result |
| --- | --- |
| `pnpm install --frozen-lockfile` | exit 0 (baseline) |
| `pnpm -r --if-present build` (baseline) | exit 1 — all packages + `vite build` + cargo release succeed; `tauri build` fails ONLY at updater signing (`TAURI_SIGNING_PRIVATE_KEY` unset — pre-existing env gap, bundles still produced) |
| `pnpm -r --if-present test` (baseline) | exit 0 — 487 studio + 19 app vitest |
| `pnpm --filter ./packages/void-studio build` | exit 0 (tsc) |
| `pnpm vitest run src/notation/derive.test.ts` | exit 0 — 16 tests |
| `pnpm vitest run` (apps/void-tauri) | exit 0 — 29 tests (19 visuals + 10 score) |
| `pnpm exec tsc --noEmit` (apps/void-tauri) | exit 0 |
| `pnpm exec vite build` (apps/void-tauri) | exit 0 |
| `cargo test --workspace --exclude void-tauri` | not run — no rust touched |

## Live verification (Chrome, real read path)

`pnpm exec vite :5175` + Chrome at `/#/score`. The Tauri IPC transport is
absent in a plain browser, so `client.readView`/`readViewPages` were
stubbed to return real-shaped `ReadResponse` pages; every layer above —
`mergeReadPage`, `parseClipItem`/`parseNoteItem`, `deriveScoreNotes`,
`layoutScore`, selection actions, the SVG renderer — ran the production
code path. Verified: mount with parts rail populated; paper renders
noteheads/stems/chord/tie/measure numbers; note click → accent highlight +
inspector shows `Bars 1 · Pitch C4 · Onset 0 · Length 960000` (raw ticks
under display quantize); export button enabled only with notes present;
honest wire-gap block visible. Screenshots: `evidence/UIP7-score-*.png`.

Not verified live: engine-attached run (the mock worker returns a stub
read response — no notes); compact drawers and daylight theme reuse the
ui01-verified primitives and token vars, checked in code only.

## Acceptance mapping — UI-T34

- *"Change display quantization"* — quantize select (rail + inspector)
  re-derives layout only; tests assert raw onset/duration ticks are
  identical under `off`/`1/8`/`1/16` grids.
- *"map selection back to the piano roll"* — score element id IS the
  engine `note_id`; `selectNotes`/`toggleNote`/`selectNote` writes are the
  same actions the roll binds, so selection round-trips both directions.
- *"Performance unchanged by engraving settings"* — no write path exists
  from display settings to ops; edits go exclusively through note ops.
- *"supported editing/export separately tested"* — note-op paths are the
  piano roll's shipped ops; `exportMusicXml` has 4 dedicated cases.
