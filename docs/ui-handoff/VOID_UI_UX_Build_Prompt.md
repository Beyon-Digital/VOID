# VOID / Signal Studio — UI/UX implementation kickoff

Implement the supplied VOID design in **Beyon-Digital/VOID**. Deliver a working native-connected studio UI, not a screenshot imitation or another static prototype. Start work now; do not return another planning-only document.

## Read these sources first

- Figma design: https://www.figma.com/design/8EExJVjnqfpva0gasuZAhA?node-id=4-2
- Click-through prototype: https://www.figma.com/proto/8EExJVjnqfpva0gasuZAhA?node-id=4-2&starting-point-node-id=4%3A2
- Design system: https://www.figma.com/design/8EExJVjnqfpva0gasuZAhA?node-id=2-310
- In-file behavior contract: https://www.figma.com/design/8EExJVjnqfpva0gasuZAhA?node-id=18-9
- Responsive and engineering map: https://www.figma.com/design/8EExJVjnqfpva0gasuZAhA?node-id=18-50
- `VOID_Design_System_and_Screen_Map.json`: canonical tokens, actual Figma IDs, state/permission/data contracts, 28 screens, component map and tests.
- `ACCEPTANCE_TESTS.md`: 36 implementation scenarios; not pre-passed tests.
- `references/engine-handoff/HANDOFF.md`, `CONTRACTS.md`, `WORK_PACKAGES.md`, `FEATURE_MAP.md`, `TEST_MATRIX.md`, and the actual current repository's `AGENTS.md` and implementation state.

The supplied file adds a real Figma source to original decision D08. Preserve all other architecture, licensing, security and release gates. Do not treat the 28 designed states as complete UI coverage of the original 116 functional requirement groups. Preserve and track all remaining requirements. Do not restart already-completed engine work or overwrite another agent's branch.

## Source-of-truth rules

1. Inspect the current branch, local changes, manifests, native adapter, APIs, components and tests. Report the exact commit you are implementing against. The earlier static audit commit is historical, not an instruction to reset to it.
2. Load the available Figma design-to-code guidance before retrieving design context. Read Figma variable/component definitions and screenshots for the exact nodes below. Adapt retrieved reference code into existing project conventions; do not paste a generated page with hardcoded absolute panel geometry.
3. The JSON and Figma are the v1 design source. The native contracts own music/time/state semantics. Resolve an actual mismatch explicitly and update the mapping; never silently fork two definitions.
4. Code paths in the JSON are proposed target paths, not a claim that those files already exist. Reconcile them with the current tree. Capability/data labels describe contracts to map, not already-implemented endpoints.
5. All waveform previews, candidate names, meter values, CPU percentages, job progress, saves and preflight checks in Figma are examples. Use fixtures only in stories/tests. Production UI must subscribe to real state and show clear unavailability when a capability is absent.

## Preserve the architecture

- **Tauri + Rust:** shell, worker supervision, project containers, assets, jobs, checkpoint integrity, permissions and coordinated history.
- **Tracktion Engine + JUCE/C++:** authoritative musical Edit, sample clock, transport, recording, MIDI, routing, DSP, plugin state and musical undo.
- **React/TypeScript:** bounded acknowledged view state plus transient selection, viewport and gesture/AI previews. No competing mutable song graph or durable UI save format.
- **Separate workers:** inference, analysis, visual rendering and media processing. No UI IPC, model calls, filesystem, database, unbounded allocation or locks introduced into the audio callback.
- The UI sends validated typed commands and consumes bounded snapshots/deltas. Carry commandId, projectId, target IDs and expectedRevision for edits. Retries cannot duplicate notes or clips. Never infer scene membership from an ID prefix.
- Native plugin editors stay native windows. A custom web placeholder is not a functioning plugin editor. Scanner isolation does not imply runtime plugin crash isolation. Show audio stopped when the engine stops.

## Visual direction and reusable system

Build the graphite/lime Signal Studio design with calm chrome, differentiated track colors, strong selection and precise numerals. Preserve the five workspaces: **Arrange, Compose, Mix, Perform, Visuals**. Keep one project and one transport across them. Use contextual editors rather than making an AI chat panel dominate the studio.

Use `VOID_Tokens.css` and the JSON semantic modes: Studio dark and Daylight. Colors, spacing, radius and typography must be reusable tokens. Inter is for controls, Space Grotesk for titles and IBM Plex Mono for time/numerals; obtain approved font assets through the project build and retain the system fallbacks. No font binaries are included here.

Implement/reuse these primitives in `packages/void-ui`: ActionButton, IconButton, WorkspaceTab, StatusBadge, Field, TrackHeader, TimelineClip, ParameterKnob, ChannelFader, DeviceSlot, AssetRow, SceneCell and Toast. Audio/MIDI/ghost/selected are TimelineClip variants, not duplicate unrelated component implementations. Figma fader level variants illustrate a continuous parameter; do not turn them into discrete runtime gain steps.

Compose StudioHeader, TransportBar, HealthBar, library/inspector panels, native-backed editors and recovery panels in `packages/void-studio`. Keep native command/subscription adapters in `packages/void-client`. Use the current project's existing accessible popover/dialog/selection primitives where compatible. Do not add another UI framework merely to draw a slider. Do not detach all Figma instances or flatten entire screens into images.

## Implementation sequence

### UI00 / reconcile and prove the connection

Map each surface to actual engine capabilities. Keep W00–W07 foundation gates intact. The new design is not a reason to defer native playback, plugin, checkpoint or render qualification. Make a small UI-to-native path work before expanding 28 surfaces. Add a screen/component traceability ledger with `not_started`, `in_progress`, `blocked`, `verified` and evidence links.

### UI01–UI02 / design system and first conventional path

Read **S01 4:2**, **S04 4:308**, **S22 4:2108**, **S23 4:2208**, then the project/record/export nodes from the JSON.

Build: Projects S13 → New S14 → Empty song S27 → Audio setup S15 → recording S05 / arrangement S01 / manual piano roll → mixer S04 → export S11/S12. Advanced comping S06 belongs to W17 and must not block the F1 first-song gate. Wire real save/reopen, arming, monitoring, native transport, clip edits, note edits, mixer controls and audio export. Keep the original W08–W11 first-song acceptance criteria. Preserve legacy Electron until the defined cutover gate, not indefinitely as a parallel product UI.

### UI03 / context-aware composing

Read **S02 4:108**, S25 12:1833, S26 12:2156, S03 4:208, S20 4:1908 and S08 4:708. Implement conventional note editing independently of AI. Then support request, ghost preview, audition, partial/full acceptance, dismissal, stale rejection and undo. Add S10 candidate assets and S17 background work behind model/capability availability. Preserve W12–W16 tests.

### UI04 / producer and performance depth

Wire nondestructive takes/comping S06, advanced routing/automation, plugins S18 and explicit scene launch queue S07/S28 against W17–W21. Multiple panels cannot maintain conflicting selection or parameter copies. No queued scene is called playing until the native launch is acknowledged.

### UI05 / visual composition

Wire S09 to W22–W24. Imported images/video, static textures, procedural scenes and generated assets share the same layer/timeline semantics. Preview is separate from Program; output starts disarmed, requires explicit display selection/arming and exposes blackout. Native audio timing drives visual scheduling. Late frames or generation failures never hold the audio callback. Produce real verified audiovisual export rather than an animated UI demo.

### UI06 / specialist scope

Use S24 as a notation layout starting point only. Implement actual engraving/editing/selection/export with W25 and retain W26–W29 unpictured spatial, synchronization, advanced synthesis and extension requirements. Do not mark unpictured work complete merely because the main screens exist.

## Behavior contracts that must survive the redesign

**Editing and transport:** preserve selected IDs and musical scroll/zoom when changing workspaces. Time positions derive from the native timebase, including tempo changes and loops. UI repaint can interpolate presentation from clock snapshots, but cannot schedule actual audio. Real gain units/ranges/descriptors drive controls.

**Knobs, faders and fields:** pointer capture, numeric entry, fine adjustment, arrows, unit labels, clamping, reset and one-gesture-one-undo. Focus visibility and accessible names are mandatory. Modifier shortcuts must not hijack text input or IME composition. Escape cancels an active preview/gesture before closing unrelated UI. Explicitly test pointer-up loss and device removal.

**AI proposals:** requests include musical context and revision. Original notes remain untouched before acceptance. Audition uses a native preview path and stops cleanly. Partial acceptance commits exactly selected note IDs; remaining ghosts remain separate. Dismiss-after-partial removes remaining ghosts, not committed notes. A stale proposal refuses acceptance and offers refresh; no silent rebase. Reject invalid ranges, unknown IDs, oversized output and malformed payloads. Preserve model/seed/source provenance. One accepted transaction has one undo action.

**Gestures:** pointer/trackpad and keyboard alternatives work without a camera. Keep raw contour separately from snapped interpretation. Reversible quantization and explicit target/scale/time mapping. Lost focus, pointer capture, tracking confidence or device input releases held notes. Camera permission is optional, scoped and explicit.

**Generation:** jobs are asynchronous and cancellable, resource-bounded and independent of audio. No automatic model download, cloud upload, provider switch or output insertion. Missing models show a reason and installation choice. Accepted results become immutable project assets with manifest/provenance, not transient URLs.

**Reliability:** S18 retains missing plugin state and offers locate/replace/bypass. S19 truthfully reports stopped audio, restarts stopped and validates checkpoint/take recovery. S21 never shows Saved until durable checkpoint acknowledgement. Unsaved changes survive a failed destination write. Export preflight uses real assets, routing and devices; success requires a verified output at a fixed revision. Do not double-dither or normalize without explicit selection.

**Performance and memory:** virtualize track lists and note grids; request visible slices and waveform peak levels of detail. Bound caches, metering subscriptions and job results. Do not put PCM, tensors, huge model blobs or complete waveforms in React state. Keep telemetry localized so meter updates do not rerender the whole song. Measure same-session memory, not a generic framework benchmark. Add long-session load/scroll/undo tests and worker failure cases.

**Responsive/accessibility:** reference desktop1600×1000 and laptop1280×832. At compact widths close library/inspector into drawers instead of shrinking all text. Keep the transport and selected-object scope visible. Test resizable panels, focus return, 200% text scaling and screen readers. State must not depend on color alone. Design a sensible minimum window or reflow for smaller sizes; record the decision. Reduce nonessential motion according to OS preferences.

## Verification and completion

Implement the 36 cases in `ACCEPTANCE_TESTS.md` alongside the original engine tests. Use actual repository scripts; do not invent a test pass. Run visual comparisons against the key Figma frames in dark, daylight and compact sizes. Correct cosmetic defects in shared primitives first: double padding, misaligned baselines, clipped buttons, unstyled fields and theme-unaware surfaces.

Figma checks already performed cover node structure, missing fonts and destination existence, not browser click execution, accessibility certification, audio, hardware or benchmarks. Treat those application checks as work to run. Mark missing hardware/OS/model/licence credentials as explicit blockers with reproducible commands, not success. Do not buy licences, change the product licence, distribute unapproved sample banks/models, grant Figma access or publish releases without authorization. Make all unblocked progress on the current task branch.

Deliver focused PRs in dependency order with the screen IDs, relevant Wxx package, screenshots, test commands/results, exact remaining blockers and traceability updates. Use supported native CI for each target OS. Include a short agent resume note and keep the canonical map in sync. Do not claim full Logic parity from a UI skin.

**Start with UI00 + UI01, then complete the real first-song path in UI02 before expanding AI or specialist controls.**
