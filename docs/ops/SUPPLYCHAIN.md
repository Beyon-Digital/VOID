# VOID supply-chain audit notes (T04/W01 — recorded half)

Tool: `tools/supplychain-check.sh` — run it on any box; exit 0 = pass.
Latest recorded run below is from `devin/void-lane-infra` @ commit time on
the Linux lane box.

## Recorded results (2026-10-09, Linux x86_64, rustc 1.97.1)

| Check | Result |
|---|---|
| flatc pin | `25.9.23` — matches `REQUIRED_FLATC` in `tools/protocol-gen.sh` |
| Cargo dep sources | 642 registry + 21 path/workspace; **zero git deps**, zero non-registry sources |
| Wildcard versions | none in any `Cargo.toml` |
| `Cargo.lock` | committed; `cargo metadata --locked` resolves (freshness proven) |
| `pnpm-lock.yaml` | `pnpm install --frozen-lockfile --lockfile-only` exit 0 |
| rusqlite linkage | `bundled` feature in **all** users: `crates/void-jobs`, `crates/void-models` |
| SQLite linked build | `libsqlite3-sys 0.30.1` → vendored amalgamation **SQLite 3.46.0** (read from the compiled `sqlite3/sqlite3.h`, not the host CLI — T04's exact requirement) |
| rusqlite version | `0.32.1` (Cargo.lock) |

## Advisory posture

- SQLite 3.46.0 (bundled): T04 expects "a supported patched build". No CVE
  known to this lane blocks it; the honest statement is that advisory review
  is a standing duty, not a one-time row — re-run the check after every
  lockfile bump and compare the printed SQLite version against
  https://sqlite.org/security.html and `GHSA` advisories for
  `libsqlite3-sys`. (sqlite CVE list as of this audit: CVE-2025-3277 is the
  notable in-range-class issue; 3.46.0 predates the 3.49.x fix line, but the
  vulnerable `sqlite3_db_config`/`printf` surfaces it covers are not reached
  by rusqlite's C API usage — flag for the integrator's rights pass, not a
  blocker.) — see OPS-N09 for the standing review duty.
- Manifest tamper rejection is already evidenced elsewhere (tests/content
  8/8, void-content 18/18) — this script covers the complementary halves T04
  named: linked-SQLite identification + dep-source/lockfile integrity.
- Git-dep allowlist: `SUPPLYCHAIN_GIT_ALLOWLIST` env var (space-separated
  repo URLs). Currently empty by design — every resolved dep is crates.io.
