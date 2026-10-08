# W14 — Gestures, Patterns, Quick Sampling & Harmonic Controls

Studio/UI-side implementation of WORK_PACKAGES.md W14. Everything here is
pure TypeScript in `packages/void-studio/src/{gestures,patterns,harmony,learn}/`
with presentational components in `packages/void-ui/components/`. Engine
input plumbing (real pointer/MIDI streams into these layers) is a separate
macOS concern — every committed gesture produces ordinary
`InsertNoteOp`/`SetNoteOp`/`InsertAudioClipOp` under one `transaction_id`,
and undo is the normal `UndoOp` path.

## gestures/

**Model.** A capture produces `RawGestureNote[]` — onset ticks, pitch,
velocity exactly as input. The rendered `GestureNote[]` preview applies a
reversible `RhythmTransform` chain on top:

```
onset = quantize(grid, strength) → swing(pair placement) → timing jitter
velocity += velocity jitter      (all seeded: stream(seed, index, lane))
pitch  = constraint(rawPitch, rawOnset)   — optional harmony hook
```

Raw fields are carried through every rendered note (`rawStartTicks`,
`rawPitch`, `rawVelocity`), so changing strength/swing/humanize re-renders
from the untouched raw layer — quantization can never compound or destroy
intent (T57).

- `contour.ts` — pointer path → fixed-cell resample → merged same-pitch
  notes. `makeContourMapping` wires the viewport's px→ticks.
- `rhythm.ts` — tap timestamps → exact relative onsets (`ms·bpm·16` ticks;
  first tap anchors `originTicks`). `onsetsToRawNotes` is the
  keyboard/numeric parity path (beats/ms/ticks).
- `keyboard.ts` — full step-entry alternative: arrows move cursor/pitch,
  Enter places, Backspace removes. Same `RawGestureNote` output — every
  gesture has a non-pointer equivalent (GEST-06).
- `session.ts` — zustand store. Phase machine `idle → armed → capturing →
  preview → committed`. Capture requires explicit `arm()` — gestures never
  steal navigation. `release()`/`panic()` discard pending input and the
  ghost preview (nothing can be left "on": previews never reach the
  engine). `assertGestureViewState` enforces the view-state-only rule.
- `commit.ts` — `commitGesture` mints noteIds + one `transactionId`,
  sends each `InsertNoteOp` via `sendWithStaleRetry`, stops at the first
  failure receipt. `undoGestureCommit` issues `UndoOp{transaction_id}` —
  one undo reverses the phrase without touching earlier takes (GEST-05).

## patterns/

- `stepPattern.ts` — drum grid model: rows × steps (`on, velocity, gatePpm,
  repeats, probabilityPpm, tie`). `stepRoll(seed, rowId, globalStep)` —
  deterministic probability keyed on the row **identity**, so inserting or
  editing one row never re-rolls another (PAT-01). `renderPattern` emits
  raw notes: ties extend the previous note, `repeats` ratchet within the
  step, cycle length = longest row (polyrhythm legal).
  `patternToInsertOps` materializes through the standard commit path.
- `pads.ts` — GM-flavoured 4×4 pad bank, choke groups (hats share group 1),
  slice binding. `padToNote` → raw note for the shared commit path.
- `slices.ts` — quick-slice spec: boundaries strictly inside (0, asset
  length); `sliceByCount` covers the asset exactly (remainder spread over
  leading slices). `sliceToClipOp` emits `InsertAudioClipOp` with
  `offset_ticks`/`length_ticks` — the source asset is a region reference,
  never copied or decoded (T59, SND-02/SND-05).
- `loops.ts` — deterministic built-in template list (`builtinLoops`),
  `searchLoops` text/tag/bpm filter, `previewLoop` renders ticks at the
  project bpm for an honest wall-clock `durationMs` — pure, zero sends.
- `store.ts` — editor view store (focus cell, query, template, pads).
  `applyTemplate` deep-clones so editing can't mutate the shared list.

## harmony/

- `chords.ts` — real pitch-class sets (maj/min/dim/aug/sus2/sus4/6/m6/7/
  maj7/min7/min7b5/dim7/add9), strict `parseChordSymbol` (returns null on
  anything unrecognized), flat-spelling preservation, `SCALES` +
  `impliedScale` conventions.
- `identify.ts` — pitch collection → ranked `ChordGuess[]` by
  coverage×precision. Ambiguity stays visible; `isConfident` gates
  suggestion-level output (INTEL-02) — nothing is auto-applied.
- `track.ts` — ordered non-overlapping chord regions. `upsertRegion`
  deterministically trims/splits/drops overlaps; `chordAt(ticks)`;
  JSON serialize/parse for the later app-state lane (the wire protocol
  has no chord ops — this is documented, not hacked around).
- `constraint.ts` — `makeConstraint(spec)` → `PitchConstraint` hook for
  the gesture render chain. `mode` = off | chord | scale; uncovered
  onsets fall back to the configured scale or pass through — a missing
  region never means "snap to something wrong" (TIME-05).
- `follow.ts` — explicit harmonic-follow transform on SELECTED notes:
  plan (previewable changes + skip reasons) → `SetNoteOp`s under one
  `transactionId` → `undoHarmonicFollow` via `UndoOp`.
- `store.ts` — chord-track editor view store (selection, constraint mode).

## learn/

- `types.ts` — `ControlSpec` (cc/note/axis/key), `TargetSpec` (param or
  macro), `Mapping` (curve + range + invert + out-of-range policy).
- `values.ts` — real value math: normalized input → invert → curve
  (linear/exponential/logarithmic/toggle) → [min,max]. Out-of-range input
  is clamped or rejected per the mapping (MIX-07). Macro fan-out applies
  each member's own curve/range.
- `store.ts` — registry + arming + dispatch:
  - **Explicit single arm** — a control only moves a parameter while that
    target is armed; navigation gestures can never change music (T58).
  - **Learn capture** — `beginLearn(target)` then the next 'down' event
    binds that control; capture precedes the arm check because learning
    normally happens before arming.
  - **Held tracking** — momentary 'down' controls are held until 'up';
    `releaseAll('disconnect' | 'focus-loss' | 'navigate')` releases every
    held control, disarms, and drops pending capture (documented
    all-notes-off semantics).
  - **PANIC** — releases held, disarms, cancels learn, increments
    `panicCount`, idempotent. Post-panic controls are ignored until the
    user re-arms.
  - Dispatch returns `ParamChange[]` — the caller applies them to the real
    parameter owner; this store never sends to the engine.

## void-ui components (additive, presentational)

`GesturePad` (armed contour surface), `TapPad` (tap capture, Enter/Space
parity), `StepGrid` (keyboard-operable drum grid, aria-pressed cells),
`DrumPadGrid` (velocity-from-position pads + C-group/bound badges),
`ChordRegionStrip` (region blocks + constraint-mode badge),
`LearnMatrix` (mappings table + arm/remove/learn/PANIC — always
reachable), `LoopBrowser` (searchable template listbox).

## Test-matrix mapping

| Row | Covered by |
|-----|------------|
| T57 | `gestures/gestures.test.ts` (quantize/swing/humanize math, raw retention, reversibility, arm→capture→preview→commit→undo, keyboard parity), `w14.test.ts` scenario |
| T58 | `learn/learn.test.ts` (only-armed-responds, learn capture, rebind uniqueness, releaseAll on disconnect/focus-loss, panic priority, clamp/reject), `w14.test.ts` scenario |
| T59 | `patterns/patterns.test.ts` (row-scoped edits, PAT-01 determinism, tie/ratchet/polyrhythm, slices immutable+complete, choke groups, tempo preview), `harmony/harmony.test.ts` (regions, constraint, explicit follow+undo), `w14.test.ts` scenario |

## Known gaps / honest limits

- Real pointer/MIDI streams are not plumbed (engine-side macOS lane);
  these layers take synthesized events — semantics fully testable headless.
- Chord-track persistence awaits an app-state op; `serializeChordTrack`
  output is ready for that lane.
- Gesture ghost preview renders through `preview[]` view state; the
  piano-roll's ghost layer is the intended host surface (proposals/).
