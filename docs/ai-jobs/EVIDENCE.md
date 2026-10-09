# W15 evidence — commands, exit codes, hashes, timing

Everything below ran on the W15 lane box (Linux x86_64, python 3.10.12,
CPU only). Two evidence levels: standalone protocol runs under
`env -i` (proves the argv contract with a cleared environment, exactly
what `WorkerRuntime` does with `env_clear`), and the JobRunner suite.

Fixture: `synth_melody_wav()` synthesized by the test suite — 8-note
melody (MIDI 60 64 67 69 67 64 62 60, 0.55 s spacing, legato 0.5 s),
48 kHz stereo PCM16, sha256 of the python-synth equivalent
`b48eedaa1464d0221b02111e6f8f5492757a2f5dc95250c670ef2d29c97635e0`.

## JobRunner suite (authoritative)

```
$ cargo test --manifest-path tests/models/Cargo.toml -- --test-threads=1 --nocapture
... exit code 0 — 6 passed, 0 failed (95.45 s wall)
```

| Test | Result | Measured |
|---|---|---|
| `w15_manifests_verify_and_resolve` | ok | 4 manifests verify; launcher + artifact sha256s match vendored files |
| `t61_transcribe_basic_pitch_onset_accuracy` | ok | 8/8 notes detected, 8/8 onsets ≤80 ms of ground truth, wall 10 643 ms |
| `t61_separate_htdemucs_6s_stems_align` | ok | 6 stems (drums,bass,other,vocals,guitar,piano), all frame-aligned, wall 2 533 ms |
| `t60_generate_procedural_deterministic_bytes` | ok | identical spec → identical sha256 `6e8bcdce…` across 2 runs, walls 143+125 ms |
| `t60_generate_musicgen_small_real_audio` | ok | 4.00 s @32 kHz, RMS 0.2208 (real audio), wall 21 678 ms, sha `288e2e6f…` |
| `t62_cancel_and_retry_generation` | ok | cancel mid-render → `Cancelled`, zero artifacts/staging; retry succeeded in 21 977 ms |

## Standalone protocol runs (env -i, spec via stdin)

### void-audio-transcribe

```
$ env -i workers/audio/transcribe/void-audio-transcribe --staging <st> < spec.json
{"v":1,"kind":"result","status":"succeeded","artifacts":[{"path":"transcription.mid"},{"path":"metrics.json"}]}
exit code 0 — wall 11 034 ms, inference 10 546 ms, peak RSS 631 345 152 B, noteCount 8
input sha256  b48eedaa1464d0221b02111e6f8f5492757a2f5dc95250c670ef2d29c97635e0
weights sha   3db297d54af8e01c6e5618245c956b1d71b6a2b978cb2dedb527173186552676 (nmp.tflite, 204 448 B — in-wheel)
```

### void-audio-separate

```
$ env -i workers/audio/separate/void-audio-separate --staging <st> < spec.json
{"v":1,"kind":"result","status":"succeeded","artifacts":[{"path":"stems/drums.wav"},…6 stems…,{"path":"metrics.json"}]}
exit code 0 — wall 3 206 ms, inference 2 148 ms, peak RSS 1 221 320 704 B, stemCount 6 @44.1 kHz
weights sha   d2a1745f0744721f6b8ca5bf469b67c651ea5ed1b52998cab033b2158609d411 (5c90dfd2.safetensors, 54 885 744 B)
              207405151270af8fd81c2373c25d27950916682ac91dca7884a11ce13dad6f58 (htdemucs_6s.yaml, 21 B)
HF rev        adefossez/HTDemucs-6s @ 3c5ee475be622df764938de97e4281a7b07ffa58 (vendored, HF_HUB_OFFLINE=1)
```

### void-audio-generate (musicgen-small)

```
$ env -i workers/audio/generate/void-audio-generate --staging <st> < spec.json   # prompt "80s synth arpeggio…", durationS 4
{"v":1,"kind":"result","status":"succeeded","artifacts":[{"path":"generated.wav"},{"path":"generation.json"},{"path":"metrics.json"}]}
exit code 0 — wall 30 563 ms, inference 19 413 ms, peak RSS 3 332 849 664 B
out wav sha   b365f8751b672bcd875d70285f23b07e7b016dbaef0c1326021a573de4dc8c57 (4.00 s @32 kHz)
weights sha   830fe5771e2835cbd305f3ffdda22d3ff38718fbbe8274bd19dc203265de7244 (state_dict.bin LM, 840 843 863 B)
              37d256b525d4117f8bbf790ab448a8e9f4746cd401900fe1ae72e154b1513a30 (compression EnCodec, 236 001 935 B)
              a90903540cc02cbeb7ff9f823f1a80eb778c7e22426a0e620b01c77a5ec8f5b4 (t5-base model.safetensors, 891 646 390 B)
              d60acb128cf7b7f2536e8f38a5b18a05535c9e14c7a355904270e15b0945ea86 (spiece.model, 791 656 B)
              d2acde0d8d71dd30a711834b07781b9c89feaac33fd332f60507699282740066 (tokenizer.json, 1 389 353 B)
              46dd7cb62d29c81fb551e0ef1ea274c24a46ba441eeb948897706252933df033 (config.json, 1 208 B)
HF revs       facebook/musicgen-small @ 4c8334b02c6ec4e8664a91979669a501ec497792
              t5-base @ a9723ea7f1b39c1eae772870f3b547bf6ef7e6c1 (vendored, HF_HUB_OFFLINE=1)
budget probe  ulimit -v 8388608 (8 GiB VA) → still succeeded (2 s render, 4 802 ms inference)
```

### void-audio-generate-procedural

```
$ env -i workers/audio/generate-procedural/void-audio-generate-procedural --staging <st> < spec.json   # seed 7
{"v":1,"kind":"result","status":"succeeded","artifacts":[{"path":"generated.wav"},{"path":"notes.json"},{"path":"metrics.json"}]}
exit code 0 — wall 126 ms, peak RSS 43 618 304 B, 16 notes, 4.35 s @44.1 kHz
out wav sha   cc3e3b1e7cd4e0239e95ed41a9f3623532e8b749719b8c82d0f55117a965ccd5
determinism   identical spec → identical sha (suite: 6e8bcdce… at sampleRate 48000, 2× run)
```

## Provisioning commands (for reproducibility)

```
workers/audio/transcribe/setup.sh            → basic-pitch 0.4.0, tflite-runtime 2.14.0, numpy 1.26.4 — 511 MB venv
workers/audio/separate/setup.sh              → torch 2.14.1+cpu, demucs 4.1.0, HF snapshot — 1.1 GB venv + 53 MB weights
workers/audio/generate/setup.sh              → torch/torchaudio +cpu, audiocraft 1.3.0 --no-deps, HF snapshots — 2.4 GB venv + 1.9 GB weights
workers/audio/generate-procedural/setup.sh   → numpy 2.2.6 only — ~50 MB venv
```

## Known gaps (honest)

- MusicGen weights CC-BY-NC-4.0 → non-commercial evaluation only; a
  licensable generation model remains open for F3.
- Stem quality measured on clean synth audio only; dense-mix bleed and
  varied-instrument fixtures not yet scored (T61 limitation).
- MusicGen output is sampling-based — no byte-determinism claim; seed
  recorded for provenance only.
- 30 s max render length per job (`durationS ≤ 30`); longer generation
  needs segmented requests (not implemented).
