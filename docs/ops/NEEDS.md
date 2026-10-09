# NEEDS — gaps recorded by the post-W29 infra lane (Lane AA)

Same convention as `docs/engine/NEEDS.md`: numbered append-only entries.
Each states what exists today, what is needed, and which TEST_MATRIX row it
explains. Owner for resolutions: integrator unless noted. Written on
`devin/void-lane-infra` off `devin/void-implementation` @ `a0e1923`.

## OPS-N01 — Real signing key material (authorization-gated)

**What exists:** updater wiring complete (`UPDATER.md`): plugin registered,
`createUpdaterArtifacts`, endpoint, placeholder pubkey, capability grant,
release workflow passing `TAURI_SIGNING_PRIVATE_KEY(_PASSWORD)` envs.
**Needed:** `tauri signer generate` run by a key holder + the two repo
secrets provisioned + pubkey pasted into `tauri.conf.json`. Requires owner
authorization (D06 — key material controls release authority).
**Explains:** T28 cannot pass; unsigned draft bundles only.

## OPS-N02 — T28 update-drill suite unrun

**What exists:** config + docs only.
**Needed:** tampered-payload rejection, interrupted-download recovery, and
the **install/restart-while-recording block** — there is currently no hook
between the updater and session/recording state (T28 expects refusal during
a take; nothing implements it).
**Explains:** T28 `not_run`.

## OPS-N03 — Matrix jobs have never executed on real runners

**What exists:** `rust-windows`, `rust-macos`, `ts-macos`, `ts-windows`,
`audit`, `bundle` declared and actionlint-clean.
**Needed:** first green (or diagnosed-red) runs recorded per OS with runner
images/toolchain versions — that output IS the T25 evidence.
**Explains:** T25 `not_run` until then.

## OPS-N04 — No Windows supervisor transport

**What exists:** `void-worker` uses `tokio::net::UnixStream`/`UnixListener`
unconditionally (`crates/void-worker/src/transport.rs`); `void-app` and
`void-tauri` inherit the limitation. `rust-windows` excludes exactly these.
**Needed:** named-pipe/TCP-localhost transport parity or a documented
Windows design — a named pipe keeps the same per-launch secret + scoped-peer
semantics (CONTRACTS/W02 §103-108).
**Explains:** Windows qualification blocked even when `rust-windows` greens.

## OPS-N05 — Timeout/cancellation drill unrun (T26)

**What exists:** per-job `timeout-minutes` (CI_TIMEOUTS.md), concurrency
groups with `cancel-in-progress`, secret-free `ci.yml`, secrets confined to
tag-triggered `release.yml`.
**Needed:** the harmless deliberate-timeout branch + an observed
superseded-run cancellation + fork-PR secret-isolation check.
**Explains:** T26 `not_run` (mechanisms exist, evidence doesn't).

## OPS-N06 — Clean-machine install test unrun (T27)

**What exists:** release bundles (draft, unsigned) + `INSTALL.md`
per-OS install/verify/run paths incl. flatc-free end-user statement.
**Needed:** install each artifact on a clean VM (no dev toolchain), launch
offline, record exit/screens/OS versions.
**Explains:** T27 `not_run`.

## OPS-N07 — Engine worker is not packaged in the Tauri bundle

**What exists:** `tauri.conf.json` has no `externalBin`/`resources`; the app
spawns an engine path via `spawn_engine`. Release bundles ship the shell
only.
**Needed:** decision on engine distribution — sidecar `externalBin` per
platform (needs per-OS engine artifacts in the release job) vs separate
installer; then bundle wiring.
**Explains:** T27's "all selected workers/runtimes load" cannot pass today.

## OPS-N08 — T29 runtime privacy drill unrun

**What exists:** `tools/privacy-audit.sh` static sweep (committed in CI
`audit` job): no telemetry SDKs, no off-process diagnostic sinks, no
PII-shaped literals, no committed key material; 1 warn (`env::vars().count()`
in `workers/ffmpeg-fake`, a boundary-test fixture).
**Needed:** inject private paths/prompts/audio + credential-shaped tokens
into a running build and inspect emitted logs/crash bundles; also the
diagnostic-export-must-be-user-action check (no export path exists yet —
flagged by the script itself).
**Explains:** T29 `not_run` (tooling half done, runtime half open).

## OPS-N09 — Standing advisory review for bundled SQLite

**What exists:** `supplychain-check.sh` reports the vendored SQLite version
per lockfile (3.46.0 today) and asserts `bundled` on every rusqlite dep.
**Needed:** each lockfile bump re-checks the printed version against
sqlite.org/security + GHSA advisories for `libsqlite3-sys` — a duty, not a
one-time pass. CVE-2025-3277 (3.49.x fix line) is the current flag item to
re-verify before release; rusqlite's C API surface does not reach the
affected functions per this lane's read, integrator confirms.
**Explains:** T04 advisory half (the rest of T04 is evidenced — SUPPLYCHAIN.md).
