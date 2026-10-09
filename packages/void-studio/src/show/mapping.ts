// Modular mapping engine (T96) — "Environment-style" bounded
// transform chains: input event → filter/scale/route → target param.
//
// Pure value model: `runChain` takes an event and returns concrete
// ChainChange intents or a typed drop-reason. Chains are BOUNDED —
// at most MAX_CHAIN_STEPS steps, every range finite — so a runaway
// transform is structurally impossible, not checked-after-the-fact.
// Revoking a source pairing closes every target on every chain bound
// to it ("no access after revocation" — T96 is observable, not a flag).

import type { ValueCurve } from '../learn/types';

/** An input event from any modular source (fader, OSC control,
 *  MIDI CC — the chain doesn't care where it came from). */
export interface MappingEvent {
  sourceId: string;
  /** Normalized 0..1 input. */
  value: number;
  atMs: number;
}

/** One bounded step in a transform chain. Order in the chain is the
 *  pipeline order; the engine runs them left-to-right. */
export type TransformStep =
  /** Drop events whose value is outside [min,max] — a gate, not a clamp. */
  | { kind: 'filter'; min: number; max: number }
  /** Clamp the value into [min,max]. */
  | { kind: 'clamp'; min: number; max: number }
  /** Curve-shape then remap to [outMin,outMax]. */
  | { kind: 'scale'; curve: ValueCurve; invert: boolean; outMin: number; outMax: number }
  /** Stepped quantize to `steps` divisions (>=1). */
  | { kind: 'quantize'; steps: number }
  /** Split to a secondary target chain (route fan-out): value passes
   *  through unmodified but the route may be enabled/disabled. */
  | { kind: 'route'; enabled: boolean };

/** Hard bound on chain length — a chain is a bounded pipeline, not
 *  a turing tarpit. */
export const MAX_CHAIN_STEPS = 8;

/** A target the chain writes — a parameter descriptor. */
export interface ChainTarget {
  targetId: string;
  /** e.g. 'param', 'dmx-channel', 'osc-address' — the coordinator
   *  resolves the concrete sink; we only describe it. */
  kind: 'param' | 'dmx-channel' | 'osc-address';
}

/** A declared mapping chain. */
export interface MappingChain {
  chainId: string;
  /** The source pairing this chain listens to. Revoking the source
   *  pairing must close every target on this chain. */
  sourceId: string;
  label: string;
  steps: TransformStep[];
  targets: ChainTarget[];
  enabled: boolean;
}

/** A concrete value destined for a target — produced by a chain run. */
export interface ChainChange {
  targetId: string;
  kind: ChainTarget['kind'];
  value: number;
}

/** Why a chain run produced no changes — typed, for UI diagnostics. */
export type ChainDrop =
  | 'disabled'
  | 'filtered'
  | 'route-off'
  | 'non-finite'
  | 'no-targets';

export interface ChainResult {
  ok: boolean;
  changes: ChainChange[];
  drop?: ChainDrop;
  /** Steps actually traversed — for chain visualization. */
  stepsRun: number;
}

/** Validate a chain's structure (bounds before it may arm). */
export function validateChain(chain: MappingChain): string[] {
  const errors: string[] = [];
  if (!chain.chainId) errors.push('chainId empty');
  if (!chain.sourceId) errors.push('sourceId empty');
  if (chain.steps.length === 0) errors.push('chain has no steps');
  if (chain.steps.length > MAX_CHAIN_STEPS)
    errors.push(`chain has ${chain.steps.length} steps > max ${MAX_CHAIN_STEPS}`);
  chain.steps.forEach((s, i) => {
    const num = (v: number, name: string) => {
      if (!Number.isFinite(v)) errors.push(`step ${i} ${name} not finite`);
    };
    if (s.kind === 'filter' || s.kind === 'clamp') {
      num(s.min, 'min');
      num(s.max, 'max');
      if (Number.isFinite(s.min) && Number.isFinite(s.max) && s.min > s.max)
        errors.push(`step ${i} min > max`);
    }
    if (s.kind === 'scale') {
      num(s.outMin, 'outMin');
      num(s.outMax, 'outMax');
      if (Number.isFinite(s.outMin) && Number.isFinite(s.outMax) && s.outMin > s.outMax)
        errors.push(`step ${i} outMin > outMax`);
    }
    if (s.kind === 'quantize' && (!Number.isInteger(s.steps) || s.steps < 1 || s.steps > 1024))
      errors.push(`step ${i} quantize steps out of 1..1024`);
  });
  if (chain.targets.length === 0) errors.push('chain has no targets');
  chain.targets.forEach((t, i) => {
    if (!t.targetId) errors.push(`target ${i} id empty`);
  });
  return errors;
}

function curveValue(x: number, curve: ValueCurve): number {
  switch (curve) {
    case 'linear':
      return x;
    case 'exponential':
      return x <= 0 ? 0 : x * x;
    case 'logarithmic':
      return Math.sqrt(Math.max(0, x));
    case 'toggle':
      return x >= 0.5 ? 1 : 0;
  }
}

/** Run an event through a chain — bounded, deterministic, honest
 *  about drops. Returns changes for EVERY target on the chain
 *  (route-enabled), or a typed drop. */
export function runChain(chain: MappingChain, event: MappingEvent): ChainResult {
  if (!chain.enabled || event.sourceId !== chain.sourceId)
    return { ok: false, changes: [], drop: 'disabled', stepsRun: 0 };
  if (chain.targets.length === 0)
    return { ok: false, changes: [], drop: 'no-targets', stepsRun: 0 };

  let v = event.value;
  if (!Number.isFinite(v)) return { ok: false, changes: [], drop: 'non-finite', stepsRun: 0 };
  let stepsRun = 0;
  for (const s of chain.steps.slice(0, MAX_CHAIN_STEPS)) {
    stepsRun++;
    switch (s.kind) {
      case 'filter':
        if (v < s.min || v > s.max)
          return { ok: false, changes: [], drop: 'filtered', stepsRun };
        break;
      case 'clamp':
        v = Math.min(s.max, Math.max(s.min, v));
        break;
      case 'scale': {
        const x = s.invert ? 1 - v : v;
        v = s.outMin + (s.outMax - s.outMin) * curveValue(Math.min(1, Math.max(0, x)), s.curve);
        break;
      }
      case 'quantize':
        v = Math.round(v * s.steps) / s.steps;
        break;
      case 'route':
        if (!s.enabled) return { ok: false, changes: [], drop: 'route-off', stepsRun };
        break;
    }
    if (!Number.isFinite(v))
      return { ok: false, changes: [], drop: 'non-finite', stepsRun };
  }
  return {
    ok: true,
    changes: chain.targets.map((t) => ({ targetId: t.targetId, kind: t.kind, value: v })),
    stepsRun,
  };
}

/** Close intents when a source pairing is revoked: every chain bound
 *  to `sourceId` emits a `closed` record for each of its targets —
 *  "no access after revocation" is a produced action, not a hope. */
export interface ClosedTarget {
  chainId: string;
  targetId: string;
  kind: ChainTarget['kind'];
  /** Always 'closed' — the post-revocation state. */
  state: 'closed';
}

export function closeChainsForSource(
  chains: MappingChain[],
  sourceId: string,
): ClosedTarget[] {
  const out: ClosedTarget[] = [];
  for (const c of chains) {
    if (c.sourceId !== sourceId) continue;
    for (const t of c.targets) {
      out.push({ chainId: c.chainId, targetId: t.targetId, kind: t.kind, state: 'closed' });
    }
  }
  return out;
}
