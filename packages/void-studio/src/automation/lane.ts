// Automation curve evaluation + point splicing (W18, T70).
//
// All tick math is bigint via i64str/parseI64 — the lane model is
// exact at any int64 offset, which is what the string-i64 DTO rule
// (CONTRACTS §1) buys us.

import { i64str, parseI64 } from 'void-client';
import type { AutomationLane, AutomationPoint, Interpolation } from './types';

/** Ascending ticks compare (exact). */
export function cmpTicks(a: string, b: string): number {
  const x = parseI64(a);
  const y = parseI64(b);
  return x < y ? -1 : x > y ? 1 : 0;
}

function checkPoint(p: AutomationPoint): void {
  parseI64(p.ticks); // throws on non-canonical
  if (!Number.isFinite(p.value)) {
    throw new Error(`automation point value must be finite, got ${p.value}`);
  }
}

/**
 * Evaluate a sorted curve at `t`.
 *
 * Documented interpolation:
 * - `linear`: straight-line segment between neighbours; before the
 *   first point and after the last the curve holds the edge value
 *   (flat extension — a point at exactly t returns its value).
 * - `stepped`: the value of the latest point at or before t; before the
 *   first point the curve holds the first point's value.
 *
 * An empty curve has no coverage → returns `null` (the parameter's
 * manual value applies; callers must not read this as 0).
 */
export function evalCurve(
  points: AutomationPoint[],
  t: string,
  interp: Interpolation,
): number | null {
  if (points.length === 0) return null;
  const tv = parseI64(t);
  const first = points[0];
  const last = points[points.length - 1];
  if (tv <= parseI64(first.ticks)) return first.value;
  if (tv >= parseI64(last.ticks)) return last.value;
  // find the segment [a, b) containing t
  let lo = 0;
  let hi = points.length - 1;
  while (hi - lo > 1) {
    const mid = (lo + hi) >> 1;
    if (parseI64(points[mid].ticks) <= tv) lo = mid;
    else hi = mid;
  }
  const a = points[lo];
  const b = points[hi];
  if (interp === 'stepped') return a.value;
  const span = Number(parseI64(b.ticks) - parseI64(a.ticks));
  const pos = Number(tv - parseI64(a.ticks)) / span;
  return a.value + (b.value - a.value) * pos;
}

/** Lane value at `t`: base + trim (trim empty/absent contributes 0). */
export function evalLane(lane: AutomationLane, t: string): number | null {
  const b = evalCurve(lane.base, t, lane.interpolation);
  if (b === null) return null;
  const tr = evalCurve(lane.trim, t, lane.interpolation) ?? 0;
  return b + tr;
}

/** Insert/replace a point (same-ticks point is overwritten). */
export function insertPoint(
  points: AutomationPoint[],
  p: AutomationPoint,
): AutomationPoint[] {
  checkPoint(p);
  const out = points.filter((x) => x.ticks !== p.ticks);
  const i = out.findIndex((x) => cmpTicks(x.ticks, p.ticks) > 0);
  if (i === -1) out.push(p);
  else out.splice(i, 0, p);
  return out;
}

/**
 * Replace the half-open range [startTicks, endTicks): points inside are
 * removed and `written` (already inside the range) is spliced in
 * sorted. Points outside are untouched. Returns a new array.
 */
export function spliceRange(
  points: AutomationPoint[],
  startTicks: string,
  endTicks: string,
  written: AutomationPoint[],
): AutomationPoint[] {
  const s = parseI64(startTicks);
  const e = parseI64(endTicks);
  if (e <= s) throw new Error(`spliceRange: end ${endTicks} <= start ${startTicks}`);
  for (const p of written) checkPoint(p);
  const kept = points.filter((p) => {
    const t = parseI64(p.ticks);
    return t < s || t >= e;
  });
  const merged = [...kept, ...written];
  merged.sort((a, b) => cmpTicks(a.ticks, b.ticks));
  return merged;
}

/** Points inside the half-open region [start, start+length). */
export function pointsInRange(
  points: AutomationPoint[],
  startTicks: string,
  lengthTicks: string,
): AutomationPoint[] {
  const s = parseI64(startTicks);
  const e = s + parseI64(lengthTicks);
  return points.filter((p) => {
    const t = parseI64(p.ticks);
    return t >= s && t < e;
  });
}

/** Shift every point by `deltaTicks` (bigint, sign allowed). */
export function shiftPoints(
  points: AutomationPoint[],
  deltaTicks: string,
): AutomationPoint[] {
  const d = parseI64(deltaTicks);
  return points.map((p) => ({
    ticks: i64str(parseI64(p.ticks) + d),
    value: p.value,
  }));
}
