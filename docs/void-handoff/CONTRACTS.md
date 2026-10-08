# VOID — Native integration contracts

**Version 1.0.0.** Normative initial engineering defaults. Implement and test these at F0; expand typed operations through versioned changes, not opaque action dictionaries. These contracts are proposed specifications, not existing APIs or proven performance claims.

## 1. Identity, time and revision

Use stable opaque UUID strings for project, track, clip, note, scene, transaction, command, proposal and asset-reference identities. Persist the mapping in engine metadata and checkpoint app state. Content blobs use SHA-256 addresses; identical content may be deduplicated while distinct clip identities remain distinct. Scene slots carry `sceneId` and `trackId`; identifiers do not encode relationships.

Use signed 64-bit musical ticks at **960,000 ticks per quarter note** as the VOID protocol representation. The adapter converts to the selected engine's native musical representation; do not rewrite its internal scheduler. Sample positions are signed 64-bit integers at the engine's declared sample rate. Durations are positive; pickup positions may be negative. Large integers are **decimal strings in JSON/TypeScript DTOs**, native int64 in binary messages. Reject overflow; document nearest-sample rounding with ties away from zero and test it. Internally prefer exact rational arithmetic for conversions where possible; validate nonlinear tempo-map conversion against the engine.

`projectRevision` is a monotonic unsigned 64-bit **coordinator revision** for committed persistent edits. It is not wall time and is separate from transport/meter updates. Rust serializes revisions and publishes only an engine-acknowledged musical edit plus its corresponding app metadata. Do not independently increment revisions in React or a worker. `engineEpoch` changes on each engine process start. A stale epoch cannot address newly created engine handles. Saved stable IDs survive epochs; native pointers never cross the boundary.

A `ClockSnapshot` contains projectId, engineEpoch, timelineSample, deviceSampleCounter, sampleRate, transportState, loop bounds, tempoMapRevision, snapshot sequence and a monotonic host-clock timestamp in nanoseconds. Distinguish the looping musical timeline from the monotonic device counter. Pause/seek/loop/restart require explicit discontinuity handling; visuals interpolate only within the documented horizon and discard old-epoch snapshots. Clock synchronization/calibration is measured; system wall clock is not sample time.

## 2. Process protocol and permissions

Use FlatBuffers for the **native control/data-view schema**, with compatible generated Rust/C++ code pinned to a generator revision. JSON schemas in `schemas/` specify semantic DTO validation, not an alternative audio transport. Build a field-by-field DTO-to-binary conformance test. Version major/minor plus supported capabilities; reject unsupported major versions and unknown mutation types. Verify buffers and size limits before deserialization/use; FlatBuffers does not provide transport authentication.

Local transport: Unix-domain socket on macOS/Linux, named pipe on Windows, owned by the current launch/user with restrictive permissions. Use inherited handles or a protected one-time token, peer validation, worker identity and a launch nonce. No unauthenticated HTTP listener. Secrets never appear in command-line arguments or user logs. Each worker receives scoped read assets and write staging directories, not unrestricted home access. OS sandbox mechanisms require per-platform evidence; an IPC token alone does not stop arbitrary native code running as the same user.

Initial limits (configure and measure before changing):

| Resource | Initial policy |
|---|---|
| Control frame | 1 MiB maximum; reject oversized frames. Large snapshots are paged/chunked with hashes. |
| Pending document mutations | 128 entries maximum, one serial applying lane per project. Reject `BUSY` rather than lose a command. |
| UI view page | At most 2,000 object summaries or 512 KiB, whichever comes first. Use viewport/pagination and continuations. |
| Meter/clock publication | Coalesce to at most 30 updates/second per UI subscription; telemetry may be dropped, persistent edits may not. Native clock precision is unchanged. |
| Waveform tile cache | 128 MiB initial shared app-cache budget; reduce under pressure. Never decode entire long assets into JS. |
| WebView durable-data projections | 32 MiB initial target budget for data caches, separate from browser/framework memory. Bound and measure retained objects. |
| AI waiting/running jobs | Four waiting jobs, one heavy worker and up to two small jobs only when resource reservations permit. Per-job RAM/VRAM required from its qualified manifest. |
| Worker health | Heartbeats on a control thread; bounded startup/job-specific deadlines and bounded restart attempts. A heartbeat is not an audio quality measurement. |

Panic/stop/note-off requires reserved native capacity or a dedicated high-priority path. Do not put it behind edit floods. Parameter previews may be coalesced while a gesture is active; final committed parameter values are acknowledged. All arithmetic is checked and all numeric controls reject NaN/Inf.

## 3. Command, acknowledgement and read contracts

A persistent command carries `protocolMajor`, `protocolMinor`, `commandId`, `transactionId`, `projectId`, `engineEpoch`, `expectedRevision`, `operation` and a **typed payload**. The schema's initial operations are bootstrap coverage; future operations need their own validators, capability tests, undo semantics and versioning.

Processing sequence: authenticate → validate frame/schema/capability → match project/epoch → validate expected revision and targets → deduplicate → prepare off callback → apply one engine transaction → coordinate app metadata → publish receipt/revision/bounded delta. Recheck final target existence before apply. Never directly mutate an Edit from an arbitrary async thread; follow pinned JUCE/engine threading requirements.

Document acknowledgements distinguish:

| Status | Meaning |
|---|---|
| `APPLIED` | Engine/coordinator applied the transaction at revision R. This alone is **not** a durable save acknowledgement. |
| `DUPLICATE` | Same command ID and exact canonical payload hash already applied; return its original receipt without another edit. |
| `REJECTED` | Validation/conflict/capability failure; no visible committed mutation. |
| `OUTCOME_UNKNOWN` | Crash/disconnect left no verified outcome; query receipts/checkpoint and reconcile before retry. |

Repeated ID with changed payload returns `COMMAND_ID_REUSE`. Within an epoch, retain receipts sufficiently to prevent repeated execution for the active session. Persist committed receipt records with checkpoints. After restart, use recovered receipts; commands newer than the verified checkpoint have **no exactly-once guarantee**. Return unknown/recovery information; never silently replay an old request against new state. A user may explicitly reapply recovered unsaved intent through a new command after reconciliation.

Error vocabulary: `BAD_REQUEST`, `UNSUPPORTED_VERSION`, `UNSUPPORTED_CAPABILITY`, `NOT_AUTHORIZED`, `NOT_FOUND`, `STALE_REVISION`, `STALE_EPOCH`, `COMMAND_ID_REUSE`, `BUSY`, `DEVICE_UNAVAILABLE`, `ASSET_MISSING`, `PLUGIN_UNAVAILABLE`, `DISK_FULL`, `WORKER_FAILED`, `CANCELLED`, `OUTCOME_UNKNOWN`. Include safe message and relevant revision/ID, not raw paths/secrets/native stack data.

Transport requests are separate scoped control operations (play, stop, seek, panic) with request IDs/sequence, not fake persistent song edits. A render request references an immutable checkpoint or an explicitly captured revision. UI read projections use cursor/viewport filters; resubscribe with snapshot at R then ordered deltas newer than R. A gap forces bounded resync. Never run the UI render loop at the native audio rate.

## 4. Project container and atomic publication

Initial format: a local project directory with a `.void` suffix. Do not claim compatibility with native `.logicx`.

```text
song.void/
  project.json                         Stable project identity/format metadata
  CURRENT                              Atomic pointer to checkpoint ID + manifest hash
  assets/sha256/<hash>.<type>           Immutable, verified source/output blobs
  checkpoints/<checkpointId>/
    manifest.json                      Complete file/asset/revision/receipt inventory
    engine.tracktionedit               Selected engine's native serialization
    app-state.json                     Visual/cue metadata, accepted provenance/history cursor
    command-receipts.json               Receipts committed at/before this checkpoint
  staging/<transactionId>/             Incomplete imports/checkpoints; never authoritative
  recordings/<takeId>/                Growing take chunks + recoverable recording journal
```

App-private SQLite indexes recent projects, assets, worker jobs and checkpoint status. **Complete verified checkpoint manifests are the authority for a saved project.** `CURRENT` is the publication pointer; the DB is reconciled to that pointer and can rebuild the committed-project index. App jobs can use normal SQLite transactions but they cannot atomically commit external engine/media files. Use SQLite's proper backup API/checkpointed snapshots for export; never sync a live WAL database or place it on an unqualified network filesystem. Select a supported patched runtime and verify the actually linked build.

Save state machine:

1. Acquire the persistent-mutation coordinator barrier, not the audio callback lock. Capture engine/app state and receipt boundary at R. Audio continues from its native graph. Later edits may resume once capture is immutable.
2. Write engine snapshot, app state, receipts and all new referenced immutable assets under a unique staging ID. Flush file contents; validate format, size, hashes and references.
3. Write the complete manifest for R and flush it. Move the prepared directory into `checkpoints/` on the **same local filesystem**; use platform-correct durability/rename primitives. Test Windows file handles and directory flush behavior instead of assuming POSIX rules.
4. Atomically replace `CURRENT` with `{checkpointId, manifestSha256}` and make the pointer durable. Only now return `SAVE_DURABLE(R)`.
5. Reconcile/update the app DB index and prune later using reachability, retention and live-job references. Never delete last verified checkpoint or assets referenced by alternatives/history/jobs in use.

Startup: verify pointer/manifest/files. Reconcile DB to valid pointer. If pointer is missing/corrupt, offer a verified prior checkpoint based on explicit lineage/publication records; do not silently select newest mtime or promote an uncommitted staging save. Orphan complete checkpoints may be offered as recovery candidates, explicitly labeled. Quarantine incomplete staging and salvage valid recording chunks. Power-loss durability depends on filesystem/OS/device guarantees; test and publish the supported storage policy.

Schema migration creates a new checkpoint and preserves old bytes. Refuse write-open for unsupported newer versions. Portable archive export copies a complete checkpoint and referenced assets plus safe metadata, never live locks/WAL/staging secrets. Relink verifies expected hash or requires explicit user replacement.

## 5. History and cross-domain edits

Reuse Tracktion's native musical undo transactions. Rust owns an ordered user-facing history of transaction IDs that reference engine undo tokens and companion app-state changes; it does not implement an independent inverse musical engine. One drag or accepted proposal equals one transaction. Audition and meter updates do not enter document history.

F1 persistent history requirement: save/reopen preserves **musical result, alternatives, provenance and recovery checkpoints**. Do not promise restoration of the entire transient engine undo stack across restart unless the selected engine and tested implementation actually support it. After reopen show the documented undo boundary. Historical project checkpoints remain recoverable independently of an in-memory Ctrl-Z stack.

F4 audio/visual transaction: validate/prepare both participants off callback; journal a coordinator intent; commit under serialized mutation control; publish the new revision only when both agree. On a participant failure, compensate using its prepared/native undo token or restore the verified checkpoint with an explicit recovery state. Block further mutations while unresolved. Do not label this a distributed atomic transaction; test each failure boundary and expose uncertainty. Joint save captures matched participant state at the same coordinated revision.

## 6. AI proposal and worker job contracts

Job fields: ID, project ID, source revision and context hash, task kind, approved model/runtime IDs and hashes, scoped input-asset hashes, validated parameters, resource reservations, output staging directory token, deadline and cloud-consent reference when applicable. Worker manifests state allowed task kinds, supported hardware, rights, size limits and required resources. No arbitrary shell arguments, downloaded executable or unrestricted remote model code.

State machine: `queued → running → succeeded | failed | cancelling → cancelled`. Queued jobs may be cancelled directly. Terminal result is singular; cancellation may have a worker-specific grace period then process termination and staging cleanup. Timeout/crash releases reservations. Results arriving after cancellation, wrong epoch/project or expired acceptance context are discarded/quarantined, not applied.

A result contains immutable asset hashes or typed candidate notes/automation, model/runtime revision, input hashes, parameters, seed where applicable, licence/provenance and measured warnings. Success means verified output exists; it does not mean accepted into the song. Seed/config recording is not a promise of deterministic output across runtimes/hardware.

Proposal includes ID, project/sourceRevision/contextHash, scope (tracks/regions/time range), locked regions, typed operations and output assets. Audition uses a bounded **transient** engine layer excluded from checkpoints; it is removed on reject, context switch, failure and proposal expiry. Accept revalidates permissions/targets/revision/asset hashes and applies only a selected subset in one normal edit transaction. Revalidation/rebase creates a new proposal tied to new context; never silently mutate the old one.

Memory is inspectable/deletable and project-scoped by default, with provenance and opt-in broader preferences. Keep ordinary music usable when jobs/models/cloud are missing. Physical show actions and privileged app actions are not AI proposal capabilities.

## 7. Plugin, visual and export contracts

**Scanner ≠ isolated processor.** Scanner runs separately from F0 and quarantines hang/crash fixtures. Initially a loaded plugin runs inside the engine process: its failure may stop all music. Runtime processing isolation is separate W20 work with preallocated native audio/event buffers, deadlines, bounded bridge depth, declared latency, state restore and fallback. No PCM crosses Tauri's frontend bridge. Validate actual real-time behavior; native plugin code remains potentially hostile, and separate processes do not contain all OS/GPU/device faults.

Visuals use explicit beat-locked or absolute-time anchors, with rational frame rate (e.g. numerator 30000, denominator 1001) and calibrated output latency. Distinguish UI preview from program output. On overload drop/degrade visuals rather than delay audio. Start wgpu output separately; projectM is an optional distinct OpenGL integration, not an automatic compatible node.

Render/export inputs are immutable checkpoint ID, asset hashes, selected timeline range, format/channel layout, sample/frame rate, bit depth, tail policy and approved codec/model configuration. Jobs stage output and publish only on verified completion. Use argv-only process invocation and scoped filenames, not shell interpolation. Frame rounding/tail duration must be explicit. Characterize nondeterministic plugins/GPU/models; a deterministic builtin test fixture does not make all renders bit-identical.

## 8. Schema use

The included JSON schemas and examples validate the **initial semantic envelopes** and checkpoint/evidence formats. They are not a complete implementation of the 116-feature API. Extend operation payloads in the protocol task before use; run JSON/binary conformance and backward-compatibility fixtures in CI. Preserve schema IDs and increment versions for breaking meaning changes. Never ship an `any[]` actions escape hatch because a later capability lacks a typed operation.
