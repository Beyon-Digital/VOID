# VOID — Main kickoff prompt

Work in `Beyon-Digital/VOID`. Use the attached `VOID_Agent_Handoff.zip` as the implementation brief. Extract/install its contents into `docs/void-handoff/` without overwriting newer implementation or user work. The packet contains its own sources; no earlier chat is required.

Implement the product, not another plan: a complete editable music studio with conventional recording/composition/editing/instruments/mixing/export, then gestures, predictive autocomplete and AI audio/visuals. Use Tauri/Rust for the application, React/TypeScript only for bounded presentation, and the qualified Tracktion Engine/JUCE C++ worker for musical state. Integrate existing capabilities before custom infrastructure. No fake audio, meters, stubs or canned AI output counted as implementation.

Read `README.md`, `HANDOFF.md`, `CONTRACTS.md`, `WORK_PACKAGES.md`, `TEST_MATRIX.md` and the relevant `tracking/*.json`. The current handoff supersedes older reports where ownership or plugin-isolation claims conflict: Tracktion owns the song/undo; Rust coordinates app metadata/history/checkpoints. Keep all 116 feature IDs, 22 stock groups and the 20 infrastructure rows; the 41-library catalog is conditional, not an install-all list.

First read repository instructions, fetch current main and inspect local changes/open PRs. The handoff observed main at `4135db035a93b57b6b9c1f3814ff6189e4aae74a`; use the latest actual branch, never reset to that historical hash. Record the baseline and reuse any completed newer work. Create a reviewable implementation branch and make staged commits.

Begin with W00–W07 (F0) and continue W08–W11 (F1). Prove this actual path: create project → add audio/MIDI/instrument → native playback → real native plugin editor → edit → reload WebView without interrupting audio → save → close/reopen → correct offline render; then recording and a complete offline song. Do not stop at a shell scaffold. After F1, continue dependency-ready F2/F3/F4 tasks; retain F5 specialist scope and qualification gates. Use the phase prompts for continuation, not as reasons to wait for another planning round.

Pin and test the engine/JUCE/toolchain pair and record permitted development use before adoption. Preserve current repository licensing; do not purchase licences, relicense, publish/store-submit, enable paid cloud services or copy Apple samples/presets. Unknown rights block affected distribution, not unrelated lawful work. If the default engine fails a concrete qualification gate, document evidence and continue independent tasks; do not silently replace it with a homegrown DAW.

The native callback must not wait on UI, database, disk/network, AI or graphics. No PCM/model tensors or mirrored whole-song mutable state in JavaScript. Use typed validated native commands, stable IDs, explicit revisions/epochs, bounded queues, per-plugin scan quarantine, immutable media and crash-tested checkpoints. A scan subprocess is not runtime plugin isolation. Engine-hosted plugin crashes stop audio; only separately implemented/tested processing isolation may claim other tracks continue.

All AI edits are scoped proposals with preview/audition/accept/reject/partial accept, stale-result validation and one-step undo. Manual creation must work with all models/network disabled. Gestures need keyboard alternatives, explicit arming and note-off/panic safety. Static and generated visuals are ordinary revisioned assets; neither inference nor rendering may become the audio clock.

Use bounded parallel work only after contracts freeze: one integrator owns protocol/root manifests/lockfiles and at most three independent lanes own disjoint paths. No force-push, check bypass, automatic merge or public release. Preserve existing tests and fix real failures rather than hiding them.

For every package, execute its tests and individual feature assertions. Record actual commands, exit codes, build/fixture/artifact hashes and environment/device/model versions. No macOS/Windows/audio-hardware/signing pass may be inferred from Linux or mocks. Missing resources are `blocked`/`not_run`; continue independent work. Use the documented `development_ready` target-specific prerequisite state only when real native software/failure checks pass and only external qualification is outstanding; never call that full completion or release approval. Update canonical tracking JSON and `PROGRESS.md`, regenerate readable views, and run the packet validator. Packet integrity validation is not app QA.

At each checkpoint/PR report: start/current SHA; completed task IDs; working user journeys with evidence; tests run/failed/blocked; pins/licence state; remaining requirement IDs and exact next ready task. Do not claim full Logic parity or production readiness without the complete evidence. If the session ends, leave committed runnable progress and a precise resume checkpoint rather than restarting the design.

Start W00 now, then implement the native foundation.
