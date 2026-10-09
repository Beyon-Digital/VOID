# W24 — Audiovisual export and visual qualification gate (Lane U)

Non-native half of the AV export lane: the `void-av` runner crate, a
`void-fake-ffmpeg` fixture binary, the detached `tests/av/` suite, and
the `void-studio/src/av/` view-state layer. Patterned exactly after
`crates/void-export` (stage → verify → provenance → atomic publish;
failures quarantine, partial output never marked success).

## Pieces

| Path | What |
|---|---|
| `crates/void-av/` | FFmpeg argv-only exporter on `void-jobs`. Declarative codec matrix (`codec.rs`), exact rational frame-rate math (`rate.rs`), spec+frame-plan (`spec.rs`), argv builder + subprocess runner (`ffmpeg.rs`), probe-back verifier (`probe.rs`), staged session (`session.rs`), JobDb glue (`job.rs`). |
| `workers/ffmpeg-fake/` | `void-fake-ffmpeg` — real argv parsing (`-f lavfi`, `-i`, `-r`, `-frames:v`, `-t`, `-fs`, `-c:v/-c:a`, …) emitting a `VFMT1` fixture container the fake probe decodes. `VOID_FAKE_FFMPEG_MODE` selects hang/exit1/badheader/underframes/jitter/bigdrift/envdump; `VOID_FAKE_FFMPEG_ENCODERS` restricts the `-encoders` listing. |
| `tests/av/` | Detached suite (own `Cargo.toml` + committed lockfile). T87 frame math, T88 adversarial, T89 pipeline + real-ffmpeg feature-detected smoke. |
| `packages/void-studio/src/av/` | TS twin: spec DTO + bigint rational math, codec matrix + gate, job envelope, result/provenance parsing, zustand view-state store. |

## Invariants honored

- **argv-only**: `build_argv` produces `Vec<String>`; argv[0] is the exe
  basename; `env_clear` + `-nostdin`; zero shell anywhere (T88 test
  scans argv for `;`, `&&`, `|`, backtick). Output `-t` is scoped to
  the *audio input* — output-level `-t` clips the video stream at the
  audio duration and drops NTSC tail frames (verified against real
  ffmpeg 4.4.2).
- **Rational frame math**: `30000/1001` kept un-normalized; frame f owns
  `[floor(f·sr·den/num), floor((f+1)·sr·den/num))`. Index lookup uses
  `((s+1)·num − 1)/(sr·den)` floor — plain `s·num/(sr·den)` floor
  under-counts fractional boundary starts (frame 1 opens at sample
  1601, not 1602). Rounding policy is an explicit spec field.
- **Pinned inputs**: checkpoint files resolve inside the verified
  checkpoint dir with sha256 re-check; asset blobs materialize into
  staging; `lavfi_test` sources exist only for the offline harness and
  are character-allowlisted.
- **Typed unavailability**: absent binary, absent encoder, or unknown
  codec id → `CodecUnavailable`, never a fake success. Codec rights
  (`cleared`/`development_only` + license note) live in the matrix and
  land in provenance.
- **Verify before publish**: ffprobe `-count_frames` measured frame
  count must equal the plan exactly; audio sample count via
  `duration_ts`/`duration` or a bounded decode-count fallback
  (matroska/nut carry no per-stream duration); drift must be ≤ one
  frame of samples. `provenance.json` records config hash, argv hash,
  tool versions, artifact sha, and expected-vs-measured evidence —
  bit-identical rerenders are explicitly NOT claimed
  (`NONDETERMINISM_NOTE`).
- **Bounded resources**: wall-clock timeout + `-fs` output cap +
  post-verify byte cap; stdout/stderr drained on bounded threads
  (1 MiB); kill-on-cancel.
- **Crash boundaries**: failpoint at every session step (StageDir,
  ResolveInputs, Render, Verify, Provenance, Rename, Publish) leaves no
  published output; renamed-but-unpublished dest dirs are reaped by
  `abort()`.

## Running

```sh
cargo test --manifest-path tests/av/Cargo.toml   # offline suite (fake ffmpeg)
# real ffmpeg path runs automatically iff `ffmpeg`+`ffprobe` are on PATH
```

## Studio notes

`void-studio/src/av` builds/validates the spec exactly as the runner
expects and folds job events + result cards into view state. Codec
pickers may only offer `selectableAvCodecs(...)` — real encoders
detected from `ffmpeg -encoders`. `AV_EXPORT_LIST` is a protocol gap
(see NEEDS.md); the request builder asks for it honestly.
