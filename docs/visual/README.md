# VOID visuals (W22)

Native visual composition engine — a **real wgpu renderer** that runs as
a separate output path alongside audio. Audio never waits on the GPU:
the visual engine owns a dedicated thread, consumes ops on a bounded
channel and clock snapshots on a latest-wins slot, and drops/degrades
visual work under load rather than delaying a single audio tick.

## Components

| Path | Contents |
|---|---|
| `protocol/visual/void_visual.fbs` | Wire contract — **draft rev0** (`voidvis` namespace). Layer stack/media/transform/blend/transition/anchor/output-route/checkpoint ops, `VisualPersistentCommand` envelope (command_id + transaction_id + project_id + engine_epoch + expected_revision), `VisualReceipt` ack + `VisualTelemetryEnvelope` split — same discipline as `protocol/void_control.fbs`. |
| `crates/void-visual/` | Engine: `scene` (ops → state + inverse journal), `tempo` (tick↔sample map), `clock` (ClockSnapshot acceptance), `media` (PNG/JPEG decode + bounded ffmpeg pull), `render` (wgpu compositor), `runtime` (own thread, drop counters), `checkpoint` (canonical-JSON snapshot join). |
| `tests/visual/` | Detached T81/T82/T83 suite (`cargo test --manifest-path tests/visual/Cargo.toml`). |
| `packages/void-studio/src/visuals/` | UI view-state: layer/anchor/transition/output projections, op payload builders, zustand store (no pixel buffers, no document state). |

## Runtime guarantees (measured, not asserted)

- **Audio never waits.** `push_clock`/`send_command` are non-blocking;
  unconsumed clocks are overwritten and counted (`dropped_clocks`),
  displaced produced frames are counted (`dropped_frames`), renders
  skipped while a newer clock waits are counted (`skipped_renders`).
- **Program outranks preview.** The engine renders the program channel
  first; under pressure preview starves first, never program.
- **Beat-anchored.** All timing derives from `ClockSnapshot` ticks via
  the tempo map (960_000 ticks/quarter, signed int64, ties round away
  from zero). Wall time is never a timing input; generator presets take
  `tick`/`beat_phase`/audio features as uniforms — deterministic.
- **Bounded resources.** Op queue 128 → `Busy`; media verified by sha256
  before decode; image dims ≤ 8192, video pull ring = 4 frames, max 4
  concurrent video layers; generator `param_json` ≤ 16 KiB.
- **No fake rendering.** Layer shader composites real RGBA textures with
  real blend math (normal premultiplied src-over, add, multiply, screen);
  transitions merge two stack composites (cut/fade/wipe); generators are
  WGSL preset bodies (`black`, `color-bars`, `checker`, `gradient`,
  `plasma`, `pulse`).

## Offscreen evidence path (headless-verifiable)

`OutputRoute { target: OFFSCREEN, readback: true }` renders the channel
to an `Rgba8Unorm` target, maps it to CPU pixels and returns
`frame_sha256` — the determinism proof (`same inputs → same pixels`) on
any adapter, including software fallbacks.

**Adapter used in this lane's CI/local run:** `Gl` backend —
`llvmpipe (LLVM 15.0.7, 256 bits)` (Mesa 23.2.1, Ubuntu 22.04). No
Vulkan ICD was present; wgpu's fallback adapter path was exercised
(`force_fallback_adapter` first, normal discovery second). A `window`
feature (winit surface) exists behind `--features window` for hosts
with a display; offscreen is the always-on path.

## Checkpoint join

`Scene::checkpoint()` serializes to canonical JSON
(`void-visual-checkpoint` v0) → `payload_sha256` pins it on the wire.
`SnapshotVisualStateOp` publishes into the project checkpoint's `visual`
section; `RestoreVisualStateOp` verifies `state_sha256` before applying
— a tampered or truncated payload is REJECTED, never half-restored.
Transitions, routes, anchors and the visual revision all round-trip.

## Undo semantics

`transaction_id` on every `VisualCommand` groups one user gesture.
`VisualUndoOp` rewinds **every** op committed under that transaction
(newest first); `VisualRedoOp` replays it oldest-first and re-journals
so it can be undone again. Undo/redo against an unknown transaction is
REJECTED (`NotFound`) — a partial transaction is compensated or
reported, never labeled an atomic success.

## Known limits (honest list)

- Video pull index maps ticks→seconds through the tempo map
  (`tempo_at(tick).bpm`); tempo *changes* inside a playing clip are
  approximated per-frame, not integrated. Documented for rev1.
- `window` feature compiles but the winit presentation loop is not yet
  wired end-to-end (no display in this environment to verify against);
  offscreen is the exercised path.
- Multi-display selection is a `display_id` field on `OutputRoute`;
  on this host only the offscreen target is real — display enumeration
  is deferred to the window path.
- `frame_delay_us` is a test/diagnostic hook only (drop-under-load
  proof); production paths never set it.
