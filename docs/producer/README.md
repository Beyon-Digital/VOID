# W21 — Producer gate (Lane S), non-native parts

Lane `S` / work package `W21` / branch `devin/void-lane-w21`.
TEST_MATRIX coverage: T78, T79, T80 — the Linux-verifiable halves.

## What landed

| Piece | Path | What it is |
|---|---|---|
| Accompaniment engine | `crates/void-producer/src/{engine,music,rng}.rs` | Role-scoped generators (drums/bass/keys/synth) producing real note events from scale + piecewise chord map + groove template + seed. Deterministic `XorShift128` — same spec ⇒ same bytes. Modes: `accompaniment`, `inpaint` (bounded gap, edge-conditioned), `continue` (continuation window), `vary` (re-voiced take within `variation_ppm`). |
| Proposal glue | `crates/void-producer/src/service.rs` | `request_accompaniment` mints a `void_proposals::ProposalRecord` (status `ready`, candidates ranked by measured chord-tone fit) persisted through `ProposalStore` — the existing accept/reject/stale/`plan_accept` lifecycle applies verbatim. `verify_locked_unchanged` = T78 byte-identity postcondition. |
| BS.1770 meter | `crates/void-producer/src/{pcm,loudness}.rs` | Real measurement in pure Rust: K-weighting biquad chain (shelf + RLB high-pass, coefficients recomputed per sample rate), 400 ms dual-gated integrated LUFS, 3 s LRA (10th–95th percentile with −20 LU rel gate), 4× polyphase true peak. WAV reader for PCM 16/24/32-int and 32-float, mono/stereo. |
| Mastering proposals | `crates/void-producer/src/mastering.rs` | `analyze()`/`analyze_wav()` → `MasteringProposal` record: measured report + gain/limiter/EQ `MasteringOp`s derived from measured deltas + level-matched A–B `AuditionSpec`. Lifecycle mirrors void-proposals (pending→ready→accepted\|rejected\|stale\|failed, terminal final, revalidation mints a NEW record via `supersedes`). `MasteringStore` JSON persistence, `sweep_stale`. |
| Stem batch | `crates/void-producer/src/stems.rs` | `plan_stem_batch` → one validated `void_export::ExportSpec` per track/bus sharing one frame plan; deterministic uuid-v5 job ids; manifest writer. |
| Consolidation | `crates/void-producer/src/consolidate.rs` | `plan_consolidation`/`execute_consolidation` over `AssetStore` — external refs → content-addressed assets with `Collected/AlreadyContained/Missing/Drifted` statuses and a protected-sha list fed by alternatives. |
| Alternatives | `crates/void-producer/src/alternatives.rs` | `AlternativeLedger`: create → seal (one-way) → branch → switch with audit history; `protected_asset_set()` is the do-not-evict union consolidation consumes; sealed/active/parented deletes rejected. |
| Import planner | `crates/void-producer/src/import.rs` | `ForeignProject` → `ImportPlan`: deterministic uuid-v5 id remap (collision-proof vs existing ids), ordered `PlannedOp`s (buses→tracks→clips→sends), `unresolved` sends reported (never dangling), `losses` list (e.g. video tracks). |
| Studio: producer | `packages/void-studio/src/producer/` | `AccompanimentSpec` DTO + `validateSpec`, `parseMasteringProposal` (defensive), op builders (`request_accompaniment`, `accept/reject/mastering_audition`), view-state `ProducerStore` (draft, records, audition side). |
| Studio: shortcuts | `packages/void-studio/src/shortcuts/` | Canonical binding parser/formatter, keymap store: immutable `DEFAULT_MAP`, conflict detect + resolve (loser unbound, never shared), versioned JSON export/import, `resetToDefaults`. |
| Studio: screensets | `packages/void-studio/src/screensets/` | `Screenset` presets (Arrange/Edit/Mix builtins, complete panel sets), switch = deep-copy into live, `setLocked` gates live mutation, `saveAs`/`updateActive`, `fitToViewport` letterbox math, versioned export/import. |
| Suite | `tests/producer/` | Detached `[workspace]` suite: `t78.rs` (determinism, scale/chord membership, locked byte-identity through accept, inpaint bounds, vary), `t79.rs` (fixture LUFS ±0.5, +3 LU stereo, energy doubling, explicit accept/stale/supersedes, WAV roundtrip), `t80.rs` (stem batch + manifest, consolidation + alternative protection, alternative rules, import remap/loss/unresolved). |

## Honest gaps (engine/GUI/audio needed) — see NEEDS.md
Everything below is model/spec layer verified on Linux; the native half is recorded as a gap, not faked.
