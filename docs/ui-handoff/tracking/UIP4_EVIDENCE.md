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

## P2 lane (void-p2-ungate) — NEEDS gates now live on rev-2 wire

Ungated every deferred affordance onto the shipped protocol. All sends go
through `sendCommand` on the real P1 op surface; DUPLICATE receipts are
benign, REJECTED/OUTCOME_UNKNOWN surface verbatim — nothing repaints ahead
of engine truth.

- **Record (S05)**: StartRecordingOp{take_id:''} / StopRecordingOp{discard},
  ArmTrackOp{record_enabled, monitor_mode, input_device, is_midi},
  SetCountInOp{mode:'off'|'bars'}, SetMetronomeOp; INPUT_DEVICE_LIST populates
  the input select.
- **Compose lifecycle (S20–S22)**: RequestProposalOp{context_digest=SHA-256,
  seed, max_proposals}, PreviewLayerOp + PreviewNote for audition toggle,
  ResolveProposalOp{accept, candidate_rank} on accept AND on dismiss;
  ProposalStaleEvent marks records stale on telemetry.
- **Jobs (S23)**: SubmitJobOp / CancelJobOp / PauseJobOp / InstallModelOp;
  JOB_LIST(include_terminal), MODEL_LIST, PROPOSAL_LIST read on bind via
  readViewPages; JobEvent frames drive the same store paths.
- **Generate (S21)**: submit_builds a JobSpecEnvelope{kind:'audio_generation'}
  and sends SubmitJobOp; REJECTED flashes the coordinator reason.
- **Perform (S17)**: LaunchSceneOp / StopSceneOp{scene_id:'' for panic} /
  LaunchClipOp with quantize mapped to the enum; REJECTED surfaces and
  skips the local queue unless a region-bound fallback already wires
  SEEK+SET_CYCLE.
- **Plugin recovery (S18)**: RescanPluginsOp{plugin_uid:''} and
  SetPluginBypassOp{plugin_instance_id, bypassed:false}.
- **Library (S07) + visuals ingest (S09)**: IngestAssetOp{rel_path,
  media_type} via a path field — the coordinator stages the file and the
  list repaints.
- **Export (S26)**: validated spec → exportJobEnvelope →
  SubmitJobOp{kind:'av_export'}; contextSha256 = checkpoint manifest sha,
  runtimeSha256 left for the coordinator to pin (never fabricated).
- **Save recovery (S25)**: SaveProjectAsOp{container_dir, name, reason}
  behind a destination field.

Still honestly gated: visual layer/channel commands (no voidvis op exists),
plugin rescan of a single uid only (full rescan wired), monitor OFF when the
coordinator rejects AUTOMATIC (status rides ArmTrackOp).
