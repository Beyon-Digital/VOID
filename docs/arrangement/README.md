# W17 — Recording and arrangement depth (studio lane)

Scope: `packages/void-studio/src/{takes,arrangement,scenes}/`.
Owner: Engine+UI (this lane covers the studio/UI half; native/engine
recording depth is a separate lane — see `docs/engine/NEEDS.md`
rev2 items 18–31 for the wire gaps this lane documents but cannot
fill).

Feature outcomes covered: DOC-02, ENG-05, TIME-03, TIME-04, TIME-06,
REC-03..06, ARR-03, ARR-05, ARR-06, ARR-07, PAT-02..05. Tests T66,
T67, T68 in `docs/void-handoff/TEST_MATRIX.md`.

## What's real vs view-state

The wire (protocol major.1) has clip/note/track/tempo/transport ops
only — no fade, loop, alias, group, folder, scene, section, marker,
alternative, freeze, capture, or analysis ops. So the lane is split
honestly:

- **Real ops under one transactionId** — everything that mutates the
  arrangement is emitted through `sendWithStaleRetry` /
  `arrangement/editor.ts sendPlan` as ordinary
  Remove/Insert/Move/Trim/Split/InsertNote/SetTempo/SetTimeSignature
  ops, so one `UndoOp` reverses a whole composite gesture (comping,
  section move, alternative apply, freeze clip-swap, strip-silence).
- **View-state specs** — everything the wire can't express yet
  (fades, loops-as-repeat, alias membership, folders/stacks, groups,
  sections, markers, alternatives' saved specs, scene launch state,
  freeze spec, capture buffers, stream cache). Each is modeled in
  its own feature store (`arrangement/store.ts`, `scenes/store.ts`,
  `takes/store.ts`) — never on `studioStore` (the view-state gate
  rejects it). The matching wire ops are itemized in
  `docs/engine/NEEDS.md` items 18–31.

## takes/

- `types.ts` — `TakeRecord` (immutable: lane, region, asset ref,
  offset), `TakeFolder`, `CompSpec`/`CompSegment`, `FadeSpec`.
- `takes.ts` — `makeFolder`/`addTake` (non-mutating), `takesAt`/
  `takeAt` (topmost lane), `loopTakes` (REC-04: one take per pass;
  an interrupted pass is a kept incomplete take), `Region` math.
- `comp.ts` — `validateComp`, `buildComp`, `swipeComp` (contiguous
  cover from cut points), `cycleTakeAt` (T66 switch-alternatives),
  `seamFadePlan` (crossfade spec bounded by adjacent segments),
  `compToOps` — Remove+Insert per segment under ONE transaction;
  asset offsets are exact (`segmentAssetOffset`). MIDI segments
  insert empty clips, marked via `midiSegmentClipIds` (no note-copy
  op — NEEDS §28).
- `modes.ts` — overdub/replace/loop/step placement plans;
  `stepInputOps` + `repeatNotes` (REC-03: grid-fired step repeats,
  length clamped to the grid).
- `capture.ts` — `FlashbackBuffer` consent-first ring model
  (REC-06): disabled retains nothing, consent required to arm,
  eviction past capacity, `recoverToTake` maps the retained window
  into a take only when a materialized asset exists.
- `editor.ts`/`store.ts` — `applyCompPlan` commit path + feature
  store.

## arrangement/

- `alternatives.ts` — `captureTrackAlternative`,
  `applyTrackAlternativeOps`, `applyProjectAlternativeOps`
  (DOC-02: remove+insert in one tx; assets referenced, never copied).
- `regions.ts` — `loopClipOps` (materialized repeats),
  `aliasPropagateOps` (propagate-flag fan-out), `orderTracksWithFolders`.
- `fades.ts` — `FadeMap` view-state, `crossfadeForSeam`, and
  strip-silence: `detectSilence`/`keptRegions`/`stripSilenceOps`
  (ARR-05; loudness tiles are an analysis product — NEEDS §31).
- `sections.ts` — `Section`/`Marker` (TIME-04), `checkSections`,
  `moveSectionOps` (inside clips move; boundary clips split at the
  section edge then the inside part moves; markers shift by delta;
  automation/chords recorded in `carriedSpec`), `copySectionOps`,
  `shiftMarkers`.
- `tempo.ts` — `insertTempoOp`/`insertMeterOp` (TIME-03 real ops),
  exact `ticksToBarBeat` across meter changes, `sectionBars`.
- `groups.ts` — `EditGroup`, `groupEditOps` fan-out (ARR-06),
  `assertUnprotected`/`ProtectedError` (ARR-07 client-side rails:
  protected clips, tracks, and pinned ranges refuse edit ops),
  `visibleTracks`.
- `freeze.ts` — `FreezeSpec` + tail policy (`fixed`/`silence`/
  `clip-edge`), `freezeRenderRegion`, `freezeApplyOps` (ENG-05:
  post-render clip swap in one tx).
- `stream.ts` — bounded tile-cache view model (TIME-06): resident/
  loading/evicted tiles, LRU eviction at capacity, hit/miss counters,
  `tileWindows`. Metadata only — never PCM in the WebView.
- `editor.ts` — `sendPlan`/`undoPlan` commit path.
- `store.ts` — the whole arrangement view-state surface.

## scenes/

- `scenes.ts` — `Scene`/`SceneSlot` track×scene grid, `LaunchQuantize`
  (immediate/beat/bar/custom + meter-aware `quantizeLaunchAt`), the
  `pending→playing→stopping→stopped` launch machine (exclusive scene
  launch stops the previous scene's slots), `sceneTransportOps`
  (SEEK+SET_CYCLE mapping — the only wire surface today).
- `variation.ts` — `CaptureRecord`/`recordEvent`,
  `captureToArrangementOps` (clip cells re-insert at launch position;
  pattern cells become MIDI clip+notes; unmaterialized cells are
  honestly skipped), `patternToClipOps` (PAT-05 via
  `patternToInsertOps`), `patternVariation`.
- `store.ts` — grid/launch/capture store.

## What needs native/engine work (all NEEDS rev2)

Per-clip fade fields (§18), loop-repeat fields (§19), alias/link ops
(§20), folder/stack routing membership (§21), grouped-clip op arrays
(§22), document-level locks (§23), section/marker ops (§24),
alternative/playlist persistence (§25), scene launch ops + launch
view (§26), tempo ramps (§27), note-copy (§28), freeze/bounce render
job (§29), flashback capture surface (§30), loudness/peaks analysis
view (§31).
