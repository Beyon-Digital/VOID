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
