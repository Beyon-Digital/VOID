# ADR-0001 — projectM for generated/reactive visuals

Status: **proposed — no code landed** (W23 scope says ADR only).
Date: 2026-10-08. Lane: T (W23).

## Context

W23 asks for "generated/reactive visuals and optional camera
conducting". The Work Package names projectM as a candidate
integration for music-reactive visuals. Two competing realities:

1. **Our visuals path is WGSL-on-wgpu.** `crates/void-visual` (lane O)
   established the generator contract (`GenU` uniforms + `gen(uv,u)`
   body), and lane T's `ShaderRegistry` validates/budgets WGSL bodies
   through `naga`. This is a GPU-native, deterministic, embeddable
   pipeline with full provenance.

2. **projectM is an OpenGL pipeline.** projectM (LGPL-2.1 for
   libprojectM; preset shaders are Milkdrop `.milk` HLSL-ish text
   interpreted by the library, not compiled to WGSL). It renders into
   its own GL context and has its own audio-analysis front-end
   (FFT → wave/spectrum texture inputs to preset "equations"). It is
   not a shader library we can feed through `naga`; it is an engine.

## Unknowns that must resolve before any integration

- **License/rights.** libprojectM is LGPL-2.1: dynamic linking is
  permitted, but bundling with a proprietary/native static engine
  needs legal review. The preset corpus ("Milkdrop presets") has murky
  redistribution status — most packs carry no license at all, which is
  a show-stopper for shipping them by default.
- **Interop cost.** projectM needs a GL context; VOID's render path is
  wgpu. Options: (a) render projectM to an offscreen GL framebuffer and
  blit the texture into the wgpu pipeline — adds a second GPU stack,
  context ping-pong, and platform-specific GL bootstrap (ANGLE/EGL/
  wgl) per OS; (b) vendor projectM's *analysis* only (FFT features) and
  re-implement presets as WGSL — real engineering, loses the preset
  corpus anyway; (c) run projectM as a *separate* output surface, never
  mixed with generated layers — simplest, weakest product value.
- **Feature parity.** Our mappers already emit normalized rms/onset/
  band-energy; projectM's per-frame feature extraction duplicates this
  at GL-texture granularity. Nothing in the projectM feature set is
  unimplementable in our pipeline — its value is the preset corpus and
  the editor ecosystem, both gated on the rights question.
- **Sandboxing.** projectM presets are interpreted math with no static
  cost bound — our T84 guarantee (watchdog + instruction cap +
  known-good fallback) has no equivalent in projectM's equation
  evaluator; a hostile/expensive preset could run unbounded CPU.

## Decision

Defer integration; keep the WGSL pipeline as the only visuals engine.
When the native side lands (NEEDS W23-01), re-evaluate with:

1. a legal review of LGPL-2.1 linking + preset-corpus rights;
2. a prototype of option (a) — offscreen GL → texture import — to
   measure the real interop cost on our three target platforms;
3. if rights are clean and cost acceptable, wrap projectM behind the
   same `PresetRef::Generated`/`Builtin` surface so fallback,
   provenance, and budget descriptors still apply (projectM itself
   would carry `ResourceBudget{allow_unbounded_loops:true}` + its own
   watchdog wall, since static cost analysis is impossible there).

Until then: **no projectM code, no preset assets, no GL dependency** —
the honest engine-gap entry lives in `NEEDS.md` W23-05.
