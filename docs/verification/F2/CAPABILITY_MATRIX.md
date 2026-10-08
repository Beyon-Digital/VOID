# F2 Capability Matrix — T65 evidence

Generated from real runs of `tests/journeys/ai` on 2026-10-08 (Linux
x86_64, CPU-only box, flatc 25.9.23, cargo 1.97.1). Every row was
produced inside `unshare -Urn env -i` — **all sockets denied, loopback
included** — via `tests/journeys/ai/offline_gate.sh` (exit 0). Artifact
hashes are the real SHA-256 of the worker-emitted file imported through
the content-addressed store; timings are the runner's own
`RunOutcome.wall_ns` + the worker-reported `metrics.json`.

## Commands

```
tests/journeys/ai/offline_gate.sh                      # exit 0 (INNER GATE PASS)
cargo test --manifest-path tests/journeys/ai/Cargo.toml --test t65_capability -- --nocapture   # 7/7 ok
```

## Matrix

| Capability | Runtime | Primary artifact (real sha256) | Wall | Inference | Peak RSS | License | Remaining gap |
|---|---|---|---|---|---|---|---|
| Symbolic proposals | `void-symbolic-worker` (interval-markov-1) | `proposals.json` 1275 B `8b05e0e2…cb3c7` | 5 ms | 0 (algorithmic) | 0 (no metrics.json) | MIT (in-repo) | none — deterministic algorithm |
| Procedural audio | `void-audio-generate-procedural` (void.audio.generate-procedural) | `generated.wav` 451 244 B `afd08463…03e4` | 220 ms | 0 (algorithmic) | 39.5 MB | MIT (in-repo) | none — byte-stable deterministic |
| Stem separation | `void-audio-separate` (void.audio.separate-htdemucs-6s) | `stems/drums.wav` (+5 more) 820 304 B `8e00b1c8…378f` | 2 736 ms | 1 330 ms | 1.27 GB | MIT (demucs weights) | none |
| Transcription | `void-audio-transcribe` (void.audio.transcribe-basic-pitch) | `transcription.mid` 1 231 B `e6a1d3a1…8e91` | 11 318 ms | 10 506 ms | 630 MB | MIT (basic-pitch 0.4.0) | none |
| Text→audio | `void-audio-generate` (void.audio.generate-musicgen-small) | `generated.wav` 128 044 B `1a25f651…97d9` | 16 571 ms | 5 284 ms | 3.21 GB | **CC-BY-NC-4.0 — FLAGGED non-commercial** | license blocks commercial use; preview/audition layer absent (NEEDS §12) |

Full sha256 (gate run):
- `8b05e0e230e56774a87abd5ae711b9125720105eb25ae8d3e7ec4eb2342cb3c7` — proposals.json
- `afd08463301c870ae72101debbaf5a50ac442f8088aeedcd2a97ff04fe7603e4` — generated.wav (procedural; deterministic — same sha on every identical spec)
- `8e00b1c80d4d0aa93927b1f685c8ae593bfcfc69a72d411d09826e9169bd378f` — stems/drums.wav (first of 6 stems; htdemucs_6s)
- `e6a1d3a11e2adbcad585c9045dcd5a81559fe438ac900ebcfe256fe7928c8e91` — transcription.mid (8 note-ons on the 8-note fixture)
- `1a25f651dd1188c6389b9e8d45ce4f32e753e53968eb1f1cb9fda4ac2f9e97d9` — generated.wav (musicgen, durationS=2 honest scale-down vs W15's 4 s)

## Latency per model (CPU box)

Measured by `tests/latency.rs::f2_latency_per_model_cpu_box` (`LATS` lines):
submit→result includes spawn + run + publish.

| Model | submit→result | run wall | inference |
|---|---|---|---|
| interval-markov-1 | 14 ms | 12 ms | — |
| generate-procedural | 206 ms | 187 ms | — |
| transcribe-basic-pitch | 11 085 ms | 11 082 ms | 10 356 ms |
| separate-htdemucs-6s | 2 777 ms | 2 593 ms | 1 318 ms |
| generate-musicgen-small (2 s render) | 16 387 ms | 16 378 ms | 5 462 ms |

## Offline posture (T65 gate)

`offline_gate_inner.sh` inside `unshare -Urn env -i`:

- Loopback TCP `127.0.0.1:9`, `127.0.0.1:1` — `Errno 101` (denied)
- WAN TCP `1.1.1.1:443` — `Errno 101` (denied)
- UDP `192.0.2.1:53` — `Errno 101` (denied)
- Manual ops: `packages/void-studio` vitest `src/gestures src/patterns src/harmony src/learn src/proposals` — **79/79 pass** inside the denied namespace
- Save path dry-run: `void_project::create` + `save` + `verify_checkpoint` + `read_current` — pass
- All five capability rows above produced inside the gate

## Musical-clock separation

`tests/latency.rs::f2_jobs_never_on_musical_clock` enforces it as a gate:

1. Dependency boundary — only `void-export`, `void-proposals`,
   `void-models-tests`, `void-journeys-ai` may declare a `void-jobs`
   edge. Any new dependent fails the test.
2. Source boundary — `native/`, `apps/`, `crates/void-worker`,
   `crates/void-protocol` must not reference `JobRunner`/`void_jobs`/
   `void-jobs`. Verified clean.
3. `monotonic_ns()` is CLOCK_MONOTONIC wall time — job deadlines derive
   from spawn time only, never from a transport/beat clock.
4. `JobRunner` API surface: `run`/`request_cancel`/`cancel_token` —
   spawn/join/poll over argv processes; there is no callback entry
   point the engine could call from the audio thread.
