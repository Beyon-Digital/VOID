// Value math for learn mappings (MIX-07).
//
// Real curves, honest ranges: input normalized 0..1 → invert → curve →
// linear interpolate into [min,max]. Out-of-range input follows the
// mapping's policy (clamp or reject) — never silently wrapped.

import type { Macro, Mapping, ValueCurve } from './types';

export class MappingError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'MappingError';
  }
}

/** Validate a mapping spec — called at bind time, never on dispatch. */
export function checkMapping(m: Mapping): Mapping {
  if (!m.mappingId) throw new MappingError('mappingId required');
  if (!m.target.id) throw new MappingError('target id required');
  if (
    !Number.isFinite(m.min) ||
    !Number.isFinite(m.max) ||
    m.min > m.max
  ) {
    throw new MappingError('mapping requires finite min<=max');
  }
  return m;
}

/** Curve shaping on normalized input x in [0,1]. */
export function applyCurve(x: number, curve: ValueCurve): number {
  const v = Math.min(1, Math.max(0, x));
  switch (curve) {
    case 'linear':
      return v;
    case 'exponential':
      // Audio-style taper: slow at the bottom, fast at the top.
      return v * v;
    case 'logarithmic':
      // Fast rise then settle — companion taper to exponential.
      return Math.sqrt(v);
    case 'toggle':
      return v >= 0.5 ? 1 : 0;
  }
}

/**
 * Map one normalized input through a mapping to a parameter value.
 * Returns null when the mapping's out-of-range policy rejects the input.
 */
export function mapValue(m: Mapping, raw: number): number | null {
  checkMapping(m);
  if (!Number.isFinite(raw)) return null;
  let v = raw;
  if (v < 0 || v > 1) {
    if (m.outOfRange === 'reject') return null;
    v = Math.min(1, Math.max(0, v));
  }
  if (m.invert) v = 1 - v;
  const shaped = applyCurve(v, m.curve);
  return m.min + (m.max - m.min) * shaped;
}

/**
 * Fan a normalized macro input out to per-member values, preserving
 * each member's own curve/range. Deterministic order = member order.
 */
export function mapMacroValue(
  macro: Macro,
  raw: number,
): { paramId: string; value: number }[] {
  const v = Number.isFinite(raw) ? Math.min(1, Math.max(0, raw)) : 0;
  return macro.members.map((member) => ({
    paramId: member.paramId,
    value:
      member.min +
      (member.max - member.min) * applyCurve(v, member.curve),
  }));
}
