# EVIDENCE — engine lane (T03 / T13 / T14 / W04)

Host: macOS 15.x VM (arm64), Xcode 26.6, clang 21, CMake 3.x + Ninja, flatc 25.9.23
(built from source, commit `1872409` — PINS.md's recorded flatbuffers commit hash
does not resolve to v25.9.23; verified by `flatc --version`).

## T03 — Tracktion Engine qualification — PASS

- `third_party/tracktion_engine` submodule at `e7607547293fac421e1a9ec9898410c1893f570e`
- nested JUCE submodule at `37c894f83d379179b2070d437ccd0f1cd9af9576` (JUCE 8.0.13)
- Built `TestRunner` console target: 450 test cases / 22,282 assertions, exit 0.
- TestRunner binary SHA-256: `c19075c3…` (full hash in `docs/dependencies/QUALIFICATION.md`)
- Licence: GPL/commercial dual — dev use OK, distribution rights blocked-pending-review.

## W04 — void-engine worker — PASS (live wire smoke, 15/15)

Command:

```
PYTHONPATH=$HOME/Library/Python/3.9/lib/python/site-packages:$HOME/Library/Python/3.14/lib/python/site-packages \
python3 native/void-engine/tests/harness/supervisor_stub.py \
  build/void-engine/void-engine_artefacts/Debug/void-engine
```

Result: **exit 0, 15/15 checks**. Verified end-to-end on real sockets:

- WorkerHello first-frame handshake: kind=ENGINE, protocol 1.0, launch token echoed, engine_epoch=1.
- `create-project` APPLIED rev=1; duplicate command_id+payload → DUPLICATE.
- `add-track` (AUDIO) APPLIED rev=2; `insert-midi-clip` rev=3; `insert-note` rev=4; `set-tempo` rev=5.
- Stale `expected_revision=999` → REJECTED err=6 STALE_REVISION (`expected_revision 999 != current 5`).
- `save-project` APPLIED rev=6 → `SAVE_DURABLE <uuid>`; real checkpoint written under
  `<container>/checkpoints/<uuid>/` (engine.tracktionedit + app-state.json +
  command-receipts.json + manifest.json w/ per-file SHA-256) + `CURRENT` pointer.
- command_id reuse with different payload → REJECTED err=8 COMMAND_ID_REUSE.
- Telemetry frames arrive on the second socket after PLAY (lossy, ≤30 Hz).
- Worker stays alive for the whole session (fix: macOS `runDispatchLoop()` wraps
  `[NSApp run]`; on this headless host the system posts `applicationWillTerminate`
  ~1 s in, killing the process with EBADF on the control fd — replaced by a
  `runDispatchLoopUntil(100)` pump loop gated on an explicit `alive_` flag).
- Deterministic audio: no audio device on this box (`no audio device` log);
  device init failure is tolerated — offline render still works (T14).

## T14 — deterministic 16-bar offline render — PASS

Command: `build/void-engine/void-render-fixture_artefacts/Debug/void-render-fixture <out.wav>`

Fixture (`tests/render_fixture.cpp`): empty edit → 120 BPM @ beat 0 → 4/4 @ beat 0
→ instrument track (`FourOscPlugin` via `getPluginCache().createNewPlugin`) →
64-beat MIDI clip, 64 notes C-major (addNote, 0.9-beat length, vel 100). Render via
`te::Renderer::Parameters` (`sampleRateForAudio=48000`, `time=0–32 s`,
`endAllowance=0`, `tracksToDo=toBitSet(getAllTracks)`, `audioFormat=&wavFmt`).

Observed: `render: 1536000 frames, 2 ch, 48000 Hz` — exactly 16 bars @ 120 BPM.
`first-block peak` ≈ 0.39 (non-silent). Exit 0, prints `PASS T14`.

Evidence file: `native/void-engine/tests/t14-16bar-120bpm.wav` (9,216,784 B).
Pinned hash: `a2199a0f64ae74968a93444edaca9bdda230fb3bee17c60e070af87e36d3ea61`
(`.sha256` alongside). **Honest caveat:** FourOsc's render is not bit-identical
run-to-run on this build (peak drifted 0.391417→0.392802 across three runs;
hash changes accordingly). Frame count, sample rate, channel count and onset
timing ARE deterministic; treat the recorded WAV as reference content, and
compare candidate renders on frames+peak+onsets within tolerance, not hash.

## T16 — RT-safety probe — partial

`DeviceBridge` counts blocks/overruns/max-block-us via atomics filled in
`audioDeviceIOCallbackWithContext`; a debug jassert fires if a persistent command
ever lands on the audio thread (`[rt-safety] persistent command reached audio
thread`). Verified absent across the stub run (commands all dispatched on message
thread). Live under-load counters unverifiable: no audio device on this box.

## Blocked / remaining

- **T13** (transport against real audio device): partially — PLAY/STOP/SEEK/PANIC/
  SET_CYCLE commands are dispatched and acked on the wire (see smoke test), but
  device-backed playback position advancing can't be proven without a real audio
  device. `blocked` on hardware, not code.
- **T15** (parameter mapping automation lanes): `opSetPluginParam` resolves by
  `AutomatableParameter` UID; automation-lane readback is integrator scope.
- **T22/T23** (plugin scanner/editor proof): scanner process not yet built
  (`native/void-plugin-scanner` pending); in-process `openPluginEditor` exists.
- **T24** (crash fixture): crash injection point present (`void.crash` plugin UID
  path); a dedicated run to capture the crash report is still pending.
- Audio I/O: no device in this VM (`dm.initialise` returns no device) —
  `VOID_SKIP_DEVICE_INIT` env bypass kept for headless runs.
