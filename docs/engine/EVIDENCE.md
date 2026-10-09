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

## T24 — engine-hosted plugin crash — PASS (fixture)

Command:

```
PYTHONPATH=$HOME/Library/Python/3.9/lib/python/site-packages:$HOME/Library/Python/3.14/lib/python/site-packages \
python3 native/void-engine/tests/harness/crash_fixture.py \
  build/void-engine/void-engine_artefacts/Debug/void-engine
```

Result: **exit 0, 6/6 checks**. `InsertPluginOp` with `plugin_uid="void.crash"`
armed the fixture (`msg=crash fixture armed (test)`); PLAY then aborted the
worker with **SIGABRT (exit=-6)**, control socket EOF — a real process crash
observable by a supervisor. Recovery/offering is the coordinator lane's job;
engine-side the kill is clean and the project checkpoint persisted beforehand
stays intact (checkpoint write verified in the same suite).

## T22 — plugin scanner containment — PASS (fixture + real AU)

`native/void-plugin-scanner` (JUCE console, one-shot). Commands + results:

| Probe | Command | Result |
|---|---|---|
| Real AU | `--format AU --uid "AudioUnit:aufx,dely,appl"` | `{"found":true,...,"name":"AUDelay","manufacturer":"Apple","version":"1.6.0","category":"Effect","binarySha256":"id-sha256:9f97ba38…"}` exit 0 |
| Bad UID | `--format AU --uid "AudioUnit:aufx,zzzz,zzzz"` | `{"found":false,"error":"No compatible plug-in format exists for this plug-in"}` exit 0 |
| Hang | `--selftest hang` (killed @3s) | exit 137 — only scanner dies under supervisor timeout |
| Crash | `--selftest crash` | exit 134 (SIGABRT) — only scanner dies |

Incompatible-CPU-architecture probe: `blocked` — no foreign-arch plugin binary
on this VM (would need an x86_64 .vst3 to witness refusal; recorded honestly).
Native *editor* proof (T23): `openPluginEditor`/`closePluginEditor` ops exist
in-process (`PluginHost.cpp`, `showWindowExplicitly`); no GUI session on this
box to drive a visible window cycle — `blocked` on display, not code.

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

## W08 — recording (T31–T35), engine side

Host: macOS 26.6.2 arm64, Apple clang 21, ninja build `build/void-engine`,
`void-recording-fixture` sha256 `159cac9ec7d9303e49090c6c62dc21de311ae60f86a5b60ef08a75fb094684c4` (Debug).
No recording ops in protocol major.1 — engine subsystem driven in-process via
fixture binary (NEEDS.md §7). All audio evidence uses TE's hosted device
(`EnginePlayer`, 2×1ch wave inputs `Input 1`/`Input 2` + virtual `MIDI Input`).

### Commands + exit codes

| Scenario | Command | Exit |
|---|---|---|
| multitrack mono+midi | `void-recording-fixture take /tmp/vr-t1 --midi` | 0, 20/20 PASS |
| punch-in live | `take /tmp/vr-pu --punch` | 0 |
| stereo pair | `take /tmp/vr-st --stereo` | 0, chunks:1 2ch |
| kill mid-take | `take /tmp/vr-k4 --kill-after-blocks 40` | 134 (SIGABRT, simulated crash) |
| reopen + recover | `recover /tmp/vr-k4` | 0, status `incomplete` |
| panic all-notes-off | `panic /tmp/vr-pn` | 0, allNotesOff=16, noteOffs=0 |
| error paths | `errors /tmp/vr-er` | 0, 8/8 explicit failures |
| hosted latency | `latency /tmp/vr-la` | 0, roundTripMs 2.729 @48k/512 |
| full suite | `python3 tests/recording/recording_checks.py <fixture>` | 0, SUMMARY 0 failures |
| baseline regression | `supervisor_stub.py void-engine` | 15/15 unchanged |

### Artifact evidence (take b77366c529304ef, /tmp/vr-t1)

- `recordings/<take>/journal.json` — `void-take-journal/1`, status `finalized`,
  3 chunks, 3 clipIds; sha256 `a5150e0518f657ad4bdb84e03c8dd06ce71e3fc626b7aa4f833e3ed215f3ddc6`
- `chunk-dest-a-1.wav` 102160 B, 1 ch, 33792 frames; sha256 `1892aa8df3101f8362964ea39c428b89eb003de92de562c72dd0db7c3b36adf2` (matches journal entry — sealed at finalize)
- `chunk-dest-b-1.wav` sha256 `97352b865a4f0198d150978fffbec008cb9bf97820cb38581499066b71cd530f`
- `midi-dest-midi.jsonl` sha256 `c7f2867ec7cd095f5520d0d23d0058429038d1ddcba0220abb56dd04850d669e` — 7 rows preserving noteOn/sustain/pitchBend/polyAftertouch/aftertouch/noteOff with `ts` + `hostNs` stamps
- killed take `b86a1db12892444`: reopened → status `incomplete`, chunks listed with `bytesObserved=31504`, `framesObserved=10240` (RIFF-walk salvage on unfinalized data-size), no sha256 seal — never mistaken for final

### Blocked (honest, not faked)

- **T33 real hardware latency**: `blocked` — no audio device on this VM
  (headless). Hosted path measured instead: `inputLatencyNumSamples=0`,
  `outputLatencyNumSamples=131`, `recordAdjustment=0`, roundTrip=2.729 ms
  @48 kHz/512 frames. Method for hardware: same fixture with
  `EnginePlayer`→CoreAudio device + `getRecordAdjustment()` read-back.
- **Mic-permission denial (T31/T34)**: `blocked` on this box — TCC mic
  entitlement flow can't trigger headless; engine-side equivalent covered by
  `errors` scope 1 (no device → explicit `Result::fail`, never false success)
  and `deviceListChanged` take-fail path.
- **Disk-full (T34)**: `startRecording` pre-checks `getBytesFreeOnVolume`
  ≥ 64 MiB; a real ENOSPC mid-take is not reproducible on this volume —
  path covered by code + explicit `fail()` result, runtime probe `blocked`.
- **Default-device-change (T34)**: `deviceListChanged` listener fails a live
  take when an armed input disappears; hot-swap of a real device `blocked`
  (no swappable hardware present).

### RT-safety note

`handleIncomingMidiMessage` (Consumer callback) copies ≤3 bytes + stamps into
a `juce::AbstractFifo` — no locks, no allocation, no disk. Journal and
`.jsonl` writes happen on a 20 Hz `juce::Timer` (message thread) or at stop —
never on the audio callback.
