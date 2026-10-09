# VOID — Implementation handoff

**Version:** 1.0.0 · **Prepared:** 8 October 2026 · **Repository:** `Beyon-Digital/VOID`  
**Observed main:** `4135db035a93b57b6b9c1f3814ff6189e4aae74a` · **Execution:** not started by this handoff task.

## 1. Assignment

Build a real, editable desktop music studio with conventional recording, composition, instruments, samples, editing, mixing and delivery. Add gesture composition, predictive musical autocomplete, generated audio and conventional/generated visuals. AI is an optional creative input, not a prerequisite to making music.

Use **Tauri + Rust for the shell/application**, **React/TypeScript for bounded presentation**, and **a separate C++ Tracktion Engine/JUCE worker as the initial engine candidate**. Reuse upstream musical state, scheduling, devices, DSP and native undo. Build VOID's workflows and integration layer, not replacement low-level infrastructure without a demonstrated gap.

**First measurable outcome:** create project → import/add audio and MIDI → play → open a real native plugin editor → edit → reload UI without stopping playback → save → reopen → render. Then add recording and finish one complete offline song. A pretty screen, working IPC ping or successful stub method does not satisfy this assignment.

All **116 original feature groups**, **22 stock-tool outcome groups** and **20 infrastructure requirements** are carried into the tracking files. The **41-library catalog is a selection register**, not an instruction to install all libraries. Preserve every requirement; stage its implementation rather than silently deleting it.

## 2. Read order and authority

Read `README.md`, this file, `CONTRACTS.md`, `WORK_PACKAGES.md`, `TEST_MATRIX.md`, then the relevant rows in `tracking/`. Consult reference documents only for deeper evidence and original requirement detail.

| Order | Authority | Rule |
|---|---|---|
| 1 | Explicit user instructions and repository safety rules | Preserve approved scope, user work and credentials. |
| 2 | This handoff and `CONTRACTS.md` | Resolve architectural conflicts in the older reports using the decisions below. |
| 3 | `tracking/TASKS.json`, `FEATURE_TRACEABILITY.json`, `TESTS.json`, `INFRASTRUCTURE.json`, `DEPENDENCIES.json` | Canonical execution/status ledgers. Update these; Markdown/HTML tables are generated views. |
| 4 | `references/` | Immutable historical research snapshots. Do not treat old architecture proposals or vendor-version claims as current implementation evidence. |

The earlier Rust architecture report proposed a Rust-owned musical graph. **This packet supersedes that part: Tracktion owns the musical Edit; Rust coordinates the application.** Earlier isolation wording suggested other tracks would survive all plugin failures. **F0/F1 engine-hosted plugins can crash the engine and stop all audio.** Only tested runtime-isolated processing may claim per-plugin fault containment. The original WASM F4 scheduling moves to hardened F5; its requirement ID remains present.

This packet was checked against the same current main SHA as the earlier static audit. No combined native stack was compiled, no VOID app launched, and no audio/GPU/signing/hardware result is claimed. Refresh HEAD, open PRs and local instructions at kickoff; never reset newer work to the recorded SHA.

## 3. Decisions and approval boundaries

| ID | State | Decision |
|---|---|---|
| D01 | User direction | Tauri/Rust application; C++ is acceptable where reuse is stronger. TypeScript remains presentation-only for authoritative musical data/audio concerns. |
| D02 | User direction | Full conventional studio plus gestures, predictive composition and AI audio/visuals. Normal instruments/samples/manual workflows remain first-class. |
| D03 | User preference | Prefer maintained, documented, production-used OSS; integrate first and build only product-specific gaps. |
| D04 | Engineering default, qualification required | Tracktion/JUCE worker, one engine Edit, FlatBuffers native control protocol, SQLite app metadata/index and separate AI/visual workers. |
| D05 | Engineering default | Desktop/local-first; qualify macOS Apple Silicon, Windows x64 and one named Linux environment independently. A Linux-only agent may advance software tasks but cannot certify untested macOS/Windows hardware. |
| D06 | Not approved | Relicensing VOID, buying commercial agreements, publishing release binaries/stores, copying Apple assets, enabling paid cloud services or dropping requested scope. |
| D07 | Gated optional scope | Cloud inference, collaboration/accounts, marketplace, native-plugin generation/export and remote/mobile companions are not first-song dependencies. Preserve relevant future requirements, but do not launch external services automatically. |
| D08 | Engineering default | First-party simple UI and one thin native adapter. No Figma source is approved or included in this packet; do not fabricate Figma parity or block a functioning studio on a new design exercise. |

Record exact development-use and distribution rights for Tracktion and JUCE **separately**. Determine allowed development activity before using a dependency. Unknown distribution rights block affected distribution; they do not justify stopping unrelated protocol, UI, fixture or recovery work. Do not assume process separation removes licence obligations. Save quotes/terms/approval references without exposing private contract or signing secrets.

## 4. Baseline and code migration

The pinned source has an Electron/React entrypoint, basic domain/port types, a session-grid component and a mostly empty Web Audio adapter. These findings come from the earlier static audit at the identical SHA, not from a new runtime test. Source links are in `references/PARITY_SOURCE.json` and the baseline file.

| Existing area | Action |
|---|---|
| `apps/void-desktop/main/index.ts`, `preload/index.ts`, renderer entrypoint | Add a parallel native target, then replace Electron packaging/default startup only after F1 equivalence for implemented behavior. Preserve git history. |
| `packages/void-daw/src/adapters/WebAudioAdapter.ts` | Never count empty methods as implemented audio. Remove it from production routing after native cutover; retain only a clearly separated test/demo use if justified. |
| `packages/void-core/src/ports/` | Use as intent references. Replace loose actions and insufficient timing/state types with versioned native contracts. Avoid a second mutable song graph. |
| `packages/void-core/src/domain/SessionView.ts` and `SessionView` component | Add explicit scene/track relationships and stable slots. Do not infer scene membership from ID prefixes. |
| `packages/void-ui/components/Knob.tsx`, `Fader.tsx` | Reuse styling only where suitable. Implement actual pointer/keyboard/accessibility events and acknowledged engine parameter updates. |
| `README.md`, `migration.md`, existing scripts | Separate ambition from verified capability. Replace failing placeholder tests with real tests, not `exit 0`. Preserve requirement IDs and reference history. |

Proposed repository layout; adapt to newer code only after recording a path map:

```text
apps/void-desktop/             Tauri project + web presentation entry
packages/void-ui/              Accessible reusable UI primitives
packages/void-studio/          Compose / Arrange / Mix / Perform UI
packages/void-client/          Typed commands, viewport views, subscriptions
crates/void-app/               Rust application coordinator and permissions
crates/void-protocol/          Native transport DTOs, negotiation, validators
crates/void-worker/            Process supervisor and scoped IPC
crates/void-project/           Checkpoints, migration, recovery coordinator
crates/void-assets/            Immutable assets, indexing, bounded caches
crates/void-jobs/               AI/analysis jobs and resource reservations
crates/void-proposals/         Scoped proposal validation and acceptance
crates/void-visual/            Later native output and visual state
native/void-engine/           Tracktion/JUCE adapter, Edit, audio and MIDI
native/void-plugin-scanner/   Separate scan process from F0
native/void-plugin-worker/    Later isolated processing, not scanner reuse
workers/                      Selected locked model/media worker bundles
protocol/                     Versioned FlatBuffers schemas + DTO mapping
tests/                        Contracts, renders, recovery, UI, model, hardware
content/                      Only approved presets/samples/manifests
third_party/                  Pinned sources/submodules and notices as allowed
docs/void-handoff/            This packet and canonical execution tracking
```

Packet-relative `tracking/`, `schemas/` and `references/` paths live under `docs/void-handoff/` once installed. All other task code paths are repository-relative proposed targets, not assertions that those files already exist.

## 5. Reuse boundary and state ownership

| Component | Authoritative responsibility | Not permitted |
|---|---|---|
| Tracktion/JUCE worker | Musical tracks/clips/notes, tempo and transport, routing, plugin state, DSP, render and native musical undo | Cloud keys, unrestricted AI execution, UI-owned timing, a competing audio device manager |
| Rust coordinator | Project identity/revision, asset catalog, durable checkpoint publication, visual/app metadata, job/provenance records, capability checks and user-facing history coordination | A mirrored mutable implementation of Tracktion's song graph |
| WebView | Transient input state, visible projections, selection, pending previews and acknowledged cache | PCM/model tensors, giant whole-song clones, direct file/process permissions or a separate save format |
| AI workers | Scoped analysis, proposed edits, immutable output assets and temporary caches | Direct mutation of live Edit, hidden acceptance, unapproved network/upload |
| Visual/export workers | Visual render/output following clock snapshots; immutable render jobs | Delaying audio for frames, inference or encoders |

At W01/W04, create `docs/engine/ENGINE_API_MAP.md`: for each VOID operation, record exact upstream symbol/file, pinned revision, required thread, ownership, lifetime and verifying test. **Do not invent Tracktion/JUCE method names based on the abstract plan.** Build from the selected official examples.

Reuse the engine's native device path, scheduling, graph operations, save/load, undo and bundled processors before adding a second library. Use a narrowly tested adapter or upstream fix; a fork must have an owner, patch ledger and update plan. Keep model and render backends replaceable.

## 6. Product workflows and UI obligations

| Workspace | Minimum usable outcome | Invariants |
|---|---|---|
| Compose | Instrument/pad browser, piano roll, MIDI/controller editing, chords/patterns and later gesture/ghost-note candidates | Selected track and scope are visible. A note remains editable after AI acceptance. |
| Arrange | Real timeline/waveforms, clips, snapping, sections, takes/comping and automation as phases advance | Source media is nondestructive; edit/undo/reopen agree. |
| Mix | Audible gain/pan/mute/solo, inserts, sends/buses and real metering as supported | No fake meters; actual routing and clipping state are shown. |
| Perform / Visuals | Later explicit track-by-scene grid, queued launches, visual timeline, preview/program and safety controls | Queued musical actions follow native time; late generation never holds a cue. |

Every command-driven control needs disabled/loading/error/unsupported states, a visible selection target and a keyboard alternative. Use semantic tokens/components, readable contrast, focus restoration, localization-ready strings and reduced motion. Do not claim WCAG or screen-reader conformance solely from adding ARIA labels; test real journeys.

A predictive suggestion is **preview → audition → accept all/part or dismiss → one transaction → ordinary editable music**. A candidate must never leak into an autosave. Preserve source recordings and locked notes. Show stale proposals and errors rather than silently rebasing them. Plain text in lyrics/filenames is data, not agent instruction.

A gesture is **arm target → preview musical interpretation → record/commit → edit/undo**. Preserve raw rhythm/contour and make quantization reversible. Pointer/touch/trackpad plus keyboard deliver the initial feature; camera is optional and later. Disconnect, lost tracking and focus loss release held notes safely.

An AI-generated image/audio/video is a **normal verified asset** after explicit acceptance. Imported/static assets follow the same editing, trim, layering, save and export path. Add provenance without forcing a generative workflow.

## 7. Implementation order

| Gate | Work packages | Required result |
|---|---|---|
| F0 | W00–W07 | Qualified development dependency path; native process/protocol; actual playback/plugin editor; crash-consistent save/reopen/render; install/CI evidence. |
| F1 | W08–W11 | One complete offline song with audio/MIDI recording, editing, playable sounds, mix, undo, save/recovery/export and measured hardware behavior. |
| F2 | W12–W16 | Resource-bounded model jobs; safe musical proposals; draw/tap gestures and patterns; qualified generation/separation/transcription where enabled. |
| F3 | W17–W21 | Producer depth: takes, tempo, advanced routing/automation, instruments/effects, plugins/isolation, exchange and inspectable intelligent assistance. |
| F4 | W22–W24 | Conventional/generated native visuals, synchronized output, optional camera/reactive paths and audiovisual export. |
| F5 | W25–W29 | Notation/scoring, spatial delivery, advanced sync/shows, modular extensions/synthesis and full requirement reconciliation. |

Follow task dependencies, not simply the stage number. F3 producer work may start after F1 while F2 develops. F4 conventional visuals may start after F1; generated visual tasks depend on F2. Keep one integrating branch owner. Use at most three bounded parallel implementation lanes after shared contracts are frozen; lock schemas, root manifests, lockfiles and release workflows to the integrator.

**Development readiness is not release qualification.** A prerequisite may be marked `development_ready` only with native functional/software/failure evidence on a named target, integrator signoff, and an explicit list of remaining external OS/hardware/signing gates. This allows dependent development on that target while keeping those tests blocked and the task not fully complete. A missing real engine, failed recovery, unapproved development-use path or mocked playback is never a readiness exception. Full `complete` and release still require all applicable evidence.

Default first session: complete F0 and move through F1 as evidence permits. Do not stop at another plan. If hardware, credentials or external rights block a lane, record the precise blocker and continue independent work. Do not fabricate a pass, delete a requirement or switch engines silently to avoid a blocker. A session ending before full completion must leave a verifiable checkpoint and exact next task.

## 8. Runtime and security rules

The native callback cannot wait for UI IPC, a database, disk/network, logging, inference, GPU work or general async tasks. Preallocate and use bounded engine/native mechanisms; prepare and dispose graphs elsewhere. Audio callback timings are measured with instrumentation caveats.

Keep models/content optional and workers on demand. Use current supported pinned builds, not automatic `latest`. Store immutable media as files, not enormous database/React blobs. Aggregate process-tree memory; a small Rust parent does not prove low total RAM. Establish per-target tested budgets before advertising performance.

`CONTRACTS.md` gives conservative initial queue/cache values for implementation; they are tunable engineering defaults, not achieved benchmarks or user minimum specifications. Under load, degrade optional AI/visual work before audio. Panic/note-off must not be starved behind ordinary edit or meter queues.

Treat plugins, media archives, shaders and models as untrusted. Child processes are **not** automatically OS sandboxes. Restrict capabilities/roots/IPC peers, validate messages, cap decode/expansion, and qualify actual OS restrictions. Do not grant the WebView arbitrary process execution or filesystem/network scope. Pin/bundle approved worker dependencies; disable dynamic remote-code loading.

Conventional creation works offline without login. Credentials belong in the OS credential store; logs are redacted and bounded. No raw private audio, prompts or camera recording by default. No automatic cloud fallback or training. Models, sample banks, presets, IRs, fonts and codecs need their own licence/source records.

## 9. CI, commands and reproducible evidence

The following are **commands the agent must implement**, not commands claimed to exist now. Map native tasks to Cargo/CMake/Ninja and keep the public entrypoint consistent:

| Planned command | Contract |
|---|---|
| `pnpm doctor` | Report selected toolchains, native libraries, protocol versions, runtime SQLite and available devices without secrets. |
| `pnpm dev:native` | Build/start Tauri and the selected native worker with no Electron audio path. |
| `pnpm build:native` | Locked release-like build of app/worker/runtime bundle. |
| `pnpm test:unit` / `pnpm test:contract` | Real native/Rust/frontend unit tests and schema/protocol/adversarial tests. |
| `pnpm test:render` / `pnpm test:recovery` | Deterministic render comparison and checkpoint/recording failpoint tests. |
| `pnpm test:e2e` | Installed/native app workflows with actual audio/state assertions; UI-only mocks stay separately named. |
| `pnpm verify:f0` / `pnpm verify:f1` | Aggregate gate report that distinguishes software pass, hardware pass, blocked and unverified. |
| `pnpm verify:packet` | Run the packet validator and traceability checks. |

Set CI job `timeout-minutes` explicitly; start at 15 for lint/schema, 30 for unit/contract, 45 for native build, and 60 for a controlled soak job, then tune with recorded evidence. These are ceilings, not time estimates. Cancel superseded branches and bound matrix concurrency. No untrusted fork code on privileged hardware runners or with signing secrets.

Use native OS build jobs and clean installers. Keep test-only automation hooks out of release binaries. Recheck the selected Tauri automation route against its actual pinned version; WebView automation does not validate native plugin editors or audio hardware. A signed updater is not a substitute for OS signing/notarization. Installation/restart must be blocked during recording/performance. Missing signing keys mean internal unsigned testing only.

Every test result records commit/build SHA, exact steps/command, exit status, fixture and artifact hashes, OS/toolchain/device/driver/plugin/model versions, observed output, result and limitations. Preserve numerical/audio/render evidence; screenshots are supplementary. Hardware not available = blocked or not run, never passed using a mocked device.

## 10. Definition of done and agent report

A work package is done only when its user outcome, referenced integration tests and **each mapped feature-specific assertion** pass, with committed implementation and evidence. A passing upstream SDK demo is a foundation proof, not an entire VOID workflow. A broad task passing does not close every feature assigned to it automatically.

Maintain `tracking/TASKS.json`, `TESTS.json`, `FEATURE_TRACEABILITY.json`, `INFRASTRUCTURE.json`, `DEPENDENCIES.json` and `PROGRESS.md`. Generate readable views with the included tools. Use `examples/evidence.json` only as a schema example; it contains no executed result. The packet validator checks integrity, not application correctness.

At each checkpoint/PR, report:

1. Starting/current SHA, branch and task IDs completed.
2. User-visible functions that actually work, with evidence paths.
3. Commands/tests run, pass/fail counts, blocked hardware and exact failures.
4. Dependency pins, rights/distribution status and any contract change.
5. Remaining IDs and the next ready task; no blanket “production ready” or “Logic parity” claim without evidence.

Open reviewable PRs when repository access permits. Do not force-push, bypass checks, merge/publish or spend money without explicit authorization. Keep the whole program moving with small validated increments; do not replace implementation with repeated planning documents.

## 11. Evidence references

Original repository and dependency links are preserved in `references/PARITY_SOURCE.json` and `tracking/DEPENDENCIES.json`. The current baseline is in `tracking/BASELINE.json`. Limited upstream rechecks on 8 October 2026 confirm the separate Tracktion/JUCE licensing boundary, engine-provided data model, Tauri communication/testing constraints and SQLite WAL advisory; they do not requalify all 41 libraries. See `SOURCES.md`.
