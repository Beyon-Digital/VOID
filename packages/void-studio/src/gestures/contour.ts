// Melodic-contour capture (GEST-01): pointer/touch/trackpad path → raw
// note events.
//
// The path is divided into fixed cells of `cellTicks`; each cell takes
// the average pitch of the points inside it, and adjacent cells sharing
// a pitch merge into one note. Cell onsets are the RAW onsets — the
// transform chain may quantize them later but raw is never rewritten.
//
// Velocity is a capture parameter (pointer pressure is not assumed); a
// caller that has pressure can pass `velocityForCell` — otherwise the
// constant is honest and visible in the note data.

import { parseI64 } from 'void-client';
import type { TickViewport, Zoom } from '../viewport';
import { pxToTicks } from '../viewport';
import {
  clampGesturePitch,
  clampGestureVelocity,
  type GesturePoint,
  type RawGestureNote,
} from './types';

/** Maps surface coordinates into the musical domain. */
export interface ContourMapping {
  /** surface x → absolute tick (before any quantization). */
  xToTicks(xPx: number): string;
  /** surface y → MIDI pitch float (clamped to 0..127 by the capture). */
  yToPitch(yPx: number): number;
}

/** Wire the studio viewport into a mapping: x via pxToTicks, y linear
 * between pitchLow (bottom edge) and pitchHigh (top edge). */
export function makeContourMapping(
  viewport: TickViewport,
  zoom: Zoom,
  pitchLow: number,
  pitchHigh: number,
  heightPx: number,
): ContourMapping {
  const lo = Math.min(pitchLow, pitchHigh);
  const hi = Math.max(pitchLow, pitchHigh);
  const span = hi - lo;
  return {
    xToTicks: (xPx) => pxToTicks(xPx, viewport, zoom),
    yToPitch: (yPx) => {
      if (!Number.isFinite(yPx) || heightPx <= 0 || span <= 0) return lo;
      const frac = 1 - yPx / heightPx; // top edge = high pitch
      return lo + Math.min(1, Math.max(0, frac)) * span;
    },
  };
}

export interface ContourOptions {
  /** Cell width in ticks — the contour's capture resolution. */
  cellTicks: string;
  /** Default velocity when no per-cell function is given (1..127). */
  velocity?: number;
  /** Optional per-cell velocity (cell average pitch, cell onset). */
  velocityForCell?: (avgPitch: number, cellStartTicks: string) => number;
}

/** One contour cell after resampling — intermediate representation. */
interface Cell {
  startTicks: bigint;
  pitch: number;
  velocity: number;
}

function resample(points: GesturePoint[], m: ContourMapping, cellTicks: bigint): Cell[] {
  if (points.length === 0 || cellTicks <= 0n) return [];
  const pts = points
    .filter((p) => Number.isFinite(p.x) && Number.isFinite(p.y))
    .map((p) => ({ t: parseI64(m.xToTicks(p.x)), y: m.yToPitch(p.y) }))
    .sort((a, b) => (a.t < b.t ? -1 : a.t > b.t ? 1 : 0));
  if (pts.length === 0) return [];
  const firstCell = pts[0].t / cellTicks;
  const lastCell = pts[pts.length - 1].t / cellTicks;
  const cells: Cell[] = [];
  for (let c = firstCell; c <= lastCell; c++) {
    const lo = c * cellTicks;
    const hi = lo + cellTicks;
    const inside = pts.filter((p) => p.t >= lo && p.t < hi);
    if (inside.length === 0) continue; // sparse drawing → no note this cell
    const avg = inside.reduce((s, p) => s + p.y, 0) / inside.length;
    cells.push({ startTicks: lo, pitch: clampGesturePitch(avg), velocity: 0 });
  }
  return cells;
}

/**
 * Contour points → raw notes. `points` may arrive in any order; only
 * their mapped x positions matter. A draw spanning <1 cell still yields
 * exactly one note (≥ MIN_NOTE_LENGTH via the caller's cell size).
 */
export function contourToRawNotes(
  points: GesturePoint[],
  m: ContourMapping,
  opts: ContourOptions,
): RawGestureNote[] {
  const cellTicks = parseI64(opts.cellTicks);
  const cells = resample(points, m, cellTicks);
  const notes: RawGestureNote[] = [];
  const defaultVel = clampGestureVelocity(opts.velocity ?? 96);
  for (const cell of cells) {
    const vel = clampGestureVelocity(
      opts.velocityForCell
        ? opts.velocityForCell(cell.pitch, cell.startTicks.toString(10))
        : defaultVel,
    );
    const last = notes[notes.length - 1];
    if (last && last.rawPitch === cell.pitch && last.rawVelocity === vel) {
      // Same pitch across adjacent cells → extend the run, don't stack.
      last.lengthTicks = (
        parseI64(last.lengthTicks) + cellTicks
      ).toString(10);
      continue;
    }
    notes.push({
      index: notes.length,
      lengthTicks: cellTicks.toString(10),
      rawStartTicks: cell.startTicks.toString(10),
      rawPitch: cell.pitch,
      rawVelocity: vel,
    });
  }
  return notes;
}
