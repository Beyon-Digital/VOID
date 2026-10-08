# W15 adapter qualification — what qualified, what didn't, and why

Qualification rule from WORK_PACKAGES W15: an adapter counts only with
real run evidence (command, exit code, output SHA-256, timing). Every
row below ran end-to-end through the real `crates/void-jobs` JobRunner
(`cargo test --manifest-path tests/models/Cargo.toml`, 6/6 green) and
standalone `env -i` protocol runs. Numbers: `EVIDENCE.md`.

## Qualified adapters

### Transcription — `void-audio-transcribe` ✔ QUALIFIED

- **Adapter:** `basic-pitch==0.4.0` (Spotify, MIT) on `tflite-runtime==2.14.0`, CPU.
- **Why it qualifies:** real audio→SMF conversion; on the synthesized
  8-note ground-truth fixture it detected **8/8 notes, all within 80 ms**
  of the true onsets. 10.5 s inference for a 4.7 s input; 631 MB peak RSS.
- **Trap found:** `tflite-runtime` 2.14 is built against the numpy-1 ABI
  — `numpy>=2` dies with `_ARRAY_API not found`. requirements pins `numpy<2`.
- **Limits:** polyphonic/monophonic melodic audio → note events; no
  chord labels, no tempo map beyond MIDI ticks, percussion is not its
  domain. Instrument classes map to GM program 0.

### Separation — `void-audio-separate` ✔ QUALIFIED (6 stems)

- **Adapter:** `demucs==4.1.0`, model `htdemucs_6s` (Hybrid Transformer
  Demucs 6-stem), CPU-only torch `2.14.1+cpu`.
- **Why it qualifies:** produces **all six required stems**
  (drums, bass, other, vocals, guitar, piano) — the four-stem gap in the
  W15 text (`Four stems do not fulfill the six-stem requirement`) does
  not apply to `htdemucs_6s`. Every stem preserves the input's frame
  count at the model's 44.1 kHz rate (alignment verified in the suite).
  2.1 s inference on the 4.7 s fixture; 1.22 GB peak RSS.
- **Weights:** 53 MB safetensors, vendored into the bundle at provision
  (`HF_HUB_OFFLINE=1` at runtime — job-time downloads are impossible).
- **Limits:** stem *quality* (bleed, artifacts on dense mixes) is
  workload-dependent and only spot-checked on clean synth audio —
  honest gap recorded for varied-instrument fixtures. `baguettes`/
  `hdemucs_mmi` candidates not evaluated.

### Generation — `void-audio-generate` ✔ QUALIFIED

- **Adapter:** Meta `facebook/musicgen-small` via `audiocraft==1.3.0`
  on torch `2.14.1+cpu` (audiocraft ships `--no-deps`; its declared
  torch==2.1.0 pin predates current CPU wheels — verified working).
- **Why it qualifies:** real text→music synthesis, ~5× realtime on CPU
  (4 s audio in 19.4 s inference; 2 s in 4.8 s). 3.33 GB peak RSS,
  ~1.9 GB vendored weights (EnCodec + LM + T5 encoder), fully offline
  at job time. RMS of rendered output 0.22 — real audio, not silence.
- **LICENSE CAVEAT (recorded honestly):** musicgen-small weights are
  **CC-BY-NC-4.0** — non-commercial evaluation only. The worker emits
  this warning on every run; release/distribution rights stay open.
- **Determinism:** sampling-based; `seed` is recorded but identical
  specs are NOT guaranteed byte-stable across runtimes — the manifest
  says so. The deterministic slot is filled by the procedural worker.

### Generation — `void-audio-generate-procedural` ✔ QUALIFIED (`kind: procedural`)

- **Adapter:** in-repo additive synth (harmonic partials, ADSR, vibrato,
  Schroeder reverb, soft clip) — real DSP, not a canned clip.
- **Why it qualifies:** identical spec → byte-identical WAV (asserted:
  two runs, same sha256). 126 ms wall, 44 MB RSS, code-only (no
  weights). Labeled `procedural` in its manifest — never presented as
  a learned model.
- **Role:** deterministic always-available generation path + the honest
  fallback if the CC-BY-NC license blocks MusicGen for a use case.

## Disabled / skipped candidates (with reason)

| Candidate | Status | Reason |
|---|---|---|
| `htdemucs` (4-stem) | not adopted | 4 stems < the 6-stem requirement; `htdemucs_6s` qualified instead. |
| `htdemucs_ft` | skipped | ~5.5 GB weight download > 2 GB budget rule; documented, not fetched. |
| ACE-Step | skipped | requires torch GPU-class deps + multi-GB weights; not CPU-feasible in the <10 min envelope. |
| Stable Audio / cloud APIs | rejected | paid cloud is out of scope (D06). |
| Riffusion/MusicGen-medium+ | skipped | >2 GB weights / >10 min CPU per render. |

## T60–T62 coverage

- **T60** (generation qualification): `t60_generate_musicgen_small_real_audio`,
  `t60_generate_procedural_deterministic_bytes` — real outputs, real hashes,
  timing/memory recorded; license + determinism recorded per worker.
- **T61** (stem/transcription accuracy): `t61_transcribe_basic_pitch_onset_accuracy`
  (8/8 notes ≤80 ms of truth), `t61_separate_htdemucs_6s_stems_align` (6 stems,
  frame-aligned). Varied-instrument bleed remains an open limitation.
- **T62** (cancel/retry/accept): `t62_cancel_and_retry_generation` — real
  mid-run cancel leaves no provenance/staging, retry imports fresh output;
  accept-alternate/A-B state machine lives in
  `packages/void-studio/src/generation/` (view-state, 7 vitest cases).
