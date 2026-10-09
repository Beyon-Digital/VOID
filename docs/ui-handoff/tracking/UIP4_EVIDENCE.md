# UIP4 evidence — proposal lifecycle, gestures, generate, jobs

Lane: `devin/void-lane-uip4` (base `79ef7f22fe68142eb498677e911176ee77353db2` on `devin/void-ui`).

## What landed

- **Ingest module** (`packages/void-studio/src/ingest/`): defensive routers
  fold `proposal`/`proposal_list`/`proposal_removed` control events into the
  proposals store, `model_list` into the model registry rows, and `JobEvent`
  telemetry into the jobs + generation stores. Malformed payloads are dropped,
  never guessed (typed validation before engine mutation — UI-T12).
- **Staleness** (`ingest/stale.ts`): `rescanStaleness` marks drifted live
  records `context_changed` on every revision advance; `latestContextSha`
  detects superseded context; `targetClipExists` revalidates the target from
  a real parsed CLIP_LIST at accept time. `session_ended` staleness on
  engine-lost (UI-T15).
- **Accept orchestration** (`ingest/accept.ts`): `acceptProposalCandidate`
  = `planAcceptLocal` (project/target/context revalidation) →
  `applyAcceptPlan` — one InsertNoteOp per selected ghost sharing one
  `transactionId` through the real `sendCommand` pipeline; `markAccepted`
  records the transaction; `undoAcceptedTransaction` issues one
  `UndoOp{transaction_id}` — one accepted transaction = one undo (UI-T13/T16).
  `dismissProposal` removes ghosts only (UI-T16).
- **Compose** (`screens/compose/`): TrackListColumn + roll + suggestion
  inspector. While a live proposal exists the roll is `SuggestionRoll` —
  real NOTE_RANGE notes locked/muted, ghost notes dashed and
  per-index-toggleable for partial accept (S26), locked-range ghosts muted
  and non-selectable, proposal-region divider, velocity lane. Stale records
  still paint their preview frozen/non-interactive with accept refused (S20).
  Inspector (`uip4/SuggestionPanel`): request context echo, ranked
  `ProposalCard`s from real records, Accept-all/Accept-N-selected/"Keep my
  original", Committed card with real undo, stale card with honest refresh
  state. Audition button is disabled with the reason — no PreviewLayerOp in
  this build (S25 honest, NEEDS §12).
- **Gesture** (`screens/gesture/`, S08): `GesturePad` (contour, armed first —
  pointer capture is the component's) and `TapPad` (rhythm). Raw points feed
  `gestureSession`; `endCapture` renders the snapped preview via
  `makeContourMapping(viewport, zoom)` / rhythm options with project bpm.
  Quantize strength + grid are reversible `setTransform` re-renders; raw is
  shown next to snapped in the inspector. Commit → `commitGesture` (real
  InsertNoteOps, one transaction, `sendWithStaleRetry`); undo →
  `undoGestureCommit`. Escape + `visibilitychange`/`blur` release armed or
  capturing sessions (UI-T17); the piano-roll alternative is always linked
  (UI-T18); camera and XY-expression are marked unavailable (UI-T18/T32).
- **Generate** (`screens/generate/`, S10): prompt/duration/model form —
  submit gated `disabled` with the SubmitJob reason (NEEDS §9). Candidates
  list real generation records; "Use this audio" sends real
  AttachAssetOp + InsertAudioClipOp under one transaction, marks
  `acceptedSha256` (UI-T20). Audition gated — no preview layer (NEEDS §12).
- **Jobs** (`screens/jobs/`, S17): filters over real JobCards via `JobRow`
  (status/percent/message/budgets/quarantine verbatim), active+recent
  sections, model registry via `ModelRegistryList` fed by `model_list`
  control events. Cancel/pause/install/submit affordances are disabled with
  reasons (NEEDS §9–11) — no invented capability flags (UI-T19/T32/T36).

## Gates (commands + exit codes)

| Command | Exit | Notes |
| --- | --- | --- |
| `pnpm install --frozen-lockfile` | 0 | pre-edit |
| `npx tsc --noEmit` (packages/void-studio) | 0 | ingest module |
| `npx vitest run src/ingest/` (void-studio) | 0 | 16 new tests pass |
| `npx tsc --noEmit` (apps/void-tauri) | 0 | screens + wiring |
| `npx vite build` (apps/void-tauri) | 0 | 292 modules |
| `pnpm -r --if-present test` | 0 | 478 vitest green |
| `pnpm -r --if-present build` | 0 packages / void-tauri bundling fails at updater signing | pre-existing env gap (TAURI_SIGNING_PRIVATE_KEY unset) — same as baseline |
| `cargo test --workspace --exclude void-tauri` | — | no rust touched |

## Honest gaps (NEEDS-tracked, not fabricated)

- Proposal **request** (RequestProposalOp), **resolve** (ResolveProposalOp),
  **audition** (PreviewLayerOp), **stale push** (ProposalStaleEvent) —
  NEEDS.md §12–14: screens show these disabled with the reason; staleness is
  detected locally from revision/context instead of an event.
- Job **submit/cancel/pause** + **JOB_LIST/MODEL_LIST** views + **model
  install** — NEEDS.md §9–11: gated affordances; lists fill from real pushed
  events only.
- **Asset/candidate audition** playback — NEEDS §12: honest disabled state.
- No seeded/demo data anywhere in production UI.
