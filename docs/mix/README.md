# docs/mix — W18 mixer, automation and advanced MIDI

Owner: lane P (`devin/void-lane-w18`). Covers TEST_MATRIX rows T69–T71.

## What exists

| Piece | Where | Kind |
|---|---|---|
| Routing graph: sends, aux buses, multi-out nodes, external I/O descriptors, VCA-style groups (control-only) | `crates/void-mix/src/graph.rs` | real model |
| Topological validation with explicit loop/feedback rejection (`MixError::Loop` names the cycle) | `crates/void-mix/src/graph.rs` | real |
| Channel-layout legality matrix (mono/stereo/quad/5.1/7.1; fold-downs require a declared downmix flag; widening non-mono is `Illegal`) | `crates/void-mix/src/layout.rs` | real |
| Per-node delay compensation: accumulated latency per path, per-edge alignment delays, impulse-alignment simulation | `crates/void-mix/src/latency.rs` | real model |
| Automation lanes: linear/stepped interpolation, off/read/touch/latch/write/trim pass state machine, move-with-region transforms | `packages/void-studio/src/automation/` | real model + commit path |
| Mixed-transaction sends (`sendAsOneTransaction`) + one-step `undoTransaction` rewind | `packages/void-studio/src/automation/controller.ts` | real wire path |
| A/B toggle producing real param ops, optional loudness gain-match | `packages/void-studio/src/automation/ab.ts` | real ops |
| MIDI event model (insert/move/resize/quantize on string-i64 ticks), step editor | `packages/void-studio/src/midi/events.ts`, `step.ts` | real model |
| Pure transforms (transpose/velocity-scale/shift/retrograde/humanize) with per-note applied audit | `packages/void-studio/src/midi/transforms.ts` | real |
| Articulation sets + keyswitch maps; per-note articulation survives edits; switch events derived for export | `packages/void-studio/src/midi/articulation.ts` | real model |
| MPE zone allocator (master ch + 15 members), `flattenToMidi1` with explicit `MpeLoss` entries | `packages/void-studio/src/midi/mpe.ts` | real model |
| MIDI 2.0 declared model (`midi2Descriptor`, `MIDI2_TRANSPORT = false`) | `packages/void-studio/src/midi/mpe.ts` | declared model only |

## Evidence

- **T69** — `cargo test --manifest-path tests/mix/Cargo.toml`: 14 tests —
  loop/feedback rejection (`Loop` error names the cycle, self-send
  rejected at insert), channel-layout matrix + typed `Layout` error,
  impulse fixtures (known 90-sample skew → compensated impulses from
  any source land on the same output sample; raw arrivals expose the
  skew), unmeasured external-I/O flagged in `unmeasured_nodes`.
- **T70** — `pnpm --filter void-studio vitest run src/automation`:
  17 tests — read/off reject `WriteNotArmed`, write/touch/latch/trim
  splice semantics, trim leaves base untouched, interpolation
  boundaries, move-with-region anchors + landing-zone overwrite,
  mixed mixer/arrangement ops under one transactionId + single
  `UndoOp` rewind via `FakeTransport`.
- **T71** — `pnpm --filter void-studio vitest run src/midi`:
  16 tests — per-event edit ops on string-i64 ticks, quantize
  ties-away + strength, step editor chord/advance, transform audits,
  articulation preservation through edits + keyswitch derivation,
  MPE channel allocation, per-note expression → member-channel events,
  `CHANNEL_EXHAUSTED` loss entries, `MIDI2_TRANSPORT === false`.

## Honest limits (see NEEDS.md)

- `crates/void-mix` is a **model/policy** crate: protocol major.1 has no
  send/route/automation-point/expression ops, so the routing graph and
  lane curves have no wire persistence yet. `sendAsOneTransaction`
  composes *existing* ops under one transaction id — that part is real
  wire. Curve persistence is `docs/mix/NEEDS.md`.
- Compensation covers declared/measured node latencies only; bridge or
  device-path latency enters only where `latency_measured` is set.
- `flattenToMidi1` models the MIDI 1.0 event surface — no SMF encoder
  is claimed (needs an export file format op first).
