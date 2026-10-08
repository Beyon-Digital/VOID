# VOID — F0 / Native foundation

Continue implementation in `Beyon-Digital/VOID` using `docs/void-handoff/`.

Read repository instructions, `HANDOFF.md`, `CONTRACTS.md`, `PROGRESS.md`, and the task/test/feature/dependency rows for **W00–W07**. Fetch the current branch and inspect existing PRs/work; reuse completed changes rather than rebuilding them.

**Prerequisite:** No earlier implementation assumed. Read the latest repository and reconcile the packet baseline.

**Implement:** Tauri + Rust supervisor; real C++ Tracktion/JUCE session; typed bounded control protocol; stable IDs/revisions; crash-consistent checkpoint; native plugin processing/editor; installed build and CI.

**Required exit:** Create/import → native play → native plugin editor → parameter edit → UI reload → save/reopen → deterministic render. Do not add model runtimes or a visual editor before this path exists.

Use the precise package steps and acceptance scenarios in `WORK_PACKAGES.md` and `TEST_MATRIX.md`; every mapped feature-specific assertion still needs evidence. Keep Tauri/Rust app ownership and Tracktion musical ownership. Integrate existing libraries, pin the selected compatible versions and respect rights gates. Typed native commands only; all UI/model input is validated. No stub success, fake audio/model result, unbounded memory, silent scope removal or new architectural replan.

Run actual checks, record build/fixture/artifact hashes and device/model/OS details, and mark unavailable external resources blocked—not passed. Continue independent ready tasks. Update canonical tracking JSON and `PROGRESS.md`, regenerate views and leave reviewable commits/PRs without bypassing checks, force-push, merge, spending or publishing.

Report working journeys, completed IDs, failed/blocked checks, evidence paths, licence/dependency changes and the exact next ready task. Implementation is the deliverable, not another plan.
