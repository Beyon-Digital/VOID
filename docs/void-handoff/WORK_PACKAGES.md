# VOID — Work packages

Generated from `tracking/TASKS.json`. Paths outside packet folders are proposed repository targets. All packages start `not_started`; no completion is inferred from a report.

## Dependency map

| ID | Gate | Package | Prerequisites | Status |
| --- | --- | --- | --- | --- |
| W00 | F0 | Baseline, source preservation and scope ledger | None | done |
| W01 | F0 | Dependency rights and compatible version qualification | W00 | done |
| W02 | F0 | Tauri shell and supervised native worker | W01 | done |
| W03 | F0 | Command protocol, object IDs and bounded state views | W02 | done |
| W04 | F0 | Tracktion musical model, device path and native playback | W03 | partial_macos |
| W05 | F0 | Immutable assets and crash-consistent checkpoints | W04 | done |
| W06 | F0 | Native plugin proof and safe scanning | W05 | partial_macos |
| W07 | F0 | CI, diagnostics, clean install and Foundation gate | W06 | partial |
| W08 | F1 | Audio/MIDI recording and monitoring workflow | W07 | partial_macos |
| W09 | F1 | Arrangement and piano-roll editors | W08 | done |
| W10 | F1 | Playable instruments, real mixer and export | W09 | partial |
| W11 | F1 | First-song qualification and Electron cutover | W10 | not_started |
| W12 | F2 | AI job runtime, budgets and provenance | W11 | partial |
| W13 | F2 | Predictive composition and safe proposal transactions | W12 | in_progress_lane |
| W14 | F2 | Gestures, patterns, quick sampling and harmonic controls | W13 | not_started |
| W15 | F2 | Audio generation, stems and transcription adapters | W12 | not_started |
| W16 | F2 | AI/gesture regression gate | W14, W15 | not_started |
| W17 | F3 | Recording and arrangement depth | W11 | not_started |
| W18 | F3 | Mixer, automation and advanced MIDI | W17 | not_started |
| W19 | F3 | Stock sound library, effects and time/pitch tools | W17, W01 | not_started |
| W20 | F3 | Plugin compatibility, optional isolation and exchange | W18 | not_started |
| W21 | F3 | Accompaniment, arrangement and producer gate | W16, W19, W20 | not_started |
| W22 | F4 | Conventional native visual composition and program output | W11 | not_started |
| W23 | F4 | Generated/reactive visuals and optional camera conducting | W22, W16 | not_started |
| W24 | F4 | Audiovisual export and visual qualification gate | W23 | not_started |
| W25 | F5 | Notation, scoring and advanced interchange | W21 | not_started |
| W26 | F5 | Spatial routing and delivery | W21 | not_started |
| W27 | F5 | Synchronization, modular control and safe shows | W24, W21 | not_started |
| W28 | F5 | Advanced synthesis and restricted extensions | W19, W24 | not_started |
| W29 | F5 | Full parity reconciliation and release readiness | W25, W26, W27, W28 | not_started |

## W00 · F0 · Baseline, source preservation and scope ledger

**Owner:** Integrator · **Depends:** None · **Status:** done

**Target paths:** `docs/void-handoff/`, `tracking/`, `README.md`, `migration.md`

### Implement

1. Read repository instructions, current branch, open PRs and package scripts; record the actual starting SHA and dirty/untracked changes without discarding them.

2. Install this packet under docs/void-handoff/. Preserve the reference snapshots. Reconcile any newer code by requirement ID; do not replay a migration over completed work.

3. Run the existing build/test commands with time limits; record failures honestly. Correct README claims so architecture declarations are not called implemented features.

4. Replace clip-ID-prefix scene membership with explicit sceneId/trackId relationships when migrating the SessionView; add a collision regression test.

### Done only when

- Starting SHA, environment and observed checks recorded. Every original feature is present in FEATURE_TRACEABILITY.json. Existing source/history is preserved.

**Required scenarios:** T01, T02

**Feature outcomes:** Cross-cutting foundation; see mapped infrastructure and all dependent features.

**Dependency decisions:** Reuse already qualified foundations; justify new dependencies separately.

**Evidence:** p; a; c; k; e; t;  ; i; n; s; t; a; l; l; e; d; ,;  ; b; a; s; e; l; i; n; e;  ; r; e; c; o; r; d; e; d; ,;  ; S; e; s; s; i; o; n; V; i; e; w;  ; s; c; e; n; e; I; d;  ; f; i; x;  ; +;  ; v; i; t; e; s; t;  ; 3; /; 3

## W01 · F0 · Dependency rights and compatible version qualification

**Owner:** Integrator · **Depends:** W00 · **Status:** done

**Target paths:** `docs/dependencies/`, `third_party/`, `tracking/DEPENDENCIES.json`, `Cargo.lock`, `pnpm-lock.yaml`

### Implement

1. Review the exact Tracktion/JUCE pair and upstream example builds; use C++20 where required by the selected engine, not an invented SDK API.

2. Pin exact engine/JUCE/submodule/toolchain/SDK revisions, feature flags, licences and source hashes. Start with the engine-supplied compatible JUCE revision.

3. Record a valid development-use path and a separate distribution-rights state. Do not buy licences, relicense the repository or publish binaries automatically.

4. Review advisory status including the actual linked SQLite build. Keep optional and evaluation libraries out of the initial installer.

### Done only when

- Dependency compatibility evidence exists; development may proceed only within recorded permitted use. Distribution stays blocked while rights/signing/content approval is unknown.

**Required scenarios:** T03, T04

**Feature outcomes:** Cross-cutting foundation; see mapped infrastructure and all dependent features.

**Dependency decisions:** CORE-01, CORE-02, CORE-03, DATA-01, DATA-02, DATA-03

**Evidence:** P; I; N; S; .; m; d;  ; +;  ; Q; U; A; L; I; F; I; C; A; T; I; O; N; .; m; d; ;;  ; T; E;  ; e; 7; 6; 0; 7; 5; 4; +; J; U; C; E;  ; 3; 7; c; 8; 9; 4; f; ;;  ; u; p; s; t; r; e; a; m;  ; s; u; i; t; e;  ; 4; 5; 0; /; 2; 2; 2; 8; 2;  ; e; x; i; t;  ; 0

## W02 · F0 · Tauri shell and supervised native worker

**Owner:** Platform · **Depends:** W01 · **Status:** done

**Target paths:** `apps/void-desktop/`, `crates/void-app/`, `crates/void-worker/`, `native/void-engine/`, `CMakePresets.json`, `Cargo.toml`

### Implement

1. Add the Tauri target beside the existing Electron entrypoint until F1 cutover; reuse UI tokens where useful, not the stub audio backend.

2. Build a C++ worker using the selected JUCE message loop and engine examples. Rust supervises worker identity, startup readiness, heartbeat, bounded restart and graceful shutdown.

3. Create a restrictive local control channel using Unix-domain sockets or named pipes; use per-launch secrets/inherited handles, peer checks and scoped permissions. No public or unauthenticated loopback API.

4. Keep a WebView reload distinct from closing the application. Confirm all child processes terminate after an intentional app exit.

### Done only when

- A real Rust shell starts, handshakes with and stops the C++ process. Renderer restart preserves the worker; engine restart produces a new epoch and explicit stopped/recovery state.

**Required scenarios:** T05, T06, T07

**Feature outcomes:** Cross-cutting foundation; see mapped infrastructure and all dependent features.

**Dependency decisions:** CORE-01, CORE-02, CORE-03, DATA-03

**Evidence:** T; a; u; r; i;  ; v; 2;  ; s; h; e; l; l;  ; +;  ; v; o; i; d; -; w; o; r; k; e; r;  ; s; u; p; e; r; v; i; s; o; r;  ; +;  ; m; o; c; k;  ; w; o; r; k; e; r; ;;  ; w; i; r; e; _; r; o; u; n; d; t; r; i; p;  ; g; r; e; e; n; ;;  ; t; a; u; r; i;  ; b; u; i; l; d;  ; -; >;  ; d; e; b; /; r; p; m; /; A; p; p; I; m; a; g; e

## W03 · F0 · Command protocol, object IDs and bounded state views

**Owner:** Platform + engine · **Depends:** W02 · **Status:** done

**Target paths:** `protocol/`, `crates/void-protocol/`, `native/void-engine/src/bridge/`, `packages/void-client/`

### Implement

1. Implement CONTRACTS.md and validate schemas. Generate compatible Rust/C++ FlatBuffers bindings and typed frontend DTOs; do not pass unvalidated JSON through to engine APIs.

2. Serialize document mutation off the audio callback; carry commandId/projectId/expectedRevision/engineEpoch and stable object IDs.

3. Implement duplicate-ID receipts, payload-hash conflict rejection, revision conflicts, paginated/viewport reads and snapshot-plus-delta reattachment.

4. Use bounded control queues and lossy meter telemetry. Clock, recordings and audio buffers must not travel through WebView events.

### Done only when

- Malformed, duplicate, stale and out-of-scope requests have deterministic outcomes; frontend state reattaches without loading the whole project.

**Required scenarios:** T08, T09, T10, T11, T12

**Feature outcomes:** Cross-cutting foundation; see mapped infrastructure and all dependent features.

**Dependency decisions:** DATA-02

**Evidence:** v; o; i; d; _; c; o; n; t; r; o; l; .; f; b; s;  ; f; r; o; z; e; n;  ; 1; .; 0; ;;  ; 3; 2;  ; o; p; s; ;;  ; C; o; n; t; r; o; l; E; n; v; e; l; o; p; e; ;;  ; r; e; c; e; i; p; t; /; d; e; d; u; p; /; r; e; v; i; s; i; o; n;  ; s; t; o; r; e; s; ;;  ; c; o; d; e; c;  ; f; u; l; l;  ; c; o; v; e; r; a; g; e

## W04 · F0 · Tracktion musical model, device path and native playback

**Owner:** Engine · **Depends:** W03 · **Status:** partial_macos

**Target paths:** `native/void-engine/src/session/`, `native/void-engine/src/audio/`, `native/void-engine/tests/`

### Implement

1. Instantiate one authoritative engine Edit and device manager. Map project/track/clip/note/parameter IDs with saved metadata; never derive relationships from ID prefixes.

2. Implement create project, import approved WAV, add audio/MIDI/instrument track, note insertion and transport start/stop/seek/cycle using actual engine APIs.

3. Run graph preparation, asset I/O and disposal off the audio callback. Use engine scheduling and latency facilities before inventing alternatives.

4. Build an offline render test fixture and real native output playback. A generated test tone is a test asset, not simulated user audio.

### Done only when

- Fixture playback/render works through the native graph. UI values change audible native parameters, not only local component state.

**Required scenarios:** T13, T14, T15, T16

**Feature outcomes:** ENG-01, ENG-02, TIME-01

**Dependency decisions:** CORE-02, CORE-03, DSP-03

**Evidence:** n; a; t; i; v; e; /; v; o; i; d; -; e; n; g; i; n; e;  ; l; i; v; e;  ; w; i; r; e;  ; 1; 5; /; 1; 5;  ; h; a; r; n; e; s; s; ;;  ; T; 1; 3; /; T; 1; 5; /; T; 1; 6;  ; n; e; e; d;  ; a; u; d; i; o;  ; d; e; v; i; c; e; /; G; U; I

## W05 · F0 · Immutable assets and crash-consistent checkpoints

**Owner:** Persistence · **Depends:** W04 · **Status:** done

**Target paths:** `crates/void-project/`, `crates/void-assets/`, `native/void-engine/src/persistence/`, `tests/recovery/`

### Implement

1. Implement the project container, content-hashed media, complete checkpoint manifest and CURRENT pointer defined in CONTRACTS.md.

2. Capture engine state, app metadata and coordinated revision at a serialized mutation barrier while audio continues. Write, flush, verify and atomically publish a complete checkpoint.

3. Make the checkpoint manifest the save authority; SQLite is a reconstructable index for committed checkpoints and a transactional store for app jobs. Do not claim a transaction spanning DB and files.

4. Retain previous verified checkpoints, recover uncertain commands safely and migrate older project schemas by copy-on-write. Refuse overwrite of newer unsupported formats.

### Done only when

- Crash/disk-full at every save boundary never destroys the last committed project. Save success means a complete verified published checkpoint, not queued writes.

**Required scenarios:** T17, T18, T19, T20, T21

**Feature outcomes:** DOC-01, DOC-03

**Dependency decisions:** DATA-01

**Evidence:** v; o; i; d; -; p; r; o; j; e; c; t; /; v; o; i; d; -; a; s; s; e; t; s; /; v; o; i; d; -; j; o; b; s;  ; +;  ; t; e; s; t; s; /; r; e; c; o; v; e; r; y;  ; 2; 2; /; 2; 2

## W06 · F0 · Native plugin proof and safe scanning

**Owner:** Engine · **Depends:** W05 · **Status:** partial_macos

**Target paths:** `native/void-plugin-scanner/`, `native/void-engine/src/plugins/`, `crates/void-plugins/`, `tests/plugins/`

### Implement

1. Run discovery/scan in a short-lived process with timeout and quarantine. Cache exact plugin binary/version/architecture identity.

2. Load one known legally usable VST3 fixture and exercise its native editor and state; test AU separately on macOS. Start with engine-hosted processing, clearly labeled not isolated.

3. Test editor open/close, resize, focus, DPI, keyboard and project-close teardown. Preserve unknown/missing plugin state blobs for later restoration.

4. Provoke scan hang and engine/plugin crash. Distinguish scanner failure, engine failure and future isolated runtime failure.

### Done only when

- Native plugin processing and native editor actually work on the first tested OS; other OS rows remain unverified until tested. Engine failure stops audio but leaves the Rust shell able to recover.

**Required scenarios:** T22, T23, T24

**Feature outcomes:** HOST-01, HOST-02, HOST-05

**Dependency decisions:** IO-01, QA-01

**Evidence:** v; o; i; d; -; p; l; u; g; i; n; -; s; c; a; n; n; e; r;  ; A; U; +; s; e; l; f; t; e; s; t;  ; i; s; o; l; a; t; i; o; n; ;;  ; T; 2; 3;  ; f; o; r; e; i; g; n; -; a; r; c; h;  ; b; l; o; c; k; e; d

## W07 · F0 · CI, diagnostics, clean install and Foundation gate

**Owner:** QA + release · **Depends:** W06 · **Status:** partial

**Target paths:** `.github/workflows/`, `tools/doctor/`, `tools/ci/`, `tests/e2e/`, `docs/verification/`

### Implement

1. Build on native OS runners with pinned toolchains, job timeouts, bounded matrix concurrency and cancellation of superseded branch runs.

2. Add structured redacted logs, callback/dropout counters and process-tree resource snapshots. Keep instrumentation that allocates out of real-time release callbacks.

3. Test release-like installation without a development toolchain. Use OS signing/notarization when credentials exist; unsigned internal builds must be labeled and cannot pass signed-release checks.

4. Prove UI reload + real plugin + save/reopen + offline render. Publish F0 evidence per platform; retain the Electron path until F1 is accepted.

### Done only when

- F0 software gate passes with exact commands and artifacts. Hardware/signing rows are passed only with real evidence, otherwise explicitly blocked; next nondependent work may continue.

**Required scenarios:** T25, T26, T27, T28, T29, T30

**Feature outcomes:** PRO-08

**Dependency decisions:** QA-02, QA-03

**Evidence:** c; i; .; y; m; l;  ; +;  ; d; o; c; t; o; r; .; s; h;  ; (; 8; /; 2; /; 0; );  ; +;  ; E; V; I; D; E; N; C; E; .; m; d; ;;  ; i; n; s; t; a; l; l;  ; p; a; t; h;  ; p; e; n; d; i; n; g;  ; n; a; t; i; v; e;  ; b; i; n; a; r; y;  ; o; n;  ; n; o; n; -; m; a; c

## W08 · F1 · Audio/MIDI recording and monitoring workflow

**Owner:** Engine + UI · **Depends:** W07 · **Status:** partial_macos

**Target paths:** `native/void-engine/src/recording/`, `packages/void-studio/src/recording/`, `tests/recording/`

### Implement

1. Implement input/output setup, arm, monitoring mode, count-in, metronome, cycle/punch boundaries and mono/stereo multitrack recording.

2. Capture MIDI with timestamps, sustain/bend/aftertouch, latency alignment and MIDI import/export/demix. Add all-notes-off on disconnect, stop and panic.

3. Write growing recordings into a recoverable recording journal with persisted chunk lengths; incomplete takes are recovered and labeled, not mistaken for final assets.

4. Handle denied microphone permission, device loss, rate/buffer mismatch, full disk and changing default devices without surprise feedback or false recording success.

### Done only when

- A person can record voice and MIDI offline, hear the intended monitoring path and recover a partial take after an induced failure. Hardware latency is measured, not inferred from buffer size.

**Required scenarios:** T31, T32, T33, T34, T35

**Feature outcomes:** ENG-01, ENG-03, TIME-01, TIME-02, REC-01, REC-02, MIDI-03, MIDI-07

**Dependency decisions:** CORE-02, CORE-03

**Evidence:** R; e; c; o; r; d; i; n; g; M; a; n; a; g; e; r; +; M; i; d; i; C; a; p; t; u; r; e; +; T; a; k; e; J; o; u; r; n; a; l;  ; m; e; r; g; e; d; ;;  ; t; a; k; e;  ; 2; 0; /; 2; 0; ,;  ; e; r; r; o; r; s;  ; 8; /; 8; ,;  ; k; i; l; l; -; r; e; c; o; v; e; r; y;  ; l; a; b; e; l; s;  ; i; n; c; o; m; p; l; e; t; e; ,;  ; p; a; n; i; c;  ; a; l; l; N; o; t; e; s; O; f; f; =; 1; 6; ,;  ; h; o; s; t; e; d;  ; r; o; u; n; d; T; r; i; p;  ; 2; .; 7; 2; 9; m; s; ;;  ; b; l; o; c; k; e; d; :;  ; r; e; a; l;  ; H; W;  ; l; a; t; e; n; c; y;  ; h; e; a; d; l; e; s; s;  ; +;  ; p; r; o; t; o; c; o; l;  ; h; a; s;  ; n; o;  ; r; e; c; o; r; d; i; n; g;  ; o; p; s;  ; (; N; E; E; D; S; .; m; d;  ; §; 7; -; 8;  ; p; r; o; p; o; s; e; s;  ; r; e; v; 2;  ; o; p; s; )

## W09 · F1 · Arrangement and piano-roll editors

**Owner:** Studio UI · **Depends:** W08 · **Status:** done

**Target paths:** `packages/void-studio/src/timeline/`, `packages/void-studio/src/piano-roll/`, `packages/void-ui/`

### Implement

1. Create Compose/Arrange/Mix workspaces with consistent transport and track selection. No approved Figma file exists in this packet: build a functional original UI, not a fictitious design match.

2. Implement virtualized tracks, multiresolution waveform tiles, region select/move/trim/split/duplicate, snapping and explicit scene/track slots.

3. Implement piano-roll pitch/start/duration/velocity edits and CC lanes with pointer and keyboard controls; group a drag into one history transaction.

4. Keep viewport caches bounded and reconcile optimistic feedback with authoritative acknowledgements. Implement focus, range semantics, reduced motion and empty/error states.

### Done only when

- Edits change real audio/MIDI, survive save/reopen and have coherent undo. Large arrangements do not clone all sample data into JavaScript.

**Required scenarios:** T36, T37, T38, T39

**Feature outcomes:** DOC-03, ARR-01, ARR-02, ARR-04, MIDI-01, MIDI-03, PRO-06

**Dependency decisions:** Reuse already qualified foundations; justify new dependencies separately.

**Evidence:** v; o; i; d; -; s; t; u; d; i; o;  ; t; i; m; e; l; i; n; e; /; p; i; a; n; o; -; r; o; l; l; /; w; o; r; k; s; p; a; c; e; s;  ; +;  ; v; o; i; d; -; u; i;  ; C; l; i; p; B; l; o; c; k; /; N; o; t; e; B; l; o; c; k; /; T; r; a; n; s; p; o; r; t; C; o; n; t; r; o; l; s; /; W; o; r; k; s; p; a; c; e; T; a; b; s;  ; +;  ; S; t; u; d; i; o; S; h; e; l; l; ;;  ; 6; 7;  ; v; i; t; e; s; t;  ; (; 4; 6;  ; n; e; w; ); ;;  ; w; i; r; e; -; n; a; m; e;  ; f; i; x; e; s; :;  ; M; o; v; e; C; l; i; p; O; p; /; T; r; i; m; C; l; i; p; O; p; /; R; e; m; o; v; e; C; l; i; p; O; p; ,;  ; C; L; I; P; _; L; I; S; T; ,;  ; S; e; t; N; o; t; e; O; p; +; R; e; m; o; v; e; N; o; t; e; O; p

## W10 · F1 · Playable instruments, real mixer and export

**Owner:** Engine + UI · **Depends:** W09 · **Status:** partial

**Target paths:** `packages/void-studio/src/instruments/`, `packages/void-studio/src/mixer/`, `crates/void-export/`, `native/void-engine/src/render/`

### Implement

1. Expose engine starter synth/sampler and utility processors with original/cleared presets; use existing DSP and approved sample assets.

2. Implement gain/pan/mute/solo, real meters, insert/bypass and channel presets; use safe monitoring defaults and display actual clipping.

3. Implement mono/stereo WAV and MIDI export with selected range, sample rate, bit depth, tails and a progress/cancel/failure flow.

4. Keep renders on an immutable checkpoint/snapshot; edits after render start do not silently change the output. Stage files and publish only completed results.

### Done only when

- A complete short song containing a recording, MIDI instrument, edits and mix settings renders correctly offline; no network/model requirement.

**Required scenarios:** T40, T41, T42, T43

**Feature outcomes:** MIDI-07, MIX-01, MIX-02, SND-01, FX-08, OUT-01

**Dependency decisions:** DSP-03, DSP-10, IO-01

**Evidence:** n; o; n; -; n; a; t; i; v; e;  ; d; o; n; e; :;  ; v; o; i; d; -; e; x; p; o; r; t;  ; (; A; r; g; v; R; e; n; d; e; r; e; r; ,;  ; s; t; a; g; i; n; g; -; >; v; e; r; i; f; y; -; >; p; r; o; v; e; n; a; n; c; e; -; >; p; u; b; l; i; s; h; ,;  ; c; a; n; c; e; l;  ; s; e; m; a; n; t; i; c; s; );  ; 1; 9; /; 1; 9; ;;  ; i; n; s; t; r; u; m; e; n; t; s; /; m; i; x; e; r; /; e; x; p; o; r; t;  ; s; t; u; d; i; o;  ; m; o; d; u; l; e; s;  ; 4; 0;  ; n; e; w;  ; v; i; t; e; s; t; ;;  ; m; i; x; e; r;  ; s; e; n; d; s;  ; =;  ; t; y; p; e; d;  ; U; n; s; u; p; p; o; r; t; e; d; C; a; p; a; b; i; l; i; t; y;  ; r; e; f; u; s; a; l; .;  ; N; a; t; i; v; e;  ; r; e; n; d; e; r;  ; j; o; b;  ; +;  ; j; o; u; r; n; e; y;  ; p; e; n; d; i; n; g

## W11 · F1 · First-song qualification and Electron cutover

**Owner:** Integrator + QA · **Depends:** W10 · **Status:** not_started

**Target paths:** `tests/journeys/`, `docs/verification/F1/`, `apps/void-desktop/`, `README.md`

### Implement

1. Run the full first-song journey and all failure-path tests on supported targets. Measure 30-minute workloads using the fixture and hardware policy.

2. Add templates, recent projects, project/track notes, relink recovery, help/onboarding and localizable copy.

3. Remove Electron packaging/default startup and WebAudio production routing only after native equivalence for implemented workflows is demonstrated. Preserve git history and any legacy project importer that is actually needed.

4. Publish supported configurations, limitations and status evidence. Do not label the complete Logic parity roadmap finished at this gate.

### Done only when

- F1 acceptance report proves one complete offline song and recovery. The Tauri application is the production path; remaining phases stay open in the tracker.

**Required scenarios:** T44, T45, T46, T47, T48

**Feature outcomes:** DOC-01, DOC-05, PRO-06

**Dependency decisions:** Reuse already qualified foundations; justify new dependencies separately.

**Evidence:** None yet. Record actual commands, artifacts and limitations.

## W12 · F2 · AI job runtime, budgets and provenance

**Owner:** AI platform · **Depends:** W11 · **Status:** partial

**Target paths:** `crates/void-jobs/`, `workers/`, `crates/void-models/`, `packages/void-studio/src/jobs/`

### Implement

1. Implement job state machine, bounded queue, reservation-based RAM/VRAM/CPU admission, progress, deadlines, cancellation and orphan cleanup.

2. Add signed model manifests, version/hash checks, optional downloads, inspectable project-scoped memory and deletion/retention rules.

3. Integrate one qualified compact runtime/model first; bundle a locked worker runtime rather than requiring users to install Python.

4. No hidden cloud fallback, raw private-project telemetry, arbitrary shell tool or automatic training. Store permitted inputs, model/runtime revision, settings and accepted-output provenance.

### Done only when

- Jobs may fail or be unavailable without disabling the manual studio. Cancellation releases resources and late results cannot mutate the project.

**Required scenarios:** T49, T50, T51, T52

**Feature outcomes:** INTEL-08, INTEL-09

**Dependency decisions:** AI-01, AI-02, DATA-03

**Evidence:** v; o; i; d; -; m; o; d; e; l; s;  ; r; e; g; i; s; t; r; y; +; m; a; n; i; f; e; s; t;  ; v; e; r; i; f; y; +; e; x; e; c;  ; a; l; l; o; w; l; i; s; t; ;;  ; w; o; r; k; e; r; s; /; P; R; O; T; O; C; O; L; .; m; d;  ; v; 1;  ; +;  ; v; o; i; d; -; f; a; k; e; -; w; o; r; k; e; r;  ; r; e; a; l;  ; s; y; n; t; h; ;;  ; v; o; i; d; -; j; o; b; s;  ; r; u; n; n; e; r;  ; (; R; L; I; M; I; T;  ; b; u; d; g; e; t; s; ,;  ; t; h; r; e; a; d; e; d;  ; p; u; m; p; ,;  ; a; r; t; i; f; a; c; t;  ; r; e; -; h; a; s; h; ,;  ; p; r; o; v; e; n; a; n; c; e;  ; p; u; b; l; i; s; h; ,;  ; a; d; m; i; s; s; i; o; n;  ; 4; /; 1; /; 2; ,;  ; l; a; t; e; -; r; e; s; u; l; t;  ; q; u; a; r; a; n; t; i; n; e; );  ; 1; 3; /; 1; 3; ;;  ; s; t; u; d; i; o;  ; j; o; b; s;  ; 1; 9;  ; t; e; s; t; s; .;  ; M; i; s; s; i; n; g; :;  ; w; i; r; e;  ; o; p; s;  ; (; S; u; b; m; i; t; J; o; b; /; C; a; n; c; e; l; J; o; b; /; J; O; B; _; L; I; S; T; );  ; —;  ; N; E; E; D; S; .; m; d;  ; r; e; v; 2

## W13 · F2 · Predictive composition and safe proposal transactions

**Owner:** Musical AI + UI · **Depends:** W12 · **Status:** in_progress_lane

**Target paths:** `crates/void-proposals/`, `workers/symbolic/`, `packages/void-studio/src/proposals/`

### Implement

1. Start with deterministic scale/chord/rhythm rules and optionally a compact qualified model; expose ghost notes and several candidates inside the piano roll/pads.

2. Use a scoped context digest of notes, chords, meter, key, selected bars, locked regions and instrument; never send the whole song by default.

3. Audition in a transient engine layer excluded from persistent snapshots. Support partial accept, reject, regenerate, editable accepted notes and one-step undo.

4. Reject or explicitly revalidate stale results; never let project lyrics, filenames or model outputs escalate permissions or execute arbitrary code.

### Done only when

- A user can compose with manual notes, audition a continuation, accept part of it, undo and reopen the original state; candidates do not become saved edits until accepted.

**Required scenarios:** T53, T54, T55, T56

**Feature outcomes:** INTEL-02, INTEL-04

**Dependency decisions:** AI-02, AI-05

**Evidence:** l; a; n; e;  ; s; p; a; w; n; e; d

## W14 · F2 · Gestures, patterns, quick sampling and harmonic controls

**Owner:** Studio UI + engine · **Depends:** W13 · **Status:** not_started

**Target paths:** `packages/void-studio/src/gestures/`, `packages/void-studio/src/patterns/`, `native/void-engine/src/input/`

### Implement

1. Implement pointer/touch/trackpad melodic contours and rhythmic taps, raw timing retention, reversible quantize/swing/humanize and gesture preview/commit.

2. Add step patterns, quick sample slicing, drum pads, loop browser, chord track/harmonic constraints and explicitly armed MIDI-learn/macros.

3. Provide keyboard/numeric alternatives for each gesture; gestures must not steal ordinary navigation or leave a note on.

4. Keep camera optional for F4 and advanced MIDI/MPE depth in F3; do not make camera access a prerequisite to gesture composition.

### Done only when

- Draw/tap/adjust a phrase, audition it, accept and undo using pointer and keyboard; normal editing still works with gestures/AI disabled.

**Required scenarios:** T57, T58, T59

**Feature outcomes:** TIME-05, MIDI-02, PAT-01, MIX-07, SND-02, SND-05, INTEL-02, GEST-01, GEST-02, GEST-05, GEST-06

**Dependency decisions:** DSP-03, AI-05

**Evidence:** None yet. Record actual commands, artifacts and limitations.

## W15 · F2 · Audio generation, stems and transcription adapters

**Owner:** AI workers · **Depends:** W12 · **Status:** not_started

**Target paths:** `workers/audio/`, `crates/void-assets/`, `packages/void-studio/src/generation/`, `tests/models/`

### Implement

1. Qualify separate adapters for generation, separation and audio-to-MIDI; do not treat a model name as a completed workflow.

2. Measure task quality, editability, timing/channel alignment, duration, hardware use and rights using owned/licensed fixtures.

3. Offer source/result A-B, crop/align, alternative clips, retry/cancel and acceptance; keep originals immutable and generated media normal editable assets.

4. Demucs/Basic Pitch/ACE-Step remain replaceable candidates. Four stems do not fulfill the six-stem requirement; unsupported categories remain explicitly open for F3.

### Done only when

- At least the enabled qualified worker has real output and end-to-end acceptance evidence. Unqualified models are disabled with reason, not faked by canned demo output.

**Required scenarios:** T60, T61, T62

**Feature outcomes:** EDIT-05, EDIT-06, INTEL-06

**Dependency decisions:** AI-03, AI-04, AI-06

**Evidence:** None yet. Record actual commands, artifacts and limitations.

## W16 · F2 · AI/gesture regression gate

**Owner:** QA · **Depends:** W14, W15 · **Status:** not_started

**Target paths:** `tests/journeys/ai/`, `docs/verification/F2/`, `tracking/`

### Implement

1. Exercise job storms, stale revisions, locked notes, prompt injection, GPU out-of-memory, cancellation and offline behavior.

2. Run usability tasks for ghost-note acceptance, rhythms and correction with musician feedback; record results, not invented approval.

3. Measure inference and interaction latency per model/hardware without putting models on the musical clock. Record material phase gaps before declaring F2 complete.

### Done only when

- F2 workflows and resource/failure gates pass for every enabled capability. Claims distinguish rule-based assistance, measured models and deferred candidates.

**Required scenarios:** T63, T64, T65

**Feature outcomes:** Cross-cutting foundation; see mapped infrastructure and all dependent features.

**Dependency decisions:** Reuse already qualified foundations; justify new dependencies separately.

**Evidence:** None yet. Record actual commands, artifacts and limitations.

## W17 · F3 · Recording and arrangement depth

**Owner:** Engine + UI · **Depends:** W11 · **Status:** not_started

**Target paths:** `native/void-engine/src/recording/`, `packages/void-studio/src/takes/`, `packages/void-studio/src/arrangement/`

### Implement

1. Add loop takes, comping, overdub/replace, step input, repeat and bounded recent-MIDI/audio capture with explicit retention.

2. Add project/track alternatives, region loops/aliases/folders, fades/crossfades, silence removal, stacks/groups, global sections and protected edits.

3. Use engine capabilities for tempo maps/curves, beat mapping/groove, freeze/bounce and streaming; ensure edits, compensation and saved state agree.

4. Build explicit track-by-scene clip slots, quantized clip/scene launch and stop, live capture to arrangement, pattern variations/MIDI conversion and controller remix actions. Launch scheduling remains native; no UI-clock triggers.

### Done only when

- Comp an existing recording, edit arrangement/tempo and switch alternatives without damaging source takes; validate timing and recoverability.

**Required scenarios:** T66, T67, T68

**Feature outcomes:** DOC-02, ENG-05, TIME-03, TIME-04, TIME-06, REC-03, REC-04, REC-05, REC-06, ARR-03, ARR-05, ARR-06, ARR-07, PAT-02, PAT-03, PAT-04, PAT-05

**Dependency decisions:** Reuse already qualified foundations; justify new dependencies separately.

**Evidence:** None yet. Record actual commands, artifacts and limitations.

## W18 · F3 · Mixer, automation and advanced MIDI

**Owner:** Engine + UI · **Depends:** W17 · **Status:** not_started

**Target paths:** `native/void-engine/src/routing/`, `packages/void-studio/src/automation/`, `packages/void-studio/src/midi/`

### Implement

1. Expose sends/aux/VCA/groups/multi-output/external I-O, delay compensation and channel-layout validation.

2. Add read/touch/latch/write automation, trim/relative edits, move-with-region semantics and audible A-B with shared undo.

3. Add event/step editors, transforms, articulations, keyswitches, external MIDI and MPE. MIDI 2.0 support needs end-to-end timestamp/expression preservation and declared fallback, not only a type definition.

### Done only when

- Impulse/automation fixtures verify routed timing; expression and sustain survive record/edit/export where the declared format supports them.

**Required scenarios:** T69, T70, T71

**Feature outcomes:** ENG-04, ENG-06, MIDI-04, MIDI-05, MIDI-06, MIX-03, MIX-04, MIX-05, MIX-06, MIX-08, GEST-03

**Dependency decisions:** Reuse already qualified foundations; justify new dependencies separately.

**Evidence:** None yet. Record actual commands, artifacts and limitations.

## W19 · F3 · Stock sound library, effects and time/pitch tools

**Owner:** DSP + content · **Depends:** W17, W01 · **Status:** not_started

**Target paths:** `native/void-engine/src/devices/`, `packages/void-studio/src/audio-edit/`, `content/`, `docs/content-rights/`

### Implement

1. Build curated effect and instrument coverage using the named stock inventory as musical outcomes, not Apple branding or copied presets.

2. Use selected stretch DSP; implement transient markers, varispeed, repair, note-level pitch UI, correction/shifting and render-tail handling.

3. Add streaming multisamples/zones/round robins, user patches, sampler capture and downloadable cleared packs. Evaluate deeper synthesis separately for F5.

4. Validate six-stem separation separately from four-stem candidates, drum replacement/transcription, metering/loudness and automated-mastering review.

### Done only when

- Each shipped processor/instrument has listening and numerical tests, source/rights and bounded resources. Unmet specialist sound families stay tracked.

**Required scenarios:** T72, T73, T74

**Feature outcomes:** EDIT-01, EDIT-02, EDIT-03, EDIT-04, EDIT-05, EDIT-06, SND-03, SND-06, SND-07, FX-01, FX-02, FX-03, FX-04, FX-05, FX-06

**Dependency decisions:** DSP-01, DSP-02, DSP-04, DSP-05, DSP-07, DSP-08, DSP-09

**Evidence:** None yet. Record actual commands, artifacts and limitations.

## W20 · F3 · Plugin compatibility, optional isolation and exchange

**Owner:** Plugins + platform · **Depends:** W18 · **Status:** not_started

**Target paths:** `native/void-engine/src/plugins/`, `native/void-plugin-worker/`, `crates/void-interchange/`, `tests/interchange/`

### Implement

1. Qualify AUv2/AUv3 and VST3 on supported systems. Add a real CLAP host only when selected; do not confuse exporter support with hosting.

2. Implement processing isolation as its own feature using bounded native buffers, deadlines, mute/bypass fallback, bridge-latency accounting and safe restart. Process separation is not a complete OS sandbox.

3. Build declarative supported-format import/export and loss reports, including DAWproject if selected; retain audio stems/MIDI interchange.

4. Preserve missing plugin state and compatibility registry; never promise native .logicx round trips or AAX hosting.

### Done only when

- For isolation-enabled plugins, deliberately crashing one worker leaves other tested tracks usable with declared added latency. Otherwise report engine-hosted limitation, not false isolation.

**Required scenarios:** T75, T76, T77

**Feature outcomes:** HOST-01, HOST-02, HOST-03, HOST-05

**Dependency decisions:** IO-01, IO-02, IO-04, QA-01

**Evidence:** None yet. Record actual commands, artifacts and limitations.

## W21 · F3 · Accompaniment, arrangement and producer gate

**Owner:** Musical AI + QA · **Depends:** W16, W19, W20 · **Status:** not_started

**Target paths:** `workers/symbolic/`, `packages/void-studio/src/players/`, `docs/verification/F3/`

### Implement

1. Add inspectable drum/bass/keyboard/synth accompaniment, arrangement/orchestration proposals, inpainting/continuation and variations with protected source regions.

2. Add analysis-led mastering proposals with meters, audition and explicit acceptance; do not call a loudness meter a mastering assistant.

3. Complete advanced export/stem batches, custom shortcuts/screensets, consolidation/project import and producer regression coverage.

### Done only when

- F3 producer journeys pass with loss reports and measured compatibility. Every original feature is complete, explicitly blocked or scheduled—not silently removed.

**Required scenarios:** T78, T79, T80

**Feature outcomes:** DOC-04, DOC-06, INTEL-01, INTEL-03, INTEL-05, INTEL-07, OUT-02, OUT-03, PRO-05

**Dependency decisions:** Reuse already qualified foundations; justify new dependencies separately.

**Evidence:** None yet. Record actual commands, artifacts and limitations.

## W22 · F4 · Conventional native visual composition and program output

**Owner:** Visual engine + UI · **Depends:** W11 · **Status:** not_started

**Target paths:** `crates/void-visual/`, `packages/void-studio/src/visuals/`, `protocol/visual/`, `tests/visual/`

### Implement

1. Use a separate native wgpu output window first, with optional preview copies; do not require unproven zero-copy WebView embedding.

2. Implement normal images/video/presets, layers/transforms/blends, transitions, beat/sample/timecode anchors, preview/program distinction and multi-display selection.

3. Use audio-clock snapshots and calibrated presentation latency. Drop visual frames under load; audio never waits on GPU or decoder work.

4. Implement joint audio/visual checkpoint/history semantics using the transaction protocol; model independent output and preview failure states.

### Done only when

- An imported visual sequence follows a saved song through edit/reopen/output; renderer crash and slow frames do not block the native audio callback.

**Required scenarios:** T81, T82, T83

**Feature outcomes:** VIS-01, VIS-03, VIS-05

**Dependency decisions:** VIS-01

**Evidence:** None yet. Record actual commands, artifacts and limitations.

## W23 · F4 · Generated/reactive visuals and optional camera conducting

**Owner:** Visual AI + gestures · **Depends:** W22, W16 · **Status:** not_started

**Target paths:** `workers/visual/`, `crates/void-visual/`, `workers/gestures/`, `packages/void-studio/src/visuals/`

### Implement

1. Add approved visual-generation jobs as normal assets. Prefer declarative scene parameters to arbitrary generated code.

2. Add audio-feature mappings and validated shader presets; generated shaders compile off playback with resource/time limits, watchdog and known-good fallback.

3. Evaluate projectM as a distinct OpenGL path with its own rights and compositing/export tests; do not force it into wgpu without proof.

4. Optional local camera landmarks require consent, calibration, clutch, confidence threshold, smoothing and loss-triggered note release.

### Done only when

- AI/visual/camera failures leave editable musical state intact; generated material requires explicit acceptance and tracking loss never produces stuck notes.

**Required scenarios:** T84, T85, T86

**Feature outcomes:** GEST-04, VIS-02, VIS-03, VIS-04

**Dependency decisions:** AI-07, AI-09, VIS-02

**Evidence:** None yet. Record actual commands, artifacts and limitations.

## W24 · F4 · Audiovisual export and visual qualification gate

**Owner:** Media + QA · **Depends:** W23 · **Status:** not_started

**Target paths:** `workers/export/`, `crates/void-export/`, `tests/av/`, `docs/verification/F4/`

### Implement

1. Integrate an approved FFmpeg build/codec matrix with argv-only subprocess execution, restricted inputs and bounded resources.

2. Render from an immutable checkpoint with exact rational video frame rate and audio sample time; handle fractional frame rates, tails and explicit duration.

3. Persist source/model/render configuration and characterize nondeterministic output rather than promising bit-identical model or GPU rerenders.

4. Test visual-worker/GPU/export failure, absent codec, disk full, timeouts and update blocking during performance.

### Done only when

- Conventional and generated visuals export in sync with the reference audio, with correct duration and explicit errors. F4 claims match tested GPU/codec paths.

**Required scenarios:** T87, T88, T89

**Feature outcomes:** VIS-08

**Dependency decisions:** VIS-03

**Evidence:** None yet. Record actual commands, artifacts and limitations.

## W25 · F5 · Notation, scoring and advanced interchange

**Owner:** Specialist editor · **Depends:** W21 · **Status:** not_started

**Target paths:** `packages/void-studio/src/score/`, `crates/void-notation/`, `crates/void-interchange/`

### Implement

1. Integrate qualified score engraving such as Verovio; implement actual selection/edit/parts/lyrics/tab workflows over stable musical IDs.

2. Add movie scoring with absolute-time anchors, declared timecode modes and loss-aware MusicXML/AAF/Final Cut interchange.

3. Qualify Logic/GarageBand migration only for formats/specifications lawfully supported; provide stems/MIDI alternatives and explicit unsupported-feature reports.

### Done only when

- Notation edits round-trip to musical events and supported export fixtures pass; no implied complete proprietary project conversion.

**Required scenarios:** T90, T91

**Feature outcomes:** OUT-04, OUT-05, PRO-01, PRO-02

**Dependency decisions:** VIS-04

**Evidence:** None yet. Record actual commands, artifacts and limitations.

## W26 · F5 · Spatial routing and delivery

**Owner:** Spatial audio · **Depends:** W21 · **Status:** not_started

**Target paths:** `native/void-engine/src/spatial/`, `crates/void-export/`, `tests/spatial/`

### Implement

1. Implement tested multichannel/surround and binaural paths, speaker/object metadata and clear monitoring configuration.

2. Gate Atmos/ADM BWF/MP4/head-tracking work on actual specifications, licensed dependencies and reference validators. Do not use a generic stereo export as a proxy.

3. Make unsupported outputs unavailable with a reason. Continue other F5 work when licensing or hardware blocks this lane.

### Done only when

- Every enabled spatial deliverable validates externally and on declared playback hardware; blocked commercial/specification requirements remain open.

**Required scenarios:** T92, T93

**Feature outcomes:** OUT-06, OUT-07, OUT-08

**Dependency decisions:** Reuse already qualified foundations; justify new dependencies separately.

**Evidence:** None yet. Record actual commands, artifacts and limitations.

## W27 · F5 · Synchronization, modular control and safe shows

**Owner:** Performance · **Depends:** W24, W21 · **Status:** not_started

**Target paths:** `crates/void-show/`, `native/void-engine/src/sync/`, `packages/void-studio/src/perform/`

### Implement

1. Add MTC/MIDI Clock/MMC and qualified Ableton Link with explicit tempo-master rules and measured latency.

2. Add projection mapping, cue/rehearsal workflow and scoped OSC/DMX transport. Physical output requires pairing, arming, profiles and independent manual panic/blackout.

3. Implement Environment-style MIDI/modular mappings and optional companion/control-surface contracts without putting accounts/cloud on the playback path.

4. Do not permit generated instructions to directly actuate stage hardware; strobe limits and safe-disconnect behavior are explicit controls.

### Done only when

- Sync and show fixtures pass on declared devices; manual stop/blackout and disconnect override generated/queued actions.

**Required scenarios:** T94, T95, T96

**Feature outcomes:** VIS-06, VIS-07, PRO-03, PRO-04, PRO-07

**Dependency decisions:** IO-03, VIS-05

**Evidence:** None yet. Record actual commands, artifacts and limitations.

## W28 · F5 · Advanced synthesis and restricted extensions

**Owner:** DSP + extensions · **Depends:** W19, W24 · **Status:** not_started

**Target paths:** `native/void-engine/src/instruments/`, `crates/void-extensions/`, `packages/void-studio/src/patches/`

### Implement

1. Cover advanced synthesis/resynthesis/physical-modelling outcomes using selected maintained integrations, with independent content and API reviews.

2. Add restricted WASM/declarative modules with explicit capabilities, fuel/memory limits, cancellation and no ambient filesystem/network. Start outside the critical audio path.

3. Gate native plugin export and ARA on confirmed SDK rights/compatibility. Do not turn the runtime into an unrestricted code agent.

4. Bring forward the original WASM feature IDs; their implementation is deliberately deferred from the older F4 plan into this hardened F5 lane.

### Done only when

- Enabled extension/instrument workflows have load/save, limits, recovery and rights evidence; unsupported code/plugin-export features are not falsely marked shipped.

**Required scenarios:** T97, T98

**Feature outcomes:** SND-04, FX-07, HOST-03, HOST-04

**Dependency decisions:** EXT-01

**Evidence:** None yet. Record actual commands, artifacts and limitations.

## W29 · F5 · Full parity reconciliation and release readiness

**Owner:** Integrator + QA · **Depends:** W25, W26, W27, W28 · **Status:** not_started

**Target paths:** `docs/verification/F5/`, `tracking/`, `docs/release/`, `README.md`

### Implement

1. Reconcile all 116 features, 22 stock groups, 41 dependency decisions, 20 infrastructure requirements and every test against exact commits and evidence.

2. Separate complete outcomes from partial, blocked, deferred-by-approval and unsupported interoperability. No blanket Logic-equivalence claim.

3. Run clean-machine installation/update/recovery and full musician workflows across supported platforms. Audit runtime contents, model/content rights and privacy.

4. Prepare a release candidate and incident/rollback playbook; public release, store submission, licence purchase and scope removal still require explicit authorization.

### Done only when

- Release report includes supported feature/device matrix, residual risks and reproducible evidence. Any unresolved requirement prevents a full-parity completion claim, not unrelated work.

**Required scenarios:** T99, T100

**Feature outcomes:** PRO-08

**Dependency decisions:** Reuse already qualified foundations; justify new dependencies separately.

**Evidence:** None yet. Record actual commands, artifacts and limitations.
