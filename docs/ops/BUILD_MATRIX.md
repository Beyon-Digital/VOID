# VOID CI build matrix (T25/W07 — infrastructure side)

**Status:** jobs declared in `.github/workflows/ci.yml` + `release.yml`. This
document covers the matrix *shape* and why each job exists. It is not
qualification evidence — a job existing is not a platform verified. Per-OS
pass/fail runs on real runners are the T25 evidence and stay NEEDS until the
jobs report green on main (`docs/ops/NEEDS.md` OPS-N03).

## Job inventory

| Job | Runner | Steps (same semantics as the pre-existing jobs) | Why it exists |
|---|---|---|---|
| `rust` | ubuntu-latest | fmt, clippy `-D warnings`, `cargo test --workspace --exclude void-tauri` | Pre-existing Linux gate; WebKitGTK installed so clippy covers `void-tauri` too |
| `ts` | ubuntu-latest | pnpm install → `pnpm -r build` → void-tauri `vite build` + `tsc --noEmit` → `pnpm -r test` | Pre-existing TS gate |
| `native` | macos-14 | xcode-select newest, flatc 25.9.23, `tools/protocol-gen.sh`, cmake+ninja engine+scanner build+ctest | Pre-existing engine job |
| **`rust-windows`** | windows-latest | flatc 25.9.23 (VS-generator build → PATH), `cargo fmt --check`, `cargo check` + `cargo test` on the portable subset | T25 Windows leg. `void-worker` (Unix-domain-socket transport in `crates/void-worker/src/transport.rs`) and its dependents `void-app`/`void-tauri` **cannot compile on Windows** — the job excludes exactly those three and tests every other workspace crate (superset of the `-p void-protocol -p void-jobs -p void-notation` subset). |
| **`rust-macos`** | macos-14 | flatc 25.9.23, `cargo test --workspace --exclude void-tauri` | T25 macOS leg — same crate set as the Linux rust gate |
| **`ts-macos`** | macos-14 | identical step list to `ts` | T25: frontend parity on macOS |
| **`ts-windows`** | windows-latest | identical step list to `ts` | T25: frontend parity on Windows |
| **`audit`** | ubuntu-latest | flatc, `tools/privacy-audit.sh`, `tools/supplychain-check.sh` | T04/T29 tooling wired into CI |
| `bundle` (release.yml) | 3× matrix | per-OS toolchain + `tauri-action` → draft release | T25 packaging leg / T27 artifact source |

## Honest boundaries

- **`rust-windows` is a portability check, not Windows qualification.** It
  proves the portable crates compile/test on windows-latest runners. The
  supervisor (`void-worker`) is POSIX-socket-only; Windows parity requires a
  named-pipe transport, which is unimplemented (NEEDS OPS-N04). Do not read a
  green `rust-windows` as "VOID runs on Windows".
- **`rust-macos` excludes `void-tauri`** like the Linux job — the crate
  compiles on macOS in principle, but the gate asserts tests, not a GUI build.
  The signed `.app` qualification remains NEEDS.
- **Nothing here runs audio, a GUI, a real plugin host, or hardware** — those
  rows stay `not_run`/blocked in TEST_MATRIX regardless of job color.
- `pnpm -r --filter=!void-tauri --if-present build` must precede `pnpm -r test`
  on every TS job: `void-studio` vitest resolves `void-client` via its built
  `dist/` entry (main field), so a bare `pnpm -r test` on a fresh checkout
  fails resolution. Same reason the release jobs build deps before
  `tauri-action`.
- flatc is pinned at **25.9.23** everywhere (`tools/protocol-gen.sh` asserts
  it; `tools/supplychain-check.sh` re-verifies). Windows gets it via the
  default VS generator + `Release/` PATH append (no sudo on windows runners —
  the one deliberate divergence from the verbatim recipe).
- Timeouts: `rust-windows`/`rust-macos` 30 (unit/contract budget per
  HANDOFF §9), `ts-*` 15, `audit` 15, `bundle` 45 (native build budget).
  Cancellation semantics: `docs/ops/CI_TIMEOUTS.md`.
