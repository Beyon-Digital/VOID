# W23 — Generated/reactive visuals + camera conducting (lane T)

Linux-verifiable, non-native half of W23. The native/engine side (wgpu
render, camera device access, preview-window pixels) is recorded as
NEEDS in `docs/visfx/NEEDS.md`.

## What exists

- **`crates/void-visfx`** — the policy/spec/op layer:
  - `spec.rs` — `SceneGenSpec` (declarative scene-generation input: kind,
    duration, seed, resolution, fps, params, audio bindings, shader
    body, description) with forbidden-field/action rejection and a
    sha256 identity. `SceneDoc` (validated `scene.json` the worker
    emits: tagged, generator-id'd, typed layers/actions, media
    artifacts declared by name).
  - `service.rs` — `VisualGenService` mirrors void-proposals:
    `request()` → `JobSpec{kind:VisualGeneration}` on `void-jobs`
    (params are job inputs; worker artifacts land as normal sha256
    assets via `RunOutcome::Succeeded.artifacts`); `collect()` parses
    `scene.json`, verifies declared media artifacts, runs every
    generator shader body through the registry's compile pipeline, then
    marks the record `Ready` with `VisGenProvenance`
    (job/generator/model/runtime/seed/document-sha). `plan_accept` →
    `commit_accepted` → `reject`/`mark_stale` lifecycle, single
    transaction id.
  - `shader.rs` — WGSL preset registry: `ShaderRegistry` (builtin
    presets mirrored from `void-visual` + generated presets),
    `ResourceBudget` (tex size, loop iterations, instruction cap,
    source bytes, unbounded-loop policy), `WatchdogPolicy` + `reservations()`
    (JobBudget model — enforced by the void-jobs runner rlimits),
    `FallbackPolicy` → known-good builtin (`black`) with a full
    `CompileReport.chain` evidence trail, `CompileCache` (bounded,
    keyed by composed shader sha256), kernel-outcome records
    (ok/timeout/cancelled/failed → quarantine).
  - `analysis.rs` — `module_cost()` static analysis over naga IR:
    instruction count, per-pixel texture-sample cost, loop trip-bound
    extraction (handles naga's `for`→`loop{if(!cond)break}` desugar),
    call graph with depth cap.
  - `mappers.rs` — `AnalyzerSet` (frame_size/hop/sample_rate/features)
    → `FeatureFrames` (rms/peak/onset/band-energy, all normalized
    0..1, deterministic on fixture PCM); `ParamMapper` (linear/sqrt/
    dB-floor shaping + gain + attack/release smoothing with state).
  - `camera.rs` — `CameraPolicy` (asserts: no raw retention, no network
    upload, bounded landmark history, occluded-frame drop, tracking-
    loss→release window) + `CameraAsserts` + `CalibrationRecord` +
    `ConductorSession` (consent gate → calibrate → armed → clutch +
    pinch → ControlOn/Move/Off; occluded/below-confidence frames drop;
    loss ≥ window → `ReleaseAll`). Landmark frames only — no pixel
    types exist by construction.
  - `record.rs`/`store.rs`/`error.rs` — `VisGenRecord` state machine
    (pending→ready/failed/stale/rejected→accepted), json persistence in
    `<container>/visfx/generations/<id>/record.json`, typed errors.

- **`packages/void-studio/src/visual-gen/`** — view-state-only zustand
  store: vis-gen records (request → telemetry → ready →
  accept/reject/stale), compare solo, camera consent/policy/session
  view. Mirrors the wire shapes; nothing is project truth.

- **`tests/visfx/`** — detached suite (own `[workspace]` + lockfile),
  protocol-v1 fake worker `visfx_fake_worker`. 31 tests.

- **`docs/visfx/ADR-0001-projectm.md`** — projectM decision record
  (no code; see NEEDS).

## Coverage vs TEST_MATRIX

- **T84** — naga accepts good shader; rejects malformed, forbidden
  builtins, over-budget (unbounded loop, >instruction cap); watchdog
  policy model + kernel outcomes; known-good fallback chain with
  evidence; compile cache; pipeline latency bound (never blocks).
  See `t84_generated_shader_limits.rs` (9 tests).
- **T85** — params→job→asset flow with fake worker (sha256 assets,
  provenance, accept transaction); media artifact verified into the
  store; forbidden spec fields rejected pre-submit; forbidden scene
  actions + rejected shader bodies fail the generation honestly even
  when the job succeeded; worker failure marks Failed; reject/stale
  lifecycle. `t85_visual_generation.rs` (8 tests) +
  `t85_audio_mappers.rs` (4 tests).
- **T86** — consent gate (deny/refused pre-grant/revoke), calibration
  required, clutch + pinch holds, occluded/below-confidence drops,
  tracking loss ≥ window → ReleaseAll, session end → ReleaseAll,
  privacy-assert violations enumerated, bounded landmark log.
  `t86_camera_safety.rs` (9-10 tests).
