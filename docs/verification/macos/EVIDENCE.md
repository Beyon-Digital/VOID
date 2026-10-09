# Lane BB — macOS verification evidence

Host: macOS 26.6.2, arm64 (Apple Silicon), Darwin 25.6.0.
Toolchain: rustc/cargo 1.97.1 (rustup, aarch64-apple-darwin), clang (Xcode
26 RC toolchain via CLT), cmake 4.4.4 + ninja 1.13.2 (brew), node v24.21.0,
pnpm 12.8.1, flatc 25.9.23 (built from `third_party/flatbuffers`, pinned by
`tools/protocol-gen.sh` / `void-protocol/build.rs`).

Commands run from repo root unless noted. `cargo`/`rustc` were symlinked into
`/opt/homebrew/bin` because the box ships rustup toolchains without proxies;
`flatc` was built once with
`clang++ -std=c++17 -O2 -Iinclude -I. -Igrpc src/*.cpp -x c++ grpc/src/compiler/*.cc include/codegen/*.cc -o flatc`
(minus `src/flathash.cpp`) and symlinked to `/opt/homebrew/bin/flatc`.

## Gates

| Gate | Command | Result |
|---|---|---|
| Rust workspace tests | `cargo test --workspace --exclude void-tauri` | PASS — 45 test binaries green, 0 failures |
| Clippy | `cargo clippy --workspace --all-targets -- -D warnings` | PASS — clean, exit 0 |
| pnpm install | `pnpm install --frozen-lockfile` | PASS — exit 0 |
| pnpm build | `pnpm -r --if-present build` | PASS — all packages + `void-tauri` release build |
| pnpm test | `pnpm -r --if-present test` | PASS — void-studio 462/462 + all package suites |
| void-tauri bundle | `pnpm --filter void-tauri exec tauri build` (runs inside `pnpm -r build`) | PASS — `target/release/bundle/macos/VOID.app` (14.25 MiB) + `target/release/bundle/dmg/VOID_0.1.0_aarch64.dmg` (4.03 MiB) |
| Native engine | `cmake -S native/void-engine -B build/void-engine -G Ninja && ninja -C build/void-engine` | PASS — void-engine, void-render-fixture, void-recording-fixture all build (Apple paths: `-fno-aligned-allocation`, CoreAudio/CoreMIDI) |
| T14 render fixture | `void-render-fixture /tmp/t14.wav` | PASS — 1,536,000 frames @48 kHz, peak 0.392, sha256 6ee61f71… |
| Supervisor stub | `tests/harness/supervisor_stub.py build/void-engine/void-engine_artefacts/void-engine` | PASS — 15/15 checks |
| Crash fixture | `tests/harness/crash_fixture.py …/void-engine` | PASS — 6/6 checks |
| Recording suite | `tests/recording/recording_checks.py …/void-recording-fixture` | PASS — all scenarios (T31–T35) |
| Waveclip resolver | `tests/harness/waveclip_checks.py …/void-engine` | PASS — 9/9 checks (new harness, F5-N11) |

## Detached suites (`cargo test --manifest-path tests/<name>/Cargo.toml`)

| Suite | Result |
|---|---|
| av | PASS — all green after `fake_ffmpeg()` OnceLock fix |
| content | PASS |
| exchange | PASS |
| journeys/ai | PARTIAL — `f2_jobs_never_on_musical_clock` + 2 others pass; `f2_latency_per_model_cpu_box` fails at the transcribe/separate/generate workers — provisioning gate, see NEEDS |
| mix | PASS |
| models | PARTIAL — `t60_generate_procedural_deterministic_bytes` PASSES (procedural worker provisioned with numpy only); `t60_generate_musicgen_small_real_audio`, `t61_*`, `t62_*`, `w15_manifests_verify_and_resolve` fail — unprovisioned venvs/weights (F5-N10) |
| notation | PASS |
| producer | PASS |
| recovery | PASS |
| spatial | PASS |
| sync | PASS |
| visfx | PASS |
| visual | PASS |
| wasm | PASS |
| recording | N/A — python harness (see above), not a cargo suite |
| journeys/f1 | no `Cargo.toml` — shell/harness suite only |

## macOS fixes landed on this branch

1. **`crates/void-jobs/src/budget.rs`** — `monotonic_ns()` used the Linux
   `CLOCK_MONOTONIC=1` on all platforms; Darwin's id is 6 (1 is
   `CLOCK_REALTIME`). Now cfg-selected per OS; unknown platforms get `-1`
   → `EINVAL` → graceful degrade. `apply_rlimits` was Linux-only, so
   `RLIMIT_CPU` never applied on macOS (deadline-kill tests hung). Now
   applies on `linux|macos` with Darwin's resource ids (`RLIMIT_CPU=0`,
   `RLIMIT_AS=5` — Darwin's is 5, not Linux's 9).
2. **`crates/void-jobs/tests/runner.rs`** — `worker_exe()` spawned
   `cargo build` per test; cargo unlinks+relinks the binary every
   invocation, so 13 parallel tests raced and randomly found the binary
   missing. Single `OnceLock` build now. Verified: 3 consecutive clean
   runs of the 13-test binary.
3. **`crates/void-worker/src/transport.rs`** — `create_launch_dir`
   embedded the full 36-char launch UUID in the socket dir. macOS
   `$TMPDIR` is ~50 chars (`/var/folders/xx/…/T/`), so
   `base/void-test-socks/<uuid>/control.sock` overflowed `sun_path`
   (~104 B) → `InvalidInput "path must be shorter than SUN_LEN"`. Now
   uses the first 12 chars of the launch id (48 bits; full id still on
   the `Supervisor`). Same bug hit production — the Tauri app feeds the
   same `temp_dir()` base.
4. **`tests/av/src/lib.rs`** — `fake_ffmpeg()` had the same
   parallel-`cargo build` race as (2): `is_file()` could observe the
   unlink window. Same `OnceLock` fix.
5. **`tests/journeys/ai/tests/latency.rs`** — `f2_jobs_never_on_musical_clock`'s
   `allowed_dependents` list predates lanes U (`void-av`), `void-visfx`,
   `void-producer`; all are job-lane crates that legitimately use the
   runner. Added them to the allowlist (allowlist was stale, not a real
   coupling regression — the scan is `crates/`+`apps/`+`tests/`+`workers/`).
6. **`native/void-engine/src/session/Ops.cpp`** — F5-N11 waveclip
   resolver. `te::Edit::Options::filePathResolver` was never set, so
   `SourceFileReference` fell back to
   `getEditFileFromProjectManager(edit)` → empty File → `getChildFile`
   resolved process-CWD-relative → wave clips loaded nothing and
   rendered silence (F1 finding). Now `anchorAtEditFile` resolves
   relative refs against the live edit file's own path treated as a
   directory — matching the write side's anchor convention — in both
   `opCreateProject` and `opOpenProject`. `opOpenProject` additionally
   preflights every `AUDIOCLIP`'s `source` and rejects with
   `ErrorCode_ASSET_MISSING` naming the unresolved ref instead of
   opening to silence.
7. **`native/void-engine/tests/harness/waveclip_checks.py`** — new F5-N11
   evidence harness (9 checks): create → attach WAV → insert audio clip →
   save → assert persisted `source` is the container-relative anchor →
   kill → reopen must APPLY → delete blob → reopen must REJECT with
   `ASSET_MISSING`. 9/9 on the built binary.
8. **`apps/void-tauri/src-tauri/gen/schemas/macOS-schema.json`** —
   generated by `tauri build` on macOS; platform counterpart of the
   tracked `linux-schema.json`.

## Remaining NEEDS (honest gaps)

- **BB-N1 — `workers/audio` provisioning on this box is partial.**
  `generate-procedural` provisioned (numpy only) and passes. `transcribe`
  requires `tflite-runtime==2.14.0`, which publishes no wheel for
  python ≥3.12 on macOS arm64 — the box only has 3.12/3.14, so the venv
  can never satisfy the pin here. `separate` and `generate` need
  multi-GB HuggingFace weight snapshots (`snapshot_download` of
  HTDemucs-6s / MusicGen-small + t5-base) — not attempted on this box.
  Consequence: `tests/models` t60/t61/t62/w15 and
  `tests/journeys/ai::f2_latency_per_model_cpu_box` fail with
  `{dir} not provisioned` / missing weight paths. Same gate as F5-N10.
- **BB-N2 — `RLIMIT_AS` is a no-op on Darwin.** `setrlimit(RLIMIT_AS,…)`
  returns `EINVAL` for all values on macOS 26; Darwin ignores the
  address-space limit entirely. Memory budgets therefore cannot be
  enforced via rlimit on macOS — the deadline/kill path still bounds
  runtime (T50 passes), but a runaway allocator is unbounded by rlimit.
  Real enforcement needs a different mechanism (e.g. `task` memory
  limits via sandbox, or engine-side accounting). Documented, not fixed.
- **BB-N3 — void-tauri stays excluded from `cargo test --workspace`
  (F5-N13)** per the task brief; the real build is covered by the
  `tauri build` bundle step above.
- **BB-N4 — harness python needs `flatbuffers`.** PEP 668 blocks
  `--user` install on Homebrew python; used a throwaway venv
  (`python3 -m venv /tmp/fbenv && pip install flatbuffers`). Harness
  scripts document the requirement; no repo change.
