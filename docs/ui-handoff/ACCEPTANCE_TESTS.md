# VOID — UI/UX acceptance tests

Status: **requirements for the implementation agent; none of these application tests were run in the design task.**

Use deterministic fixtures plus native integration tests. Figma fixtures are not runtime data. Preserve original engine tests in `references/engine-handoff/TEST_MATRIX.md`.

## UI-T01 · First empty project
**Screens:** S13 S14 S27
**Action:** Create a blank project from home.
**Pass:** Show no seeded notes, tracks or assets; choose a first action. No mandatory account/model download.

## UI-T02 · Workspace continuity
**Screens:** S01 S22
**Action:** Select a region and change workspaces repeatedly.
**Pass:** Stable selection, musical time, history and viewport restoration; no second project loaded.

## UI-T03 · Theme and sizing
**Screens:** S01 S22 S23
**Action:** Exercise 1600×1000 and 1280×832 in both themes.
**Pass:** No clipped actionable controls; secondary panels collapse; semantic fills and readable labels in both modes.

## UI-T04 · Text zoom
**Screens:** S01 S22
**Action:** Use 200% text scaling with keyboard navigation.
**Pass:** Reflow or intentional scroll retains controls and focus; no whole-screen bitmap scaling.

## UI-T05 · Knob and fader
**Screens:** S04
**Action:** Drag, fine-adjust, type values, press arrows, reset and undo.
**Pass:** Displayed units, handle, parameter and engine agree; values bounded; one drag produces one undo transaction.

## UI-T06 · Command retry
**Screens:** S01 S02
**Action:** Replay the same edit commandId after a dropped acknowledgement.
**Pass:** No duplicate note/clip; reconcile the acknowledged revision.

## UI-T07 · Native time mapping
**Screens:** S01 S02
**Action:** Test tempo changes, zooming, off-grid notes, loop edges and sample boundaries.
**Pass:** UI geometry maps through the shared native time contract, not independent JS timers.

## UI-T08 · Keyboard and text
**Screens:** S01 S02
**Action:** Type a space, shortcuts and IME composition inside fields; then exit.
**Pass:** No unwanted transport start. Editor shortcuts resume only in the appropriate context.

## UI-T09 · Input permission denial
**Screens:** S15 S05
**Action:** Deny microphone permission or remove the selected device.
**Pass:** Explain the blocker without a fake meter or recording state; allow configuration or a manual MIDI path.

## UI-T10 · Record preservation
**Screens:** S05 S06
**Action:** Record, stop, review, cancel comp and reopen project.
**Pass:** Original take stays available; flush errors surfaced; review only after retained output is ready.

## UI-T11 · Nondestructive comp
**Screens:** S06
**Action:** Choose three phrases from four takes, commit and undo.
**Pass:** Comp changes are reversible; source asset bytes remain unchanged.

## UI-T12 · Ghost preview and audition
**Screens:** S02 S25
**Action:** Request notes, audition and stop without accepting.
**Pass:** No durable musical edit or auto-insert; preview bus stops cleanly and original remains unchanged.

## UI-T13 · Partial acceptance
**Screens:** S02 S26 S03
**Action:** Accept four selected notes, then accept the remaining four.
**Pass:** Only selected IDs commit each time; two acknowledged transactions; ghosts separated from notes.

## UI-T14 · Partial undo and discard
**Screens:** S26 S02
**Action:** Undo the last accepted subset; alternatively discard remaining ghosts.
**Pass:** Undo restores accepted subset only; discard never deletes already committed notes.

## UI-T15 · Stale response
**Screens:** S20
**Action:** Change the song while the suggestion is generating.
**Pass:** Reject old expectedRevision; disable acceptance; refresh explicitly; no silent rebase.

## UI-T16 · Invalid output
**Screens:** S02
**Action:** Return malformed notes, unknown IDs, out-of-range velocities or nonfinite time.
**Pass:** Typed validation refuses the proposal before engine mutation and preserves the project.

## UI-T17 · Gesture safety
**Screens:** S08
**Action:** Draw with pointer capture, lose focus, cancel, reconnect and commit.
**Pass:** Held notes release on loss; preview discarded on cancel; committed notes remain editable.

## UI-T18 · Gesture alternatives
**Screens:** S08
**Action:** Use piano roll/keyboard without camera or continuous movement.
**Pass:** Equivalent conventional composition remains available; camera consent is never forced.

## UI-T19 · Generation failure
**Screens:** S10 S17
**Action:** Cancel, exhaust memory or kill a worker mid-job.
**Pass:** Song unchanged; job state accurate; partial assets quarantined/cleaned; no automatic cloud fallback.

## UI-T20 · Asset acceptance
**Screens:** S10 S16
**Action:** Accept a generated result, move source folder and reopen.
**Pass:** Imported immutable asset has provenance and availability state; media not an ephemeral URL.

## UI-T21 · Scene timing
**Screens:** S07 S28
**Action:** Queue a scene just before and just after a bar boundary.
**Pass:** Queued is visible until native launch; explicit sceneId/trackId membership; no prefix inference.

## UI-T22 · Live safety
**Screens:** S07
**Action:** Trigger all-notes-off or stop all clips while optional workers are busy.
**Pass:** Native action remains responsive; state acknowledgement visible; no AI permission gate on panic.

## UI-T23 · Preview and program
**Screens:** S09
**Action:** Open visuals and generate a candidate with an external display connected.
**Pass:** Output remains disarmed until explicit arm; preview never unexpectedly goes live.

## UI-T24 · Visual failure
**Screens:** S09
**Action:** Kill renderer or overload GPU during playback.
**Pass:** Expose preview/output failure; native audio does not wait for frames; blackout/fallback is visible.

## UI-T25 · Missing plugin
**Screens:** S18
**Action:** Reopen with an unavailable or quarantined plugin.
**Pass:** Saved state and routing retained; locate/replace/bypass explicit; no silent substitute.

## UI-T26 · Engine exit
**Screens:** S19
**Action:** Terminate native engine or force a plugin crash on the in-process path.
**Pass:** UI says audio stopped, offers verified recovery and restarts stopped; no uninterrupted-audio claim.

## UI-T27 · Failed checkpoint
**Screens:** S21
**Action:** Simulate no space, denied write or disappearing destination.
**Pass:** Keep unsaved edits and last good checkpoint; header does not show Saved until durable verification.

## UI-T28 · Real export
**Screens:** S11 S12
**Action:** Export a known fixture, including tails, then compare bytes/sample properties.
**Pass:** Real render duration/format/channels correct; success only on verified file; no double dither.

## UI-T29 · Preflight failures
**Screens:** S11
**Action:** Disconnect routing, remove an asset and disable a plugin.
**Pass:** Report actual missing dependencies and blocking conditions instead of hardcoded green checks.

## UI-T30 · Memory and repaint
**Screens:** S01 S04
**Action:** Open large fixture, scroll/zoom and operate meters for a long session.
**Pass:** Bounded viewport subscriptions, peak-level-of-detail and cache eviction; no raw PCM/tensors in WebView state.

## UI-T31 · Renderer reload
**Screens:** S01 S19
**Action:** Reload presentation while native playback/recording is active.
**Pass:** Test architecture behavior; engine never depends on a WebView timer; retain truthful health and reconciliation.

## UI-T32 · Least privilege
**Screens:** S17
**Action:** Attempt worker path escape, unapproved upload and unauthorized device command.
**Pass:** Controller rejects requests; no ambient filesystem/network/camera rights; redact diagnostics.

## UI-T33 · Accessibility
**Screens:** S01 S04 S08
**Action:** Screen-reader and keyboard-test controls, selection and errors.
**Pass:** Names, units, focus and noncolor state cues; grids expose manageable visible subsets and meaningful navigation.

## UI-T34 · Score mapping
**Screens:** S24
**Action:** Change display quantization and map selection back to the piano roll.
**Pass:** Performance unchanged by engraving settings; stable note IDs; supported editing/export separately tested.

## UI-T35 · Native plugin window
**Screens:** S01 S04
**Action:** Open, focus, resize and close a real VST3/AU editor on each supported OS.
**Pass:** No fake web editor; native window ownership and focus shortcuts verified.

## UI-T36 · Update inhibition
**Screens:** S01 S17
**Action:** Offer update during recording, performance or unsaved changes.
**Pass:** No forced restart, install interruption or lost work; exact update policy enforced.
