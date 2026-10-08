// Move-with-region automation semantics (W18; MIX-06, T70).
//
// When an arrangement region moves, its automation segments move with
// it and the curve *splits* at the region boundaries. The transform is
// documented and total:
//
// 1. Points inside [start, end) shift by `deltaTicks` (bigint-exact).
// 2. Boundary anchors are inserted at (start+delta) and (end+delta)
//    carrying the pre-move edge values — the moved block's edges keep
//    their original values instead of ramping across the landing gap.
// 3. The move is cut+paste-overwrite: outside points inside the
//    landing zone are removed; the vacated range is left empty and the
//    curve interpolates across it.
// 4. The trim layer gets the same transform — moving a region moves
//    its relative offsets too.
//
// Uses the shared `Region` shape ({startTicks, lengthTicks}) from takes.

import { i64str, parseI64 } from 'void-client';
import type { Region } from '../takes/takes';
import type { AutomationLane, AutomationPoint } from './types';
import { cmpTicks, evalCurve } from './lane';

function movePoints(
  points: AutomationPoint[],
  region: Region,
  deltaTicks: string,
  interp: 'linear' | 'stepped',
): AutomationPoint[] {
  const start = parseI64(region.startTicks);
  const end = start + parseI64(region.lengthTicks);
  const delta = parseI64(deltaTicks);
  const landStart = start + delta;
  const landEnd = end + delta;

  const inside = points.filter((p) => {
    const t = parseI64(p.ticks);
    return t >= start && t < end;
  });
  // outside points in the landing zone are overwritten by the block.
  const outside = points.filter((p) => {
    const t = parseI64(p.ticks);
    if (t >= start && t < end) return false; // inside — moves
    return t < landStart || t >= landEnd;
  });

  const moved: AutomationPoint[] = inside.map((p) => ({
    ticks: i64str(parseI64(p.ticks) + delta),
    value: p.value,
  }));

  // Boundary anchors carry the pre-move edge values.
  const vStart = evalCurve(points, i64str(start), interp);
  const vEnd = evalCurve(points, i64str(end), interp);
  const anchors: AutomationPoint[] = [];
  if (vStart !== null) anchors.push({ ticks: i64str(landStart), value: vStart });
  if (vEnd !== null) anchors.push({ ticks: i64str(landEnd), value: vEnd });

  const out = [...outside, ...moved, ...anchors];
  out.sort((a, b) => cmpTicks(a.ticks, b.ticks));
  // dedup same-ticks keeping the later assembly entry (block/anchors win)
  const dedup = new Map<string, AutomationPoint>();
  for (const p of out) dedup.set(p.ticks, p);
  return [...dedup.values()].sort((a, b) => cmpTicks(a.ticks, b.ticks));
}

/** Move a region's automation segments by `deltaTicks` (may be negative). */
export function moveRegionAutomation(
  lane: AutomationLane,
  region: Region,
  deltaTicks: string,
): AutomationLane {
  return {
    ...lane,
    base: movePoints(lane.base, region, deltaTicks, lane.interpolation),
    trim: movePoints(lane.trim, region, deltaTicks, lane.interpolation),
  };
}

/**
 * Split the curves at `atTicks` — inserts an evaluated point at the
 * boundary so each side of a later move/delete keeps its exact edge
 * value (stepped and linear both get an explicit breakpoint).
 */
export function splitAutomationAt(lane: AutomationLane, atTicks: string): AutomationLane {
  const split = (pts: AutomationPoint[]): AutomationPoint[] => {
    const v = evalCurve(pts, atTicks, lane.interpolation);
    if (v === null) return pts;
    if (pts.some((p) => p.ticks === i64str(atTicks))) return pts;
    return [...pts, { ticks: i64str(atTicks), value: v }].sort((a, b) =>
      cmpTicks(a.ticks, b.ticks),
    );
  };
  return { ...lane, base: split(lane.base), trim: split(lane.trim) };
}

/** Remove automation inside the region (curve bridges the hole). */
export function removeRegionAutomation(
  lane: AutomationLane,
  region: Region,
): AutomationLane {
  const drop = (pts: AutomationPoint[]): AutomationPoint[] => {
    const s = parseI64(region.startTicks);
    const e = s + parseI64(region.lengthTicks);
    return pts.filter((p) => {
      const t = parseI64(p.ticks);
      return t < s || t >= e;
    });
  };
  return { ...lane, base: drop(lane.base), trim: drop(lane.trim) };
}
