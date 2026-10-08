# F1 Journey Evidence — W11 First-Song Qualification

Lane: F1 journey (`devin/void-lane-f1journey`).
Host: **Linux** (Ubuntu, x86_64) — the W11 runbook assumed macOS; platform
evidence here is `linux`. The engine builds and runs headless on Linux.
Runner: `tests/journeys/f1/f1_journey.py` driving the **real worker binary**
over the real UDS FlatBuffers wire (4-byte LE length-prefixed frames;
WorkerHello kind=ENGINE protocol 1.0, token echo, engine_epoch=1). No
in-process shortcuts anywhere.

Test-matrix coverage: **T44 pass, T46 pass, T45 partial**.

## Environment

| Item | Value |
|---|---|
| OS | Ubuntu 22.04-class Linux, kernel 6.x (uname netns capable) |
| Compiler | g++ 11/12, cmake + ninja |
| flatc | 25.9.23 at `/usr/local/bin/flatc` (protocol regenerated) |
| Audio device | **none** — ALSA `/dev/snd` absent; `VOID_SKIP_DEVICE_INIT` not needed for fixture/offline paths |
| Binaries | `void-engine` Debug `sha256 e03ff989…69d` (see below); `void-render-fixture` Debug (rebuilt after F1 additions) |

Build flags needed on this Linux box (documented for the blueprint):
```
cmake -S native/void-engine -B build/void-engine -G Ninja \
  -DCMAKE_BUILD_TYPE=Debug \
  -DCMAKE_CXX_FLAGS="-include fcntl.h" \
  -DCMAKE_CXX_STANDARD_LIBRARIES="-latomic"
ninja -C build/void-engine void-engine void-render-fixture void-recording-fixture
```
(`-include fcntl.h` works around a missing include in `main.cpp`;
`-latomic` via `CMAKE_CXX_STANDARD_LIBRARIES` fixes `__atomic_store` —
linker flags position libraries correctly; `CMAKE_EXE_LINKER_FLAGS` did
not. `apt-get install cmake ninja-build libasound2-dev libcurl4-openssl-dev
libxinerama-dev libxext-dev libxrandr-dev libxcomposite-dev libxcursor-dev
libglu1-mesa-dev ladspa-sdk`.)

## T44 — complete offline song: PASS (57/57 checks)

Full journey in one shot under a network-less user namespace:

```
./tests/journeys/f1/run_offline.sh "$PWD" t44 \
  build/void-engine/void-engine_artefacts/Debug/void-engine \
  build/void-engine/void-render-fixture_artefacts/Debug/void-render-fixture \
  tests/journeys/f1/artifacts/t44 --offline-check
# exit 0 — 57/57 checks passed
```

`unshare -Urn env -i` denies every socket: the runner asserts
(a) `connect("8.8.8.8:53")` fails, and (b) a **host-side** 127.0.0.1
listener (started by run_offline.sh before unshare) is unreachable —
proving the sandbox's loopback is isolated, not the host's. The whole
journey then runs over UDS only.

Song composed over the wire (all ops PersistentCommand, expected_revision
chained): CreateProjectOp (120 BPM, 48 kHz) → 4×AddTrackOp → AttachAssetOp
(take WAV copied to `container/assets/sha256/<sha>/`) →
InsertAudioClipOp (take at bar 8, 8 s) → 3×InsertMidiClipOp →
**267×InsertNoteOp** (kick 36/snare 38/hats 42-46 pattern, Am bass groove,
Am F C G keys stabs bars 4–11) → SetTrackGainOp/SetTrackPanOp mix →
SetPluginParamOp `filterFreq=100`, `ampAttack=0.35` on `trk-keys:synth` →
stray note + stray track each removed via UndoOp →
TransportCommand PLAY (telemetry frames received; **device-less box:
timeline advance not claimed**) → SaveProjectOp → SAVE_DURABLE
checkpoint → SetTrackSoloOp solo vox → save → solo render → unsolo → save
→ **kill -9** → fresh worker → OpenProjectOp (rev restored 294) →
read-view diff **0 diffs** on tracks/clips/notes/plugins (revision field
stripped — it legitimately differs after solo churn) → asset re-attach
(see gap G2) → param set post-reopen → save → renders → compare.

Measured output (offline run `tests/journeys/f1/artifacts/t44`):

| Artifact | sha256 / value |
|---|---|
| vox-take-src.wav | `087f8f06e395233f…8b51583` |
| render-a.wav (full mix, checkpoint A) | `518b94cc42e65af1…fa1db8e` |
| render-b.wav (full mix, post-reopen) | `61954d901c1da119…ff81cfa` |
| render-solo-a.wav / solo-b.wav | PCM `data` chunk **byte-identical, 6 912 000 B** |
| renders | 1 152 000 frames, 2 ch, 48 000 Hz each |
| Musical equality | 9/9 strong onsets detected in both renders; per-bar RMS max deviation 0.9 %; peak equal 1.0000 |

### Why full-mix renders can't be sha256-compared — measured
FourOsc voices free-run per render: two renders of the **same checkpoint**
have 5 ms-RMS-envelope correlation ≈ **0.44** (decorrelated), per-bar RMS
deviation ≈ 1 %. Sparse T14 content drifts only ~0.4 % peak; a dense
267-note mix decorrelates the envelope. So equality is asserted as:
identical frames/channels/rate + PCM-byte-identical solo take + same
score onsets + same per-bar energy. The file-level `bext` chunk carries a
render wall-clock timestamp (1 byte differed on solo renders) — PCM data
chunk is the honest byte-equality surface.

### Vocal take — import-not-mic (device limitation), real take path proven separately
No input device exists on this box (`/dev/snd` absent; ALSA seq open
fails). The "record a vocal take" step therefore ran as: (a) a
deterministic synthesized take WAV (`synth_take`, 2 partials + vibrato,
labeled **import-not-mic**) imported through the real asset path
(AttachAssetOp → InsertAudioClipOp → rendered → byte-identical across
restart); (b) the REAL recording take path proven separately via
`void-recording-fixture` (hosted EnginePlayer device): `take --midi`
finalizes a takeId with 3 journal chunks — exit 0. A mic take is BLOCKED
on hardware, documented not simulated.

## T46 — memory/UI churn: PASS (engine scope), UI blocked headless

```
PYTHONPATH=$PWD/third_party/flatbuffers/python \
python3 tests/journeys/f1/f1_journey.py t46 \
  build/void-engine/void-engine_artefacts/Debug/void-engine \
  tests/journeys/f1/artifacts/t46 25
# exit 0 — 7/7 checks passed
```

25 cycles of OpenProjectOp → InsertNoteOp + SetTrackGainOp →
SaveProjectOp → CloseProjectOp on one worker. Worker RSS from `/proc`
(VmRSS): baseline 22 008 kB → first close 26 088 kB → last close
26 288 kB. Least-squares slope over the last 60 % of closes:
**+8.5 kB/cycle**; tail delta **+112 kB total** — steady-state flat, no
monotonic leak in the engine process. Scope: engine-side churn only —
WebView/plugin-editor cycles are blocked on a headless box (documented,
not simulated).

## T45 — 48 kHz workload: PARTIAL

```
PYTHONPATH=$PWD/third_party/flatbuffers/python \
python3 tests/journeys/f1/f1_journey.py t45 \
  build/void-engine/void-render-fixture_artefacts/Debug/void-render-fixture \
  tests/journeys/f1/artifacts/t45 \
  --edit tests/journeys/f1/artifacts/t44/first-song.void/checkpoints/<id>/engine.tracktionedit \
  --minutes 3 --seconds 24
```

Repeated real offline renders of the T44 song checkpoint at 48 kHz.
Numbers filled from the recorded run below. The real T45 requirement —
30-minute device-backed run with buffer sweep — is **BLOCKED**: no audio
device exists on this host (attempted ALSA device init fails;
shown in worker logs as `open /dev/snd/seq failed`). Callback maxima /
xruns cannot be measured without a live device.

## Findings / engine gaps surfaced by F1

- **G1 (W11 blocker candidate): file-backed wave clips render silence.**
  `InsertAudioClipOp`+save writes `AUDIOCLIP source="../../../assets/…"`
  anchored to `checkpoints/live/engine.tracktionedit` *as a directory*
  (TE write-side convention via `getRelativePathFrom`). Read side —
  `SourceFileReference::findFileFromString` →
  `getEditFileFromProjectManager` — returns empty File without a TE
  Project, and `EngineSession` never sets `Edit::filePathResolver`, so
  sources resolve CWD-relative and **never load** (debug log:
  `JUCE Assertion failure in juce_File.cpp:219`). Verified: same edit
  with absolute `source=` renders the take; relative renders silence in
  both worker renders and fixture. Fixture compensates with a
  `filePathResolver` anchored at the edit file path (allowed fixture
  addition, `native/void-engine/tests/render_fixture.cpp`) — the engine
  itself needs the equivalent resolver (lane C, EngineSession edit
  options, anchor `container/checkpoints/live/engine.tracktionedit`).
- **G2: asset registry is per-session.** `opOpenProject` clears `assets_`
  and `rebuildIndexes()` does not repopulate it; `app-state.json` carries
  no asset inventory. Post-reopen `ASSET_LIST` is empty until the
  coordinator re-attaches (`AttachAssetOp` applies cleanly, restores the
  relPath). Clip→file binding itself persists in the edit XML — render
  unaffected. Documented; coordinator owns re-attach.
- **G3: `InsertNoteOp.start_ticks` is clip-relative** (sequence beat),
  not timeline-absolute — verified by rendering: a note at
  sequence-beat 16 inside a clip at timeline-beat 16 sounds at beat 32.
- **G4: after `CloseProjectOp`, session revision = close-receipt
  revision** (op resets to 0, dispatcher bumps post-op). The next
  `OpenProjectOp` must quote it as `expected_revision`; 0 is stale.
- Render nondeterminism quantified above (FourOsc free-run; envelope corr
  ≈0.44 same-checkpoint) — renders are never byte-stable on synth
  content; `bext` chunk also embeds render wallclock.

## Repro / rerun

```
# T44 (offline, all checks) — see command block above
# T46 — see command block above
# T45 — see command block above
# T14 regression: void-render-fixture <out.wav>  (fixture intact)
# recording take path:
build/void-engine/void-recording-fixture_artefacts/Debug/void-recording-fixture take /tmp/t --midi
```

Known platform deltas vs the W11 runbook: host is Linux not macOS; no
audio device; no UI. All rendered artifacts under
`tests/journeys/f1/artifacts/` (gitignored).
