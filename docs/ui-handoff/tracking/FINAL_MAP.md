# FINAL_MAP — UI acceptance coverage (UI-T01–T36) + W26–W29 statement

Sweep lane: `devin/void-lane-uip7` @ `44a3a5cce0781498d0793d5fa4ebdad0d898d47b`.
Method: walked `SCREENS.json` / `COMPONENTS.json` / `UI_TESTS.json`; every
`verified` row was re-checked for real evidence (a file/test/recorded run —
per README, never a claim). Result of the audit:

- **SCREENS** — all 5 `verified` rows (S05, S13, S14, S15, S27) carry
  `UIP1_EVIDENCE.md` with command+exit-code gates and a recorded run;
  caveats are named inside the doc. No downgrade. The 4 `implemented` rows
  (S04, S09, S11, S12) carry UIP3/UIP6 evidence with gates. S24 moves to
  `verified` under this lane (UIP7_EVIDENCE.md).
- **COMPONENTS** — all 13 `verified` rows name real files under
  `packages/void-ui/components/` (existence re-checked) plus
  `editing.test.ts` (in-tree). No downgrade.
- **UI_TESTS** — previously zero `verified` rows. UI-T34 → `verified` this
  lane (executable tests + recorded run). No other row gains status:
  passing gates of a screen lane are screen evidence, not acceptance-run
  evidence; claims without a run stay `not_started`.

## Per-row map

Status legend: `verified` = executable test or recorded run exists;
`partial` = some of the behavior is covered by executable tests but the
full scenario never ran; `not_started` = no run; `blocked` = needs
hardware/licence/infrastructure this env cannot supply.

| Test | Status | Evidence today | Remaining need |
| --- | --- | --- | --- |
| UI-T01 create blank project | partial | S13/S14/S27 verified (UIP1 recorded run); `applyProjectTemplate` ops + tests | engine-attached create→arrange run |
| UI-T02 select region, change workspaces | partial | S01 implemented (UIP2); viewport/screensets stores coded | recorded cross-workspace continuity run |
| UI-T03 1600×1000 + 1280×832 both themes | partial | tokens/provider verified (UI01 gallery run); compact drawers coded in every screen lane | recorded pass at both resolutions × themes |
| UI-T04 200% text scaling + keyboard | not_started | — | recorded pass; no zoom-scale harness |
| UI-T05 drag/fine/type/arrows/reset/undo (S04) | partial | `editing.test.ts` (22) covers gesture contracts; S04 implemented w/ sendWithStaleRetry | recorded mix-gesture + undo run |
| UI-T06 replay dropped-ack commandId | not_started | — | transport-fault harness |
| UI-T07 tempo/zoom/off-grid/loop/sample edges | partial | piano-roll unit tests cover snap/off-grid positions | recorded run across edges |
| UI-T08 space/shortcuts/IME in fields | partial | `mayUseGlobalShortcut` + editable-target/IME guards implemented | recorded field-editing run |
| UI-T09 deny mic / remove device | partial | S15 verified honest `—` states (UIP1) | real device/permission hardware run → NEEDS §8 view gap stays |
| UI-T10 record/stop/review/cancel comp/reopen | partial | S05 verified; review-box transition logic tested (recording.test.ts 10) | engine-attached take-lane run |
| UI-T11 choose phrases from 4 takes + undo | partial | S06 implemented (UIP5): planCompApply/applyCompPlan one-tx + undo wired | recorded comp run |
| UI-T12 request notes, audition, stop | partial | S02/S25 implemented (UIP4): proposal ingest + ghost notes wired; request/audition ops gated honest (NEEDS §9-14) | RequestProposalOp/PreviewLayerOp on wire; recorded audition run |
| UI-T13 accept 4 notes then remaining 4 | partial | S26/S03 implemented (UIP4): partial acceptance = one tx of selected note ids, then remainder | recorded accept run |
| UI-T14 undo accepted subset / discard ghosts | partial | implemented (UIP4): UndoOp{transaction_id} + dismiss removes only ghosts | recorded undo run |
| UI-T15 change song while generating | partial | S20 implemented (UIP4): revision-drift rescan + accept-time revalidation + disable on stale | ProposalStaleEvent on wire; recorded stale run |
| UI-T16 malformed notes/ids/velocity/time | partial | `parseNoteItem`/`parseClipItem` defensive drops tested (derive + piano-roll tests) | recorded malformed-feed run on S02 |
| UI-T17 pointer capture, lose focus, cancel, reconnect | partial | `usePointerGesture` contract tests (lost-pointer commits once, Escape cancels) | recorded draw run on S08 |
| UI-T18 piano roll/keyboard, no camera | partial | roll keyboard ops tested | recorded run on S08 |
| UI-T19 cancel/OOM/kill worker mid-job | partial | S10/S17 implemented (UIP4): job telemetry states honest; cancel gated (no CancelJob op on wire) | SubmitJob/CancelJob ops; fault harness |
| UI-T20 accept result, move folder, reopen | partial | S10 implemented (UIP4): AttachAssetOp+InsertAudioClipOp one tx; S16 implemented: provenance + missing-state rows | recorded move+reopen run |
| UI-T21 queue scene at bar boundary | partial | S07/S28 implemented (UIP5): explicit launch queue settles on native clock only | recorded queue run |
| UI-T22 all-notes-off / stop all clips busy | partial | S07 implemented (UIP5): panic/stop-all bound to real ops | busy-worker recorded run |
| UI-T23 visuals + external display | partial | S09 implemented; output state machine tested (19 cases) | display-connected run (display enum deferred — UIP6 gap table) |
| UI-T24 kill renderer / GPU overload | partial | S09 honesty states coded | fault-injection run |
| UI-T25 reopen w/ unavailable/quarantined plugin | partial | `parseDeviceFromItem` surfaces failed/quarantined verbatim | recorded plugin scan run |
| UI-T26 engine kill / in-process crash | not_started | — | fault harness |
| UI-T27 no space / denied write / vanishing dest | not_started | — | fs-fault harness |
| UI-T28 export fixture, compare bytes | partial | S11 spec build/validate tested; S12 honest read | runner seam absent (UIP3 gap 1) — blocked on submit op |
| UI-T29 disconnect routing/asset/plugin in preflight | partial | `runPreflight` executable tests (7) cover blockers | recorded dialog run |
| UI-T30 large fixture, long session | not_started | — | perf harness + fixture |
| UI-T31 reload UI during playback | not_started | — | live run |
| UI-T32 worker path escape/upload/device cmd | not_started | — | security harness (S01/S17) |
| UI-T33 screen-reader + keyboard pass | partial | a11y primitives (roles, focus-visible, aria labels) across screens | AT-driven pass on S01/S04/S08 |
| UI-T34 display quantize + roll selection map | **verified** | `derive.test.ts` 16 + `score/model.test.ts` 10; recorded Chrome run (`UIP7-score-*.png`) | — |
| UI-T35 open/focus/resize/close VST3/AU editor | partial | `OpenPluginEditorOp` real send; rejection quoted verbatim (S04) | per-OS plugin hosts — blocked on platform/licence matrix |
| UI-T36 offer update during recording/unsaved | not_started | — | updater path also unsigned in env (tauri signing gap) |

## Unpictured W26–W29 coverage statement

The S-screen map is pictured scope only. Per `WORK_PACKAGES.md` +
`docs/verification/F5/F5_RECONCILIATION.md`, the unpictured packages stay
**engine-level partial** — the UI handoff does not claim them:

- **W25 notation/interchange** — partial upstream (notation NEEDS-01/03/05;
  `.logicx` blocked NEEDS-04). This lane adds honest UI on top of the same
  state: derived view + MusicXML where implemented; engraving/score-ops/AAF
  gaps shown, not faked.
- **W26 spatial routing/delivery** — engine-level `partial` (T92/T93
  partial_linux; licensed validators + head-tracking HW → show NEEDS 1–4).
  No UI parity claimed; no screen fakes it.
- **W27 sync/show (MIDI/DMX/OSC endpoints, Ableton Link)** — engine-level
  `partial` (T94–T96 pass_linux; NEEDS 5–10). No UI surface exists or is
  claimed.
- **W28 WASM extensions / advanced synthesis** — engine-level `partial`
  (T97 pass, T98 partial; wasm N1–N5 gates). No UI claim.
- **W29 reconciliation** — audit delivered upstream; integrator acceptance
  pending (F5-N15). This file is the UI-side contribution, not a parity
  declaration.

Nothing in `tracking/` or `screens/` marks unpictured work complete.
