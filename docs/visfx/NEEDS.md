# visfx NEEDS — engine/platform gaps recorded by lane T (W23)

Format follows `docs/void-handoff/NEEDS.md`.

## NEEDS W23-01 — wgpu render surface (engine/GUI)

**Blocked item:** actual GPU execution of generated/builtin shaders —
per-frame render, tex allocation, real watchdog kill of a runaway
draw, preview/program pixels.
**What exists instead:** `ShaderRegistry` validates + budgets WGSL via
`naga` (parse + `Validator` with `Capabilities::empty()` + static
cost); `WatchdogPolicy::reservations()` is a `void-jobs` budget model
(cpu_seconds/memory_bytes/deadline enforced by runner rlimits;
`vram_bytes` honestly "0" — rlimits cannot cap GPU). Kernel outcomes
are recorded (`KernelOutcome`) and quarantine presets, but there is no
live kernel to time out on this box.
**Needed:** engine-side wgpu device/surface (native/void-engine or the
preview pane), a frame-driver calling `ShaderRegistry::resolve()` per
scene layer, and a real deadline/cancel wire from `WatchdogPolicy` into
the render thread.

## NEEDS W23-02 — camera device + landmark tracker (privacy surface)

**Blocked item:** real camera capture and the tracker model
(MediaPipe-class hand/pose landmarks), calibration UX, live frame
rate.
**What exists instead:** `ConductorSession` + `CameraPolicy` model the
full contract the runtime must enforce — consent gate, calibration
record, clutch, confidence floors, smoothing window, landmark-only
frames (`TrackerFrame` carries confidence/gesture/coords — *no pixel
fields exist*, so retention of raw frames is impossible by
construction), occluded-frame drop, loss≥window `ReleaseAll`. Privacy
asserts are first-class policy fields (`CameraAsserts`) whose
violation is enumerated by `violations()`.
**Needed:** a tracker backend that emits `TrackerFrame`s (landmarks +
confidence only — never pixels) at ~sample_fps; device consent wiring
into `grant_consent`/`deny`; the UI binding that feeds `ControlEvent`s
to the control bus. Raw-pixel paths must stay absent when
`no_raw_retention` is set — the enum is the enforcement point.

## NEEDS W23-03 — live audio analysis feed

**Blocked item:** the real-time analyzer that produces `FeatureFrames`
from the engine's audio thread and feeds `ParamMapper` into shader
uniforms (`u.rms`, `u.peak`, `u.p0`…).
**What exists instead:** deterministic `analyze()` over fixture PCM +
the mapper chain with attack/release; the `void-visual` `GenU` uniform
already carries rms/peak fields.
**Needed:** engine hookup — the analyzer runs on the audio callback
output tap (read-only copy; never on the callback itself), mappers
tick per visual frame, values land in uniforms. T84's "never blocks
audio" guarantee depends on this staying off the audio thread.

## NEEDS W23-04 — preview/program output plumbing

**Blocked item:** routing accepted `SceneDoc`s onto the preview and
program channels (video out, export path).
**What exists instead:** `plan_accept` verifies the channel exists and
every declared artifact resolves in the asset store before the
transaction id is minted; layers carry `channel: preview|program` and
`media` layers carry the imported artifact names.
**Needed:** the channel/route registry on the coordinator side +
engine output wiring; `commit_accepted` then drives the documented
layers.

## NEEDS W23-05 — projectM interop (see ADR-0001)

OpenGL-only pipeline + licensing/interop unknowns — decision recorded
in `docs/visfx/ADR-0001-projectm.md`. No code landed.
