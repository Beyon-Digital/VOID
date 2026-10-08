# NEEDS — protocol/schema gaps found by the engine lane

Owner: `protocol/` is integrator-owned; this file only records what the engine
needed vs what exists in `void_control.fbs` as of `devin/void-implementation`.

1. **`expected_revision` has no "skip" sentinel.** It's `ulong` with no opt-out
   value, so the gate is a hard `== revision_` match — a client that only wants
   best-effort application must track the exact revision. Documented behaviour,
   workable, but a `expected_revision = 0` means "project at revision 0" and
   there is no way to say "don't care". Consider `optional ulong` or an explicit
   `any = 0xFFFFFFFFFFFFFFFF` convention.

2. **No `name` on `InsertMidiClipOp`.** Clip naming needs a follow-up
   `SetClipName`-style op or an added field; engine currently auto-names clips
   `clip-<n>`.

3. **`SaveProjectOp` carries only `reason`** — no target directory field, so the
   checkpoint always lands in the container created by `CreateProjectOp` /
   `OpenProjectOp`. That's correct per CONTRACTS §4 (container-chosen), just
   noting there's no "save-as" / export path in schema.

4. **`InsertAudioClipOp` takes `rel_path` + `asset_id`** — assets must be staged
   in `container/assets/sha256/` *before* the op; there is no op to ingest/upload
   an asset over the wire (intentional: no bulk data on the socket). The
   supervisor must copy files into the container first. Documented here so the
   coordinator lane knows ingestion is its job.

5. **No op to read back undo/redo depth or list checkpoints.** `ViewKind` reads
   cover PROJECT_SUMMARY..RECEIPT_LIST; a CHECKPOINT_LIST view would let the UI
   show save history without touching persistence internals. Worked around with
   RECEIPT_LIST.

6. **`MeterFrame` has no per-track meter array** in this schema rev — engine
   publishes peak/RMS pairs the schema supports; per-track meters need a schema
   field if the mixer wants them (check with integrator before assuming).

## W08 recording lane (protocol major.1 gaps — engine drives these in-process)

7. **No arm/record/monitor/punch ops exist in `void_control.fbs`.** The take
   lifecycle — input select + record-enable, monitor mode, count-in,
   metronome, punch-in/out — has zero wire representation. Recording is
   currently engine-internal (`src/recording/RecordingManager`, exercised via
   `void-recording-fixture`). Needed (names proposed, not implemented):
   `ArmTrackOp{track, input_device, monitor_mode, is_midi}`,
   `StartRecordingOp{take_id?}`, `StopRecordingOp{discard}`,
   `PunchOp{track, enable}`, `SetCountInOp{mode}`,
   `SetMetronomeOp{enabled, gain, recording_only}`,
   `SetPunchInOutOp{enabled}`, `RecordArmView`/`TakeListView` view kinds,
   `InputDeviceLostEvent` telemetry. Until schema rev 2 the supervisor cannot
   drive recording — W08 evidence is produced in-process only.

8. **No device-enumeration view.** `arm` needs the engine's input device list
   (names, channel configs, latencies) — an `INPUT_DEVICE_LIST` ViewKind would
   let the supervisor build the arm UI without a device-manager round trip.

## W12 AI-jobs lane (protocol major.1 gaps — runner drives these in-process)

9. **No `SubmitJobOp` / `CancelJobOp` in the PersistentOp union.** The AI job
   lifecycle (CONTRACTS §6) has no wire ops: `WorkerKind::AI_JOB` exists but
   nothing can enqueue or cancel a job. Needed (names proposed, not
   implemented): `SubmitJobOp{spec}` (the job/1.0.0 envelope verbatim),
   `CancelJobOp{job_id}`. Until then jobs are driven in-process by the
   `void-jobs` runner and the studio builds the intended payload shapes in
   `packages/void-studio/src/jobs/job.ts` (`submitJobOp`/`cancelJobOp`).

10. **No `JOB_LIST` / `MODEL_LIST` ViewKind.** The job list (cards, budget
    badges, progress) and the model registry read view cannot be fetched over
    the socket. Studio parses both defensively (`parseJobCard`,
    `parseModelRow`) so the shapes are pinned — the coordinator only needs to
    emit them. `jobListRequest`/`modelListRequest` carry the proposed params.

11. **No `JobEvent` telemetry union member.** Runner progress lines
    (`{"v":1,"kind":"progress",...}`) and status transitions have no
    `TelemetryEvent` representation; studio expects
    `{kind:"JobEvent", project_id, job_id, status, percent?, message?,
    quarantined?}` (snake_case — `parseJobEvent` already reads it and
    `void-jobs::JobEvent` serializes exactly that shape).

## W13 predictive-composition lane (protocol major.1 gaps — proposals drive in-process)

12. **No transient audition / ghost-note layer.** W13 wants candidates
    auditioned in a transient engine layer excluded from persistent
    snapshots (ghost notes that never become saved edits until accepted).
    There is no `WorkerKind` for a preview layer and no view for it, so
    ghost notes currently live only in the WebView-side view store
    (`packages/void-studio/src/proposals/ghost.ts`, `ghost:true`
    markers) and playback audition is impossible without a real engine
    layer. Needed (names proposed, not implemented):
    `PreviewLayerOp{proposal_id, clip_id, notes[], enable}` — a
    non-persistent insert addressed by id, torn down on accept/reject/
    session end; or a `PREVIEW_LAYER` ViewKind if the coordinator wants
    it display-only. Until then "audition" renders visually but cannot
    sound — noted as a gap, not silently faked.

13. **No `PROPOSAL_LIST` ViewKind and no proposal lifecycle wire
    commands.** Proposal records (pending→ready→accepted/rejected/stale,
    provenance attached) cannot be read or mutated over the socket —
    generation requests, the list read, and accept/reject confirmations
    all stay in-process via `void-proposals`. Needed:
    `PROPOSAL_LIST` ViewKind (record shape mirrors
    `crates/void-proposals::ProposalRecord`, already camelCase serde),
    `RequestProposalOp{context_digest, seed?, max_proposals}` (enqueues
    the symbolic job), `ResolveProposalOp{proposal_id, accept|reject}`
    for terminal transitions. The studio parses records defensively
    (`parseProposalRecord`) so the shape is pinned.

14. **No stale-invalidation telemetry.** Region edits must mark live
    proposals stale (T55) but nothing on the wire reports a context hash
    change keyed to proposals. Needed: a `ProposalStaleEvent{project_id,
    proposal_id, cause}` telemetry member (JobEvent already covers job
    progress itself, so only the proposal-specific edge is missing), or
    piggyback `STALE_REVISION` receipts — proposals listening on command
    receipts would catch region edits the same session makes; edits from
    other sessions still need the dedicated event.
||||||| a505956d

## W11-SUPPORT lane (protocol major.1 gaps — UI/coordinator-side shell features)

15. **No `SetProjectNoteOp` / `SetTrackNoteOp` in the PersistentOp union.**
    Project and track notes (WORK_PACKAGES W11, FEATURE_MAP DOC-05) have no
    wire op, so a note typed in the UI cannot be committed to the document.
    Studio ships the proposed op shapes in
    `packages/void-studio/src/notes/dto.ts` (`proposedSetProjectNoteOp`,
    `proposedSetTrackNoteOp`, `NOTE_OPS_AVAILABLE = false`) and keeps typed
    notes as explicitly-labelled *unsent-intent drafts* (app-local, never
    shown as document values). `notes/parse.ts` already reads `note`/`notes`
    fields defensively out of PROJECT_SUMMARY / TRACK_LIST summaries so a
    view field lands as soon as the coordinator emits one. Needed:
    `SetProjectNoteOp{text}`, `SetTrackNoteOp{track_id, text}` (or a single
    `SetNoteOp{target}`), plus a `note` field on the summary payloads.

16. **No coordinator surface for the recent-projects index.** CONTRACTS §4
    puts the recents list in coordinator-owned app-private SQLite, but no op,
    view, or invoke exposes it — `OpenProjectOp` can reopen a container yet
    nothing records or returns "recently opened". Until a `RECENTS_LIST`
    view (or a `list_recent_projects` invoke) exists,
    `packages/void-studio/src/recents/` keeps an app-local cache of *real
    opens from this install* through an injected KeyValueStore — reopen
    tokens only (container dir, project id, name, timestamp), written only
    after an APPLIED create/open receipt. It is a cache, not the authority;
    stale entries surface as failed opens, never edited to look right.

17. **No asset ingest / relink-commit op.** Related to item 4: a missing
    asset renders as a `MediaLink::Missing` placeholder, and `AttachAssetOp`
    only registers a blob that is already inside `container/assets/sha256/`.
    The recovery path — the user picks a file, its bytes are hashed and
    placed into the container, then the link resolves — has no wire
    representation. `packages/void-studio/src/relink/` does the client-side
    sha256 verify (`verifyCandidate` → relink vs explicit-replace, mirroring
    `void-assets::link.rs`) and builds the `AttachAssetOp`; the coordinator
    still needs the ingest half (`IngestAssetOp{rel_path}` / a file-copy
    invoke, then the attach) and a `RelinkAssetOp{asset_id, sha256}` that
    performs the `relink()`/`replace()` transitions server-side so the UI
    does not coordinate the two steps blind.
