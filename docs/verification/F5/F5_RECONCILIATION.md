# F5 Reconciliation — full-parity sweep (W29, Lane Y)

**Branch:** `devin/void-lane-w29` · **Base:** `devin/void-implementation` @
`9099089` (head at audit time) · **Auditor:** Lane Y session (Linux x86_64,
Ubuntu, rustc 1.97.1, node 22.20.0, pnpm 9.0.0, flatc 25.9.23).

**Rules applied:** evidence only — statuses are reported, never upgraded;
`partial_*`, `blocked` and `not_run` are not-done. Every not-done test row maps
to a NEEDS entry (`NEEDS_MAP.json`, machine-checked by `trace_check.py`).

## Gate commands (this box, this branch)

| Command | Exit | Result |
|---|---|---|
| `pnpm install --frozen-lockfile` | 0 | clean, lockfile unchanged |
| `pnpm --filter void-client --filter void-core --filter void-daw --filter void-ui build` | 0 | prerequisite: void-studio vitest resolves `void-client` via its built `dist` entry |
| `cargo test --workspace --exclude void-tauri` | 0 | **146 passed / 0 failed** across 45 test binaries (20 crates; `void-tauri` excluded — GUI crate, see F5-N13) |
| `pnpm -r test` | 0 | all package vitest suites green (void-studio 41 files/462 tests, void-client, void-ui, void-core, void-daw, …) |
| `cargo test --manifest-path tests/<suite>/Cargo.toml` (13 detached suites) | 0 each | recovery 22/22 · mix 14/14 · producer 12/12 · exchange 12/12 · av 29/29 · content 8/8 · notation 18/18 · spatial 17/17 · sync 22/22 · visfx 31/31 · visual 14/14 · wasm 31/31 |
| `cargo test --manifest-path tests/models/Cargo.toml` | fail → **1/6** | provisioning-gated (F5-N10): procedural passes after its `setup.sh`; transcribe/separate/generate need multi-GB vendored weights |
| `python3 docs/verification/F5/trace_check.py` | 0 | 36/36 non-pass rows map to resolved NEEDS anchors; anomalies declared |
| `python3 docs/void-handoff/tools/render_views.py` | 0 | run before/after — tracking untouched, views regenerated identical (no diff) |

## Per-package reconciliation (W00–W29)

Sources: `tracking/TASKS.json` status + evidence strings, merge commits on
`devin/void-implementation`, lane-branch logs (all merged — no dangling work).

| WP | Ledger status | Implementing commits (lane merge `^2` tip / direct) | Tests (T-ids → status) | Gaps |
|---|---|---|---|---|
| W00 baseline/scope | done | `33acc07` direct | T01 not_run, T02 pass_linux | T01 row never flipped → F5-N14 |
| W01 dep rights/pins | done | `8fbf82d` (TE `e760754`+JUCE `37c894f`+flatbuffers 25.9.23; macOS TestRunner 450/22282) | T03 pass_macos, T04 not_run | SQLite linked-build advisory open → F5-N08; distribution rights blocked → F5-N12 |
| W02 Tauri shell+worker | done | `2fa4970`, `b3de97e` | T05 pass_linux, T06/T07 not_run | renderer-independence + IPC adversarial scenarios spec-unrun → F5-N07 |
| W03 protocol/IDs/views | done | `792b616` | T08–T12 not_run | covered at unit level, never run as written → F5-N07 |
| W04 musical model/device path | partial_macos | lane A merge `77a26bb` (`83bfe85`, `075872f`) | T13/T15/T16 partial_macos, T14 pass_macos | live device + callback measurement → F5-N02; waveclip resolver → F5-N11 |
| W05 assets/checkpoints | done | lane B merge `2570f24` (`87dc295`, `00443f1`) | T17–T21 pass_linux (tests/recovery 22/22) | none open |
| W06 plugin proof/scan | partial_macos | `2afd5e1` (lane A scope) | T22 pass_macos, T23 blocked, T24 pass_macos | native editor lifecycle + formats → engine #33/#34, F5-N02 |
| W07 CI/diagnostics/install | partial | `d2975ac`, `2d6a9ec`, `b3de97e` | T25–T30 not_run | build matrix, signing/updater, clean install, privacy sweep → F5-N03/N04/N05/N07 |
| W08 recording | partial_macos | lane E merge `94dd2e9` (`26e7eac`, `28753aa`) | T31–T35 pass_macos | in-process evidence only; arm/monitor wire ops → engine NEEDS 7–8 |
| W09 arrangement/piano-roll | done | lane D merge `0189ce4` (`225d8e3`) | T36–T39 pass_linux | none open |
| W10 instruments/mixer/export | partial | lane F merge `cb73c7c` (`391d8d6`, `d566f2c`) | T40–T43 pass_linux | playable-instrument/audio evidence is studio+export-side; engine render of instruments → producer/content NEEDS |
| W11 first-song/cutover | partial | lanes I `08821eb` + M `36ac2fd` | T44 pass_linux (57/57 offline, byte-identical PCM), T45/T47 partial_linux, T46 pass_linux, T48 blocked | 30-min device run + human participant → F5-N02/N06 |
| W12 job runtime/budgets | partial | lane G merge `227af02` (`70bcf73`) | T49–T51 not_run, T52 pass_linux (empty evidence → F5-N14) | wire SubmitJob/CancelJob → engine 9–11; GPU OOM → F5-N09 |
| W13 proposals | partial | lane H merge `98a7d68` (tip `b185761`) | T53–T56 pass_linux | transient audition layer + proposal wire → engine 12–14 |
| W14 gestures/patterns | partial | lane K merge `84db50b` (tip `e3cc02c`) | T57–T59 pass_linux | scene/launch scheduling → engine 26; camera later → visfx W23-02 |
| W15 audio AI adapters | partial | lane J merge `c593945` (tip `589ce3a`) | T60–T62 pass_linux | provisioning gate → F5-N10; six-stem quality spot-check note in ai-jobs/EVIDENCE.md |
| W16 F2 regression gate | partial | lane L merge `36c1f89` (tip `f434bee`) | T63 pass_linux (journeys/ai 18/18), T64 blocked, T65 pass_linux | human session → F5-N06 |
| W17 recording/arrangement depth | partial | lane N merge `95ccc2d` (tip `ba2abc1`) | T66–T68 pass_linux | fades/loops/alias/folders/sections/alternatives/scenes/copy-notes/freeze/capture/loudness → engine 18–31 |
| W18 mixer/automation/MIDI | partial | lane P merge `bd9540c` (`77ab63c`) | T69–T71 pass_linux | send/bus/latency/automation-lane/MPE/articulation ops → mix M1–M10 |
| W19 stock content/audio-edit | partial | lane Q merge `dd4d626` (`fc49c3c`) | T72 not_run, T73 pass_linux, T74 partial_linux | stock DSP conformance + licensed packs + engine renderers → content-rights 1–10 |
| W20 plugin exchange/isolation | partial | lane R merge `2b323b0` (`59f0a80`) | T75/T76 partial_linux, T77 pass_linux (.dawproject zero-loss) | isolation runtime + real plugin hosting → engine 32–35 |
| W21 accompaniment/producer | partial | lane S merge `7bfe8ea` (`b07c972`) | T78–T80 pass_linux | accompaniment/mastering/import exec ops → producer 1–10 |
| W22 visual engine | partial | lane O merge `54822c3` (tip `905f44c`) | T81–T83 pass_linux | wgpu surface/preview-program plumbing → visfx W23-01/W23-04 |
| W23 reactive visuals/camera | partial | lane T merge `0c7c9bf` (`6e567b5`) | T84/T86 pass_linux, T85 partial_linux | projectM ADR evidence-only → visfx W23-05; camera tracker → W23-02 |
| W24 AV export | partial | lane U merge `d63b12f` (`349b76a`) | T87–T89 pass_linux | GPU-program feed + AV view kinds/job telemetry → av NEEDS 1–7 |
| W25 notation/interchange | partial | lane V merge `d7957de` (`096de33`) | T90/T91 pass_linux | Verovio GUI + AAF/FCPXML + score ops wire → notation NEEDS-01/03/05; .logicx intentionally blocked NEEDS-04 |
| W26 spatial | partial | lane W merge `099096e` (`7057034`) | T92/T93 partial_linux | licensed validators + head-tracking HW → show NEEDS 1–4 |
| W27 sync/show | partial | same merge `099096e` | T94–T96 pass_linux | MIDI/DMX/OSC endpoints + Ableton Link ADR → show NEEDS 5–10 |
| W28 WASM extensions | partial | lane X merge `67ac8b3` (tip `43ac142`) | T97 pass_linux, T98 partial_linux | ARA + plugin-export gates (ADRs) → wasm N4/N5; wire ops → N1–N3 |
| W29 this audit | not_started → this lane | this branch | T99/T100 not_run → audit delivered, integrator acceptance pending → F5-N15 | — |

## Findings (reported, not fixed)

1. **Ledger drift (F5-N01).** Features (116), stock groups (22), infrastructure
   (20), dependencies (41) and platform rows (3) were never updated post-merge —
   all sit at `not_verified`/`unqualified`/`not_tested`. This report derives
   their state from test rows + commits; `TRACE_AUDIT.json` carries both fields
   per row.
2. **T52 anomaly.** Only pass row without an evidence string — declared in
   `NEEDS_MAP.json.evidence_anomalies`; integrator should fill or downgrade.
3. **T01 anomaly.** `not_run` while W00 is `done` — the baseline procedure ran
   (commits exist) but the row was never flipped.
4. **Provisioning gate (F5-N10).** `tests/models` fails 5/6 on a fresh box until
   `workers/audio/*/setup.sh` runs (up to ~3 GB of vendored weights). The
   lane-recorded pass stands; reproduction requires provisioning.
5. **README staleness.** The "honest snapshot" block predates lane merges —
   says no Tauri shell/engine exists. The appended *Verification and release
   readiness* section supersedes it with dated pointers.
6. **Distribution rights open (F5-N12).** Tracktion GPL-3.0 / JUCE AGPL-3.0
   dual licences: development use fine, distribution BLOCKED pending
   review/purchase — DEPENDENCIES.json `license_approval` unset on all 41 rows.
7. **Known engine bug (F5-N11).** `docs/engine/F1_WAVECLIP_RESOLVER_GAP.md`:
   wave clips render silence post-reopen until `Edit::filePathResolver` is set
   in open/create paths.

## Status vocabulary note

`pass_macos` rows (T03, T14, T22, T24, T31–T35) are engine-qualified evidence
from the macOS lane — legitimate passes, not portability claims. `partial_*`
and `not_run` rows are individually explained in `NEEDS_MAP.json`; nothing is
counted as done that has not run.
