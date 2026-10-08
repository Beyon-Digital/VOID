# VOID engine qualification — T03 evidence

Recorded by the engine lane on the macOS qualification box, 2026-10-08,
branch `devin/void-lane-engine`.

## Pinned pair (source of truth: `docs/dependencies/PINS.md`)

| Component | Pin | Resolved SHA |
|---|---|---|
| Tracktion Engine | `develop` @ `e7607547293fac421e1a9ec9898410c1893f570e` | `e7607547293fac421e1a9ec9898410c1893f570e` (VERSION.md → 3.5.0) |
| JUCE | engine-supplied submodule `modules/juce` | `37c894f83d379179b2070d437ccd0f1cd9af9576` |
| FlatBuffers flatc | `v25.9.23` | tag object `edbe17738352418245d7228e7fd9f12c3ddc34c4` → commit `187240970746d00bbd26b0f5873ed54d2477f9f3` |

> Correction note: `PINS.md` lists flatbuffers commit `03fffb25…` — the actual
> commit the annotated tag `v25.9.23` resolves to is `18724097…` (verified via
> `git ls-remote --tags` and `git rev-parse HEAD` after checkout). PINS.md is
> integrator-owned; flagged here for the integrator to fix.

Submodules registered in `.gitmodules`:

- `third_party/tracktion_engine` → `https://github.com/Tracktion/tracktion_engine` @ `e7607547`
- `third_party/tracktion_engine/modules/juce` → juce-framework/JUCE @ `37c894f8` (engine-recorded compatible revision)
- `third_party/flatbuffers` → `https://github.com/google/flatbuffers` @ tag `v25.9.23` (`18724097`)

## Host / toolchain

| Item | Value |
|---|---|
| OS | macOS (arm64, Apple silicon, 12 cores, 16 GiB) |
| Xcode | 26.6 (build 17F113) |
| macOS SDK | 26.5 (`xcrun --show-sdk-version`) |
| Compiler | Apple clang 21.0.0 (clang-2100.1.1.101) |
| CMake | 4.2.0 (Homebrew) |
| Ninja | installed via Homebrew |
| flatc | 25.9.23 — built from source at pinned tag (`cmake -G Ninja -DFLATBUFFERS_BUILD_TESTS=OFF && ninja flatc`) |

## Upstream example build evidence (T03)

Commands run:

```sh
cd third_party/tracktion_engine
git checkout e7607547293fac421e1a9ec9898410c1893f570e
git submodule update --init --depth 1 modules/juce   # → 37c894f83d379179b2070d437ccd0f1cd9af9576
cmake -G Ninja -B /tmp/te-build -DCMAKE_BUILD_TYPE=Release -DTE_ADD_EXAMPLES=ON .
ninja -C /tmp/te-build TestRunner
```

Results:

| Step | Exit code | Notes |
|---|---|---|
| `git checkout` TE pin | 0 | detached HEAD at `e7607547` |
| JUCE submodule init | 0 | `37c894f8` |
| CMake configure | 0 | "building for macOS", VST2 off, examples on |
| `ninja TestRunner` | 0 | 39 unity-build targets, clean link |

Binary: `/tmp/te-build/examples/TestRunner/TestRunner_artefacts/Release/TestRunner`
SHA-256: `c19075c3e1b5640b481b58687f7ced30a84e156726561c7416b3adfef1da20d4`

## Upstream functional evidence

`TestRunner` runs the engine's own doctest suite (tracktion_core,
tracktion_engine, tracktion_graph categories — Edit model, archive round
trips, tempo/time conversion, audio files, MIDI lists, graph/node
processing, allocation pools, semaphores).

Command: `/tmp/te-build/examples/TestRunner/TestRunner_artefacts/Release/TestRunner`
Exit code: **0**

```
[doctest] test cases:   450 |   450 passed | 0 failed | 0 skipped
[doctest] assertions: 22282 | 22282 passed | 0 failed
[doctest] Status: SUCCESS!
```

This is functional evidence on the actual target OS — the pinned
engine/JUCE pair compiles and its own unit tests pass on arm64 macOS with
Xcode 26.6 / SDK 26.5.

## Runtime licences / feature flags inspected

| Component | Licence (upstream text) | Feature flags used |
|---|---|---|
| Tracktion Engine 3.5.0 | GPL-3.0 / commercial dual (LICENSE.md) | `JUCE_PLUGINHOST_AU=1`, `JUCE_PLUGINHOST_VST3=1`, `TRACKTION_ENABLE_TIMESTRETCH_SOUNDTOUCH=1`, `TRACKTION_ENABLE_TIMESTRETCH_SIGNALSMITH=1`, `TRACKTION_ENABLE_TIMESTRETCH_RUBBERBAND=0` (rubberband submodule absent — permissive dependency, recorded not-present), `TRACKTION_UNIT_TESTS=1` for TestRunner only |
| JUCE (`modules/juce` @ 37c894f8) | AGPL-3.0 / commercial dual (LICENCE.md) | `JUCE_USE_CURL=0`, `JUCE_WEB_BROWSER=0`, `JUCE_MODAL_LOOPS_PERMITTED=1`, `JUCE_PLUGINHOST_AU/VST3/LADSPA`, `JUCE_STRICT_REFCOUNTEDPOINTER=1` |
| VST2 SDK | not present | `JUCE_PLUGINHOST_VST` unset — VST2 not built (SDK not distributed) |
| FlatBuffers | Apache-2.0 | header + `flatc` only, no runtime deps |

## Rights state

- **Development use: permitted** for internal builds (this box, CI) under the
  upstream licences.
- **Distribution rights: BLOCKED pending review** — TE GPL-3.0 and JUCE
  AGPL-3.0 would apply to any distributed binary unless commercial licences
  are purchased (not approved, handoff D06). No relicensing, publishing, or
  store submission performed or implied.
- Rubberband (optional time-stretch dependency) was not fetched; SoundTouch
  and SignalsmithStretch are compiled in — enough for F0/F1 scope.
- macOS engine builds certify only macOS. Windows/Linux qualification is a
  separate evidence row (per `TEST_MATRIX.md`, unverified until run there).

## Limitations

- `TestRunner` exercises the engine in-process; it is dependency
  qualification evidence, not VOID feature QA.
- The legacy VST2 path is untested by policy (no SDK).
- Engine "examples" DemoRunner GUI was not part of this build — TestRunner
  was chosen because it runs the upstream test suite headlessly.
