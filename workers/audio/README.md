# workers/audio — real local AI adapters (W15)

Four argv workers implementing `workers/PROTOCOL.md` v1 (spec JSON on
stdin → progress lines → one terminal result line). Each is a real,
qualified adapter — no canned output. Qualification + measured
numbers: `docs/ai-jobs/QUALIFICATION.md`, `docs/ai-jobs/EVIDENCE.md`.

| Bundle | Executable | Adapter | Task kind |
|---|---|---|---|
| `transcribe/` | `void-audio-transcribe` | basic-pitch 0.4.0 (tflite, bundled weights) | `transcription` |
| `separate/` | `void-audio-separate` | demucs 4.1.0 htdemucs_6s (6 stems) | `separation` |
| `generate/` | `void-audio-generate` | audiocraft MusicGen-small (text→music) | `audio_generation` |
| `generate-procedural/` | `void-audio-generate-procedural` | VOID additive-synth renderer (deterministic, `kind: procedural`) | `audio_generation` |

## Provision

```sh
workers/audio/<dir>/setup.sh     # python3 venv + pinned deps + vendored weights
```

Provisioning writes `.venv/` and `models/` inside the bundle — both
gitignored. MusicGen vendors ~1.9 GB (LM + EnCodec + T5 encoder);
separation vendors the 53 MB htdemucs_6s safetensors; transcription
weights ship inside the basic-pitch wheel (204 KB); the procedural
worker is code-only (numpy).

## Contract

Each `manifest.json` is a W12 `ModelManifest` (`formatVersion: 1`,
`workerProtocol: 1`): executable basename + sha256, artifact sha256s for
every weight file, capability flags, budget defaults measured from real
runs. Verify any time:

```sh
cargo test --manifest-path tests/models/Cargo.toml
```

The suite drives every worker through the real `crates/void-jobs`
JobRunner (never standalone calls) and fails loudly when a bundle is
unprovisioned.

## spec.parameters

Common input resolution (first match wins, see `pycommon/void_worker.py`):

- `input` — sha256 of a project asset (`assets/sha256/<sha>.<ext>`)
- `inputFile` — path relative to the job's staging dir
- `inputPath` — absolute path (coordinator-side local use only)

`transcribe`: `onsetThreshold` `frameThreshold` `minimumNoteLengthMs`
`minimumFrequencyHz` `maximumFrequencyHz` → `transcription.mid` (SMF).

`separate`: `model` (default `htdemucs_6s`), `shifts`, `overlap`,
`bitsPerSample` → `stems/{drums,bass,other,vocals,guitar,piano}.wav`.

`generate`: `prompt` (required), `durationS` (≤30), `seed`,
`temperature`, `topK`, `topP`, `cfgCoef` → `generated.wav` @32 kHz mono.

`generate-procedural`: `seed`, `progression` (chords of MIDI notes),
`secondsPerChord`, `sampleRate`, `timbre` (`pad|bright|sub`), `gainDb`
→ `generated.wav` 16-bit stereo + `notes.json`.

All workers emit `metrics.json` (wall ms, peak RSS bytes, inference ms,
output sha256s) alongside the declared artifacts.
