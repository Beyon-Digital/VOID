# W12 — AI job runtime, budgets and provenance — evidence

Scope: `crates/void-models`, `workers/` (argv protocol + `void-fake-worker`
reference), `crates/void-jobs` extension (admission, budget, runner,
provenance), `packages/void-studio/src/jobs/`, `packages/void-ui` additive
components. TEST_MATRIX rows T49–T52.

Test host: Linux, cargo 1.97.1, node v22.20.0, pnpm 12.9.1,
rusqlite 0.32 bundled (SQLite build per `JobDb::sqlite_version`).

## Commands run (all exit 0 unless noted)

| Command | Exit | Result |
|---|---|---|
| `cargo build --manifest-path workers/fake/Cargo.toml` | 0 | `void-fake-worker` debug binary |
| `cargo test -p void-models` | 0 | 12 passed, 0 failed |
| `cargo test -p void-jobs --test runner` | 0 | 13 passed, 0 failed |
| `cargo test --workspace --exclude void-tauri` | 0 | all suites green (44 total incl. W12) |
| `pnpm -r test` | 0 | void-client 17 · void-studio 126 · void-daw 4 · void-ui/void-core pass |
| `pnpm --filter void-ui build` | 0 | tsc clean |
| `pnpm --filter void-client build` | 0 | tsc clean (needed before studio tests) |

## Real artifacts produced (genuine per-run generation — never canned)

`void-fake-worker` renders a real 16-bit PCM mono sine WAV and a real
SMF-0 MIDI file from spec parameters. Verified hashes on this box:

- `out.wav` (250 ms, 440 Hz, 48 kHz, seed 0): **24044 bytes**, SHA-256
  `335048c14b264b3b73f46ddbe4a71cf08271a88e20b1165f68f8eba9be962d8a`
  (RIFF header asserted in `t49_happy_path_real_sine_wav_and_provenance`)
- `out.mid` (seed 3 pattern): SHA-256
  `e791e9d2568b2f8a079fa88876695197734d412c80a7729447b1da706b11aa94`

## TEST_MATRIX coverage

- **T49 state machine** — cancel queued (direct), cancel running
  (token + `cancelling` → SIGKILL → `cancelled`), late result quarantined
  with no publish, kill-worker-no-result = failed, single terminal
  outcome (re-run rejected), stray files counted not committed, dedup
  by content hash. `crates/void-jobs/tests/runner.rs` — 13 tests.
- **T50 admission** — `admission::decide`: ≤4 waiting, ≤1 heavy,
  ≤2 small (§2); `Busy` on a full queue. Budget kills verified for real:
  `RLIMIT_CPU` on the spin fixture, `RLIMIT_AS` on the oom fixture,
  monotonic deadline kill on the sleep fixture.
- **T51 acquisition/isolation** — manifest SHA-256 verify (tamper →
  `ManifestTampered`), executable allowlist (`[A-Za-z0-9._-]+` basename,
  no paths/`..`/remote fields — no download-time code execution),
  optional-vs-required artifact status resolution
  (`missing`/`degraded`/`rejected`/`available`), staging-escape artifact
  declaration rejected by the runner. `void-models` tests + runner
  escape test.
- **T52 provenance/privacy** — `jobs/<jobId>/provenance.json` +
  `result.json` publish only on verified success: model id+version,
  params echo, input refs, per-artifact SHA-256, argv SHA-256, budget
  echo + measured wall time. No cloud/network installs exist anywhere
  in the lane; `cloudConsentId` is spec-passthrough only and the local
  runner never reaches a network.

## Honest gaps

- Protocol union has no job ops — recorded in `docs/engine/NEEDS.md`
  items 9–11 (SubmitJob/CancelJob ops, JOB_LIST/MODEL_LIST views,
  JobEvent telemetry). Studio builds the intended shapes defensively.
- RLIMIT enforcement is Linux-only (`cfg(target_os="linux")`); on other
  platforms deadline/cancel kills still apply, memory/CPU caps don't.
- `internal` runtime kind is declared in the descriptor but the only
  implemented runner path is argv (the only kind a local worker can be).
