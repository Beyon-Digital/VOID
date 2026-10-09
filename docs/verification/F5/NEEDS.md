# NEEDS — gaps recorded by the W29 reconciliation lane (Lane Y)

Same convention as `docs/engine/NEEDS.md`: numbered append-only entries. Each
entry states what exists today, what is needed, and which tracking rows it
explains. Nothing here is a pass and nothing below was invented to fill a gap —
entries reference real files/commits where they exist and say so where they do
not.

Owner for resolutions: integrator unless noted. Written on `devin/void-lane-w29`
against `devin/void-implementation` @ `9099089`.

## F5-N01 — Ledger drift below task/test level

**What exists:** `tracking/TASKS.json` and `tracking/TESTS.json` were maintained
by lanes. `tracking/FEATURE_TRACEABILITY.json` (all 116 features + 22 stock
groups at `not_verified`), `tracking/INFRASTRUCTURE.json` (18 `not_verified`,
2 `optional_not_authorized`), `tracking/DEPENDENCIES.json` (all 41 libraries
`unqualified`, `license_approval` unset) and `tracking/PLATFORMS.json` (all
three targets `not_tested`) were never updated after lanes landed — rows sit at
their packet defaults even where matching test rows pass and commits exist.
**Needed:** an integrator pass that reconciles sub-test ledgers against
`TRACE_AUDIT.json` per-package evidence before any release-readiness claim.
**Explains:** the reconciliation derives feature/stock/infra state from test
rows + commits rather than trusting `implementation_status` fields.

## F5-N02 — Platform qualification matrix is empty below macOS-lane evidence

**What exists:** `pass_macos` evidence (T03, T14, T22, T24, T31–T35) produced
on the macOS lane box (Xcode 26.6, Tracktion Engine `e760754` + JUCE `37c894f`).
Windows x64 has zero evidence; the named-Linux target is software-verified but
audio-device/GUI evidence is absent (no device, no display qualification).
`PLATFORMS.json` records all three targets `not_tested`.
**Needed:** per-target qualification runs recorded in `PLATFORMS.json` with
exact OS/toolchain/device/driver versions; Windows build bring-up is unstarted.
**Explains:** T13, T15, T16 (partial_macos — no live audio device on the
qualifying box), T23, T25, T30, T45, T76 partial/blocked rows.

## F5-N03 — Packaging, signing, updater (deferred-by-approval)

**What exists:** `tauri build` produced unsigned `.deb/.rpm/AppImage` bundles
(commit `b3de97e`, `docs/f0/EVIDENCE.md`). No OS signing, notarization, updater
manifest/signing, or clean-machine install evidence exists.
**Needed:** signing credentials + store/distribution authorization — both
blocked by handoff D06 (explicit owner authorization required), plus a
clean-machine install run per T27 and the updater tamper/interrupt suite T28.
**Explains:** T27, T28 `not_run`; T25 partially (native build matrix per-OS).

## F5-N04 — Diagnostic privacy sweep unrun

**What exists:** logs are bounded/redacted by convention but no adversarial
log/crash-bundle inspection has run.
**Needed:** the T29 procedure: inject private paths/prompts/credential-shaped
tokens, inspect emitted logs and crash bundles for leakage.
**Explains:** T29 `not_run`.

## F5-N05 — CI timeout/cancellation drill unrun

**What exists:** `.github/workflows/` jobs carry `timeout-minutes`; the
deliberate-timeout trigger test and superseded-run cancellation check were
never executed on a real PR.
**Needed:** run a harmless timeout branch; record superseded-run cancellation
and untrusted-PR secret isolation in CI logs.
**Explains:** T26 `not_run`.

## F5-N06 — Human-participant usability rows

**What exists:** runnable protocols (`docs/verification/F2/USABILITY_PROTOCOL.md`)
but no human musician session has been executed; inventing a participant is
explicitly disallowed.
**Needed:** a real participant session for first-song (T48) and musical-
assistance (T64) tasks; gate T47's remaining closure on T48.
**Explains:** T48, T64 `blocked`; contributes to T47 `partial_linux`.

## F5-N07 — Spec-level protocol/lifecycle rows never executed end-to-end

**What exists:** components are covered by unit suites (void-protocol codec 5/5,
void-client dedup/revision vitest, void-worker supervisor_lifecycle + wire
roundtrip on Linux), but the written scenarios were never run as described:
T06 renderer reload during live playback, T07 adversarial IPC peers, T08–T12
in-order validation/idempotency/stale-revision/queue-flood/timebase scenarios,
T30 F0 vertical slice on a real device, T49/T51 job machine + model-acquisition
rows (covered at journey level by `tests/journeys/ai` 18/18 and `tests/models`,
rows never updated).
**Needed:** run each scenario as specified on a qualified target, or split rows
into suite-verified subclaims with the ledger updated honestly.
**Explains:** T01 (baseline row never flipped though W00 ran it), T04, T06–T12,
T25, T30, T49, T51 `not_run`.

## F5-N08 — SQLite linked-build + advisory review open

**What exists:** `docs/dependencies/PINS.md` records the open TODO — the actual
bundled SQLite build via `rusqlite` is not yet identified/reviewed against
advisories; manifest-tamper rejection IS covered (`tests/content` 8/8,
void-content 18/18).
**Needed:** `pnpm doctor`-style recorded linked-version + advisory sweep; the
dependency manifest tamper half is already evidenced.
**Explains:** T04 `not_run`.

## F5-N09 — GPU admission / OOM contention unrun

**What exists:** CPU/RAM admission is real and tested (`tests/journeys/ai`;
`RLIMIT_AS`/`RLIMIT_CPU` kills verified); GPU VRAM contention is modelled
(`vram_bytes` honest-zero under rlimits — visfx NEEDS W23-01).
**Needed:** real GPU-OOM contention fixture on hardware with a discrete GPU.
**Explains:** T50 `not_run`.

## F5-N10 — `tests/models` is provisioning-gated

**What exists:** worker bundles `workers/audio/*` carry `setup.sh` provisioning
(venv + vendored weights, all gitignored). On this fresh box:
`t60_generate_procedural_deterministic_bytes` passes after
`generate-procedural/setup.sh` (numpy-only); the other five tests fail with
explicit "not provisioned" panics until `setup.sh` runs (basic-pitch `.tflite`,
htdemucs_6s 53MB, musicgen ~1.9GB + T5 ~891MB downloads).
**Resolved 2026-10-09:** provisioning is a documented suite precondition —
the test file header already states `workers/audio/<dir>/setup.sh` is required
and fails loudly (never skipped/faked). Verified 6/6 green on the integrator
box with all three worker venvs provisioned (`generate`, `separate`,
`transcribe` + vendored weights). Each fresh box runs setup.sh once; that is
the reproduction contract, not a code gap.

## F5-N11 — Engine waveclip resolver gap (blocks real-media reopen) — RESOLVED (lane BB)

**What exists:** `docs/engine/F1_WAVECLIP_RESOLVER_GAP.md` — `EngineSession`
never set `Edit::filePathResolver`, so relative wave-clip sources resolved
CWD-relative and rendered silence post-reopen.
**Resolution (lane BB, macOS):** resolver wired in `Ops.cpp` open/create
paths; `native/void-engine/tests/harness/waveclip_checks.py` proves
reopen-with-media APPLIED and deleted-blob → REJECTED `ASSET_MISSING`
(9/9 green on macOS). Evidence: `docs/verification/macos/EVIDENCE.md`.

## F5-N12 — Public release / store submission / licence purchase unauthorized

**What exists:** this audit + `docs/release/RELEASE_CANDIDATE.md`. Handoff D06:
relicensing, commercial licence purchase (Tracktion GPL-3.0 / JUCE AGPL-3.0
dual-licence distribution is BLOCKED pending review — `docs/dependencies/PINS.md`),
binary publishing and store submission all need explicit owner authorization.
**Needed:** owner decision on distribution rights + authorization before any
public release work; internal unsigned testing is the current ceiling.
**Explains:** T100's release-blocking boundary; DEPENDENCIES `license_approval`
unset on all 41 rows.

## F5-N13 — `void-tauri` crate excluded from the cargo gate

**What exists:** `cargo test --workspace --exclude void-tauri` is the agreed
gate; the GUI crate needs WebKitGTK/display and builds via `pnpm -r build`
(`vite build && tauri build`), excluded from `pnpm -r` gates on this box.
**Resolved 2026-10-09:** the crate has zero unit tests — the exclusion was
unnecessary. `cargo test -p void-tauri` compiles + links the test targets with
the WebKitGTK deps CI already installs for clippy and reports 0 tests, so the
gate is now plain `cargo test --workspace` (ci.yml). Display-needed coverage
remains the e2e UI drive (void-tauri-testing skill).

## F5-N14 — Two ledger anomalies found by this audit (reported, not fixed)

**What exists:** T01 is `not_run` while owning task W00 is `done` with baseline
commits (`33acc07`); T52 is `pass_linux` with an **empty evidence field** — the
only pass row without evidence text. Per the lane's rules these are reported,
not corrected: statuses are integrator-owned.
**Needed:** integrator fills T01 evidence (packet install + baseline run) and
T52's evidence string (provenance/privacy suite reference), or downgrades.
**Explains:** T01, T52 anomalies in `TRACE_AUDIT.json`.

## F5-N15 — T99/T100 acceptance is an integrator decision

**What exists:** the reconciliation deliverables themselves — `trace_check.py`
(exit 0), `TRACE_AUDIT.json`, `F5_RECONCILIATION.md`, `FEATURE_MATRIX.md`,
`RESIDUAL_RISKS.md` and `docs/release/RELEASE_CANDIDATE.md` — produced on
`devin/void-lane-w29` against `devin/void-implementation` @ `9099089`.
**Needed:** integrator review + merge of this lane, then TESTS.json status
updates for T99/T100 by the integrator (this lane may not update tracking).
**Explains:** T99/T100 `not_run` — the audit exists, acceptance is pending.
