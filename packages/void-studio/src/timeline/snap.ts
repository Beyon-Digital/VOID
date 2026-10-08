// Snap grid — quantize tick targets to musical subdivisions.
//
// The grid is a fixed-signature subdivision keyed to the engine's
// tempo_map_revision (from ClockSnapshot). When the revision moves the
// caller re-derives the grid — the editor store carries the revision so
// stale grids are visible, not silently wrong. Rounding is "nearest, ties
// away from zero", the documented protocol rule.

import { parseI64 } from 'void-client';
import { TICKS_PER_QUARTER } from '../viewport';
import type { SnapDivision, SnapSettings } from '../workspaces/editorStore';

const DIVISION_TICKS: Record<SnapDivision, bigint> = {
  bar: 0n, // resolved via beatsPerBar
  beat: TICKS_PER_QUARTER,
  '1/2': TICKS_PER_QUARTER / 2n,
  '1/4': TICKS_PER_QUARTER / 4n,
  '1/8': TICKS_PER_QUARTER / 8n,
  '1/16': TICKS_PER_QUARTER / 16n,
  '1/64': TICKS_PER_QUARTER / 64n,
};

/** Grid step in ticks for a snap setting (min 1 tick). */
export function gridStepTicks(s: SnapSettings): bigint {
  const step =
    s.division === 'bar'
      ? TICKS_PER_QUARTER * BigInt(Math.max(1, s.beatsPerBar))
      : DIVISION_TICKS[s.division];
  return step > 0n ? step : 1n;
}

/**
 * Snap a tick position to the nearest grid multiple. Disabled snap is a
 * pass-through. Ties (exactly half a step away) round away from zero.
 */
export function snapTicks(ticks: string, s: SnapSettings): string {
  if (!s.enabled) return ticks;
  const t = parseI64(ticks);
  const step = gridStepTicks(s);
  const q = t / step;
  const r = t % step;
  // |2r| >= step → one more step away from zero (covers ties).
  const bump = (r < 0n ? -r : r) * 2n >= step ? (r < 0n ? -1n : 1n) : 0n;
  return ((q + bump) * step).toString(10);
}

/**
 * Snap a drag target: applies snapTicks to `targetTicks`. Kept separate
 * so relative-delta gestures (move) can choose between target-snap and
 * delta-snap semantics.
 */
export function snapDelta(fromTicks: string, targetTicks: string, s: SnapSettings): string {
  if (!s.enabled) return parseI64(targetTicks).toString(10);
  const snapped = snapTicks(targetTicks, s);
  return (parseI64(snapped) - parseI64(fromTicks)).toString(10);
}
