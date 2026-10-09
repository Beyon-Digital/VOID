# NEEDS — protocol/schema gaps found by the engine lane

Owner: `protocol/` is integrator-owned; this file only records what the engine
needed vs what exists in `void_control.fbs` as of `devin/void-implementation`.

> **rev-2 status (protocol minor 1):** items 3, 7, 8, 9, 10, 11, 12, 13, 14,
> 17, 26 and 32 now have wire representation — see the per-item LANDED notes.
> Ops validate in `crates/void-protocol::validate`, encode/decode in the Tauri
> codec (`apps/void-tauri/src-tauri/src/codec.rs`) and echo through
> `void-mock-worker`. Engine-side application remains the engine lane's work.

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
   **LANDED (minor 1):** `SaveProjectAsOp{container_dir, name?, reason?}` —
   checkpoint into a NEW container; the source container is untouched.

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
   **LANDED (minor 1):** `ArmTrackOp{track_id, record_enabled, input_device,
   monitor_mode, is_midi}` (monitor mode is a `MonitorMode` enum
   OFF/AUTOMATIC/ON, not a bare string), `StartRecordingOp{take_id?}`,
   `StopRecordingOp{discard}`, `SetCountInOp{mode, bars}` (mode 'off'|'bars'),
   `SetMetronomeOp{enabled, gain, recording_only}`,
   `SetPunchInOutOp{enabled, in_ticks, out_ticks}` (carries the punch window,
   so a separate `PunchOp` was not added), `TAKE_LIST` view,
   `InputDeviceLostEvent{project_id, device_id, device_name}` telemetry.

8. **No device-enumeration view.** `arm` needs the engine's input device list
   (names, channel configs, latencies) — an `INPUT_DEVICE_LIST` ViewKind would
   let the supervisor build the arm UI without a device-manager round trip.
   **LANDED (minor 1):** `ViewKind::INPUT_DEVICE_LIST`.

## W12 AI-jobs lane (protocol major.1 gaps — runner drives these in-process)

9. **No `SubmitJobOp` / `CancelJobOp` in the PersistentOp union.** The AI job
   lifecycle (CONTRACTS §6) has no wire ops: `WorkerKind::AI_JOB` exists but
   nothing can enqueue or cancel a job. Needed (names proposed, not
   implemented): `SubmitJobOp{spec}` (the job/1.0.0 envelope verbatim),
   `CancelJobOp{job_id}`. Until then jobs are driven in-process by the
   `void-jobs` runner and the studio builds the intended payload shapes in
   `packages/void-studio/src/jobs/job.ts` (`submitJobOp`/`cancelJobOp`).
   **LANDED (minor 1):** `SubmitJobOp{spec}` (carries the job/1.0.0 envelope
   as a `JobSpec` table), `CancelJobOp{job_id}`, `PauseJobOp{job_id}`,
   `InstallModelOp{model_id, model_version, source_uri}` (model-registry
   install for the MODEL_LIST surface).

10. **No `JOB_LIST` / `MODEL_LIST` ViewKind.** The job list (cards, budget
    badges, progress) and the model registry read view cannot be fetched over
    the socket. Studio parses both defensively (`parseJobCard`,
    `parseModelRow`) so the shapes are pinned — the coordinator only needs to
    emit them. `jobListRequest`/`modelListRequest` carry the proposed params.
    **LANDED (minor 1):** `ViewKind::JOB_LIST` / `MODEL_LIST`, plus a
    `ReadRequest.include_terminal` field mirroring the pinned
    `jobListRequest(includeTerminal)` param.

11. **No `JobEvent` telemetry union member.** Runner progress lines
    (`{"v":1,"kind":"progress",...}`) and status transitions have no
    `TelemetryEvent` representation; studio expects
    `{kind:"JobEvent", project_id, job_id, status, percent?, message?,
    quarantined?}` (snake_case — `parseJobEvent` already reads it and
    `void-jobs::JobEvent` serializes exactly that shape).
    **LANDED (minor 1):** `TelemetryEvent::JobEvent` emits exactly that JSON
    shape; `percent` is always present (<0 = no progress reported).

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
    **LANDED (minor 1):** `PreviewLayerOp{proposal_id, clip_id, notes[],
    enable}` with a `PreviewNote` table — the wire op is in place; the
    engine-side transient layer it drives is still the engine lane's work.

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
    **LANDED (minor 1):** `ViewKind::PROPOSAL_LIST`,
    `RequestProposalOp{context_digest, seed?, max_proposals}` and
    `ResolveProposalOp{proposal_id, accept, candidate_rank?}`.

14. **No stale-invalidation telemetry.** Region edits must mark live
    proposals stale (T55) but nothing on the wire reports a context hash
    change keyed to proposals. Needed: a `ProposalStaleEvent{project_id,
    proposal_id, cause}` telemetry member (JobEvent already covers job
    progress itself, so only the proposal-specific edge is missing), or
    piggyback `STALE_REVISION` receipts — proposals listening on command
    receipts would catch region edits the same session makes; edits from
    other sessions still need the dedicated event.
    **LANDED (minor 1):** `TelemetryEvent::ProposalStaleEvent{project_id,
    proposal_id, cause}` (cause: 'context_changed' | 'target_gone' |
    'session_ended').
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
    **LANDED (minor 1):** `IngestAssetOp{rel_path, media_type}` and
    `RelinkAssetOp{asset_id, sha256}` (sha256 enforced hex-64 by validate).
    The hash+stage half stays coordinator-side per item 4 — the op carries
    the host path and the coordinator copies into `assets/sha256/` before
    the engine attach.

## rev2 — W17 studio lane gaps (items 18+)

18. **No per-clip fade fields.** `InsertAudioClipOp`/`TrimClipOp` carry
    no `fade_in_ticks`/`fade_out_ticks`/`fade_shape`; T66's "trim
    crossfades" and ARR-05 therefore store `FadeSpec{shape,lengthTicks}`
    as studio view-state (`arrangement/fades.ts`, `takes/comp.ts`
    seamFadePlan) applied at render time by the engine once fields land.
    Needed: `fade_in`/`fade_out` tables on clip ops + a
    `SetClipFadeOp{clip_id, edge, shape, ticks}` for post-insert edits.

19. **No clip repeat/loop flag.** Region loops materialize today as real
    duplicate clips (`arrangement/regions.ts loopClipOps`) — inspectable
    and undoable but N clips not 1 clip×N. Needed: `repeats`/`loop_length`
    on clip ops so a loop is one clip instance engine-side (ARR-03).

20. **No alias/linked-instance op.** Alias groups propagate edits
    client-side by fanning identical ops (`regions.ts aliasPropagateOps`);
    a wire `alias_group` membership on clips would let the engine keep
    shared-source edits atomic. Needed: `alias_group_id` field or
    `LinkClipsOp{clip_ids, propagate}`.

21. **No folder/stack/routing membership.** Folder tracks and sum stacks
    are view-state grouping (`types.ts TrackFolder`); the engine needs
    `SetTrackParentOp{track_id, parent_id}` or a folder flag so stacks
    can own bus routing (ARR-03/ARR-06 audio grouping is unrouted today).

22. **No group/multi-clip op atom.** ARR-06 group edits fan N identical
    ops through `groupEditOps` — correct but verbose; a
    `clips: [ids]` array field on Move/Trim/Remove would cut op count.
    Optional optimization, not a correctness gap (one transaction already
    makes it atomic).

23. **No protected-edit flag.** ARR-07 protection is enforced client-side
    only (`groups.ts assertUnprotected`) — a UI safety rail, honest.
    A document-level `locked` flag on clips/tracks/ranges would survive
    hostile scripts and other clients. Needed: `lock` fields + engine
    validation.

24. **No section/marker ops.** Sections and markers are view-state
    (`arrangement/sections.ts`); every edit they drive is real ops, but
    the section table itself never persists. Needed:
    `SetSectionOp`/`SetMarkerOp` + fields on project document.

25. **No alternative/playlist ops.** Track and project alternatives are
    saved arrangement specs in studio state
    (`arrangement/alternatives.ts`) applied via remove+insert; the
    spec's persistence needs `SaveAlternativeOp{track_id, clips}` or a
    playlist-list document area (DOC-02).

26. **No scene/launch ops.** Scene slots, quantize semantics and the
    pending→playing→stopping machine are a view model
    (`scenes/scenes.ts`); actual sample-accurate launch/stop at the
    boundary is engine scheduling. `sceneTransportOps` maps a bound
    scene onto SEEK+SET_CYCLE as the only honest wire surface today.
    Needed: `LaunchSceneOp`/`LaunchClipOp` + a launch-state view page
    (PAT-02/PAT-03).
    **LANDED (minor 1):** `LaunchSceneOp{scene_id, quantize, quantize_ticks}`,
    `LaunchClipOp{slot_id, quantize, quantize_ticks}`,
    `StopSceneOp{scene_id?, quantize, quantize_ticks}` (empty scene_id =
    stop all), `LaunchQuantize` enum (IMMEDIATE/BAR/BEAT/CUSTOM) and
    `ViewKind::SCENE_LIST` for the launch-state page.

27. **No tempo ramp.** `TempoEvent.ramp` is spec-only — `SetTempoOp`
    is a step change; glissando/ritardando needs a `ramp_to_ticks` or
    tempo-curve op (TIME-03 ramps).

28. **No note-copy op.** MIDI comp segments insert an empty MIDI clip
    (`takes/comp.ts midiSegmentClipIds`) because notes can't be cloned
    from another clip; the store marks them for a follow-up engine op.
    Needed: `CopyNotesOp{src_clip_id, dst_clip_id, region}` or
    clip-content copy semantics (REC-05 MIDI comping).

29. **No freeze/bounce render op.** Freeze is a spec + post-render clip
    swap (`arrangement/freeze.ts`); the render itself is an engine job
    like the W12 AI-job family — Needed: `RenderTrackOp{track_id,
    tail_policy}` returning an asset id + completion event (ENG-05/T68).

30. **No audio-record/flashback surface.** `takes/capture.ts` models the
    consent + retention policy client-side; the circular capture buffer
    is engine memory the wire can't expose yet. Needed: capture-arm +
    `RecoverCaptureOp` returning an asset id (REC-06).

31. **No loudness/analysis view.** Strip-silence consumes tile-level
    peaks (`fades.ts LoudnessTile`) but no view emits them. Needed: a
    `LOUDNESS`/`PEAKS` view page per asset (tile table, bounded) — the
    streaming half of T68 (TIME-06 analysis product).

32. **No plugin-state restore op.** `InsertPluginOp` has no state field —
    `crates/void-exchange::plan` preserves imported plugin state blobs
    through the missing-plugin store instead of sending them (loss
    entry `plugin/state`). Needed: `RestorePluginStateOp{plugin_instance_id,
    state_asset_id}` or a state field on insert (PLG-01/T76).
    **LANDED (minor 1):** `RestorePluginStateOp{plugin_instance_id,
    state_asset_id}` — alongside `SetPluginBypassOp{plugin_instance_id,
    bypassed}` and `RescanPluginsOp{plugin_uid?}` for the plugin-recovery
    and rescan surfaces.

33. **No plugin-host isolation on this platform.** `IsolationPolicy`
    (`crates/void-exchange::isolation`) validates bounded-buffer /
    deadline / restart-budget descriptors a native worker could enforce,
    and `requested_policy` returns typed `IsolationUnavailable` (AAX
    never; AU off-macOS; `NoIsolatedHost` when the worker is unbuilt).
    Actual crash isolation needs `native/void-plugin-worker` +
    fault-evidence on a native host (PLG-02/T75 runtime half).

34. **No plugin-format runtime hosting on Linux lane.** The registry
    records AU/VST3/CLAP descriptors + per-platform hosting history, but
    this lane cannot load any real plugin binary; arch discovery is
    descriptor-declared, not probed (PLG-03/T76 runtime half).

35. **No clip enable/offset/marker/loop ops.** Imported dawproject clips
    carrying `enable="false"`, `playStart` offsets, markers and loop
    ranges emit `dropped` loss entries in `plan_import` — the wire has
    no op for any of them (fades already tracked in item 18). Needed:
    `SetClipEnabledOp`, `SetClipOffsetOp`, `InsertMarkerOp`,
    `SetLoopRangeOp` (EXC-01/T77 apply leg).
