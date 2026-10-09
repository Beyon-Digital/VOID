# VOID CI timeout and cancellation budget (T26/W07 — documentation half)

Per-job deadlines follow HANDOFF §9 (15 lint/schema, 30 unit/contract,
45 native build, 60 soak). Every job in both workflows carries an explicit
`timeout-minutes`; none relies on the 6-hour runner default.

## ci.yml

| Job | timeout-minutes | Rationale |
|---|---|---|
| `rust` | 15 | fmt + clippy + workspace unit tests (lint/unit tier) |
| `ts` | 15 | pnpm install + build + vitest |
| `native` | 60 | macOS engine + scanner C++ builds + ctest (native tier) |
| `rust-windows` | 30 | flatc build + cargo check/test portable subset (unit tier; cold-cache headroom) |
| `rust-macos` | 30 | flatc build + workspace tests (unit tier) |
| `ts-macos` | 15 | mirrors `ts` |
| `ts-windows` | 15 | mirrors `ts` |
| `audit` | 15 | flatc build + two shell audit scripts |

## release.yml

| Job | timeout-minutes | Rationale |
|---|---|---|
| `bundle` (3 OS legs) | 45 | tauri build incl. bundler + signing steps (native build tier) |

## Cancellation and supersede behavior

- `ci.yml`: `concurrency: ci-${{ github.workflow }}-${{ github.ref }}` with
  `cancel-in-progress: true` — a new push on the same ref cancels the whole
  in-flight run (all jobs, including the matrix legs).
- `release.yml`: `concurrency: release-${{ github.ref }}` with
  `cancel-in-progress: true` — re-pushing a tag supersedes the prior
  packaging run; legs share one group so no orphaned per-OS bundle uploads.
- Matrix `fail-fast: false` on the release matrix: one leg's failure must not
  cancel the other OS bundles — a cancelled leg produces no draft asset, and
  partial releases are avoided by cancellation not by sibling abort.

## Secret boundary (T26 secret-isolation expectation)

- `ci.yml` uses **no secrets at all** — fork/untrusted PR runs receive nothing
  beyond `GITHUB_TOKEN` (default `contents: read` scope for PRs).
- `release.yml` carries `TAURI_SIGNING_PRIVATE_KEY(_PASSWORD)` and declares
  `permissions: contents: write` — it triggers only on `v*` tags and
  `workflow_dispatch`, i.e. it never runs on pull_request refs. Signing
  secrets therefore cannot reach untrusted-code jobs; they are also absent
  (unset) until the owner provisions them (OPS-N01).
- No privileged/self-hosted runners are used; every job runs on
  GitHub-hosted `*-latest`/`macos-14` images.

## Not yet done (T26 runtime half — NEEDS OPS-N05)

The deliberate-timeout drill — a harmless branch that sleeps past a
`timeout-minutes` to prove cancellation fires — plus a real superseded-run
cancellation observation on a PR. Both are live-CI evidence this lane cannot
produce; documented here so the drill is unambiguous to run.
