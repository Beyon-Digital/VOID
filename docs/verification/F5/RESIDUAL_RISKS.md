# Residual risks — W29 / F5 (Lane Y)

**Rule:** no blanket equivalence claims. Everything below is *not done* — a
deferred engine half, a blocked gate, a missing hardware run, or an
intentionally unsupported interop surface — and each item names its owning
NEEDS entry. Nothing here is a surprise discovered by the audit; this list
consolidates what the lanes recorded.

## A. Unsupported interop (spec/ADR says not implemented — do not claim)

| Item | State | Owning NEEDS entry |
|---|---|---|
| Ableton Link sync | `NotImplemented` by ADR; timing-share protocol deferred | `docs/show/NEEDS.md` (Link — transport lockstep) |
| Verovio score engraving GUI | music-engraving surface not built; MusicXML/score model only | `docs/notation/NEEDS.md` NEEDS-notation-01 (Verovio) |
| projectM preset engine | ADR: evidence-only, no runtime integration | `docs/visfx/NEEDS.md` W23-05 |
| ARA2 plugin bridge | gated by ADR-0001 — host-side ARA not built | `docs/wasm/NEEDS.md` N4 |
| WASM → VST3/CLAP plugin export | gated by ADR-0002 — no plugin-format exporter | `docs/wasm/NEEDS.md` N5 |
| Plugin isolation runtime | policy/spec shipped; no supervised out-of-process host | `docs/engine/NEEDS.md` #33 |
| Head-tracking hardware | spatial head-track backend requires hardware run | `docs/show/NEEDS.md` (head-tracking item) |
| Live-device audio verification | no audio interface on the audit box; latency/topology measurement deferred | `docs/verification/F5/NEEDS.md` F5-N02 |

## B. Engine half deferred (model/policy/op layer verified; Tracktion side open)

| Surface | What exists | What's open | NEEDS |
|---|---|---|---|
| Job runtime | admission/budgets/idempotency + mock worker | `SubmitJob`/`CancelJob`/`PROPOSAL_LIST`/`JobEvent` wire ops | engine #9–#11, #13 |
| Recording depth | takes/journal (macOS in-process) + model ops | fades, loops, aliases, folders, sections, alternatives, scenes, copy-notes, freeze, capture, loudness ops | engine #18–#31 |
| Mixer/automation | studio views + model | sends/buses, latency comp, automation-lane write, MPE/articulation engine ops | `docs/mix/NEEDS.md` M1–M10 |
| Producer tools | accompaniment/mastering/import model | engine execution ops + assistive model paths | `docs/producer/NEEDS.md` 1–10 |
| Stock content | catalog + stock-processor model | per-processor DSP conformance (T72), licensed packs, six-stem | `docs/content-rights/NEEDS.md` 1–10 |
| Visual output | deterministic composer, drop-counted | real wgpu surface, camera tracker, analysis feed, preview/program plumbing | `docs/visfx/NEEDS.md` W23-01…W23-04 |
| AV export | ffmpeg argv-only exporter + rights flags | GPU-program feed, AV view kinds, job telemetry | `docs/av/NEEDS.md` 1–7 |
| Spatial | layouts/ADM gates model | licensed Dolby/ADM validators, monitor reachability | `docs/show/NEEDS.md` 1–2 |
| Sync transports | byte-exact MTC/MMC codecs | real MIDI/DMX/OSC endpoint runs, panic wiring | `docs/show/NEEDS.md` 5–8 |
| WASM host | real wasmtime host + capability gates | wire `PLUGIN_*` ops into coordinator | `docs/wasm/NEEDS.md` N1–N3, N6–N10 |

## C. Spec-level rows never run (F5-N07)

T06–T12 (lifecycle/IPC/protocol scenarios), T25–T30 (CI/packaging/privacy
scenarios), T49/T51 (job-runtime acceptance), T01 (baseline) — components are
green at unit level but the scenario rows as written were never executed. These
need the macOS/UI hardware lane or a dedicated QA pass; reported, not covered
up.

## D. Provisioning / process gates

| Gate | Owner |
|---|---|
| `tests/models` 5/6 needs `workers/audio/*/setup.sh` (vendored weights, GBs) | F5-N10 |
| Human participant for first-song/cutover drill (T45/T48, T64) | F5-N06 |
| Build-matrix CI drill + signed installer/update path (T26–T28) | F5-N03/N05 |
| Diagnostic-bundle privacy sweep (T29) | F5-N04 |
| Platform matrix: macOS spot rerun + Windows bring-up (PLATFORMS all `not_tested`) | F5-N02 |
| Public release/store submission/relicensing — requires explicit authorization (D06) | F5-N12 + `docs/content-rights/NEEDS.md` (ledger LGR-0001…0004 status `required`) |
| Sub-test ledger maintenance (features/infra/deps/platforms stale at `not_verified`) | F5-N01 |

## E. Known defect carried into release candidate

`docs/engine/F1_WAVECLIP_RESOLVER_GAP.md` — wave clips render silence after
project reopen on engine until `Edit::filePathResolver` is wired in
open/create. Fix scope is engine-lane; the risk is recorded here so the release
doc can name it as a known issue. Owner: F5-N11.

## F. Rights carry-over (blocks distribution, not dev)

- Tracktion Engine GPL-3.0 / JUCE AGPL-3.0 dual licences — distribution blocked pending licence review or purchase (`docs/dependencies/PINS.md`, F5-N12).
- `musicgen-small` weights are CC-BY-NC-4.0 — cannot ship in a commercial build (`docs/ai-jobs/QUALIFICATION.md`).
- Content ledger `LGR-0001…0004` status `required`, not executed — third-party content clearance outstanding (`docs/content-rights/ledger.json`).
