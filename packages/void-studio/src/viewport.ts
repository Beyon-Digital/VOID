// Timeline viewport math — pure functions, BigInt-safe on the tick axis.
//
// CONTRACTS.md: ticks are signed int64 at 960_000 per quarter note; sample
// positions are int64 at engine rate; nearest-sample rounding is ties away
// from zero. DTOs carry ticks as decimal strings; the viewport converts to
// bigint for math and back to strings for storage/wire.
//
// Precision note: px-space math uses IEEE doubles. That is exact for tick
// deltas within a viewport (a px delta is always << 2^53 ticks at UI zoom
// levels) but never use it to compute sample-exact positions.

import { parseI64 } from 'void-client';

/** 960,000 protocol ticks per quarter note. */
export const TICKS_PER_QUARTER = 960000n;
/** One bar of 4/4 = 3,840,000 ticks. */
export const TICKS_PER_BAR_44 = 4n * TICKS_PER_QUARTER;

/** Zoom bounds for ticks-per-pixel. */
export const MIN_TICKS_PER_PIXEL = 1e-3;
export const MAX_TICKS_PER_PIXEL = 1e9;

/** Viewport range on the musical tick axis (decimal strings on the wire). */
export interface TickViewport {
  /** Left edge, ticks. May be negative (pickup region). */
  startTicks: string;
  /** Right edge, ticks. Always > startTicks. */
  endTicks: string;
}

export interface Zoom {
  /** Musical ticks covered by one screen pixel. Larger = zoomed out. */
  ticksPerPixel: number;
}

export function clampTicksPerPixel(v: number): number {
  if (!Number.isFinite(v) || v <= 0) return MIN_TICKS_PER_PIXEL;
  return Math.min(MAX_TICKS_PER_PIXEL, Math.max(MIN_TICKS_PER_PIXEL, v));
}

export function makeViewport(startTicks: string, endTicks: string): TickViewport {
  const start = parseI64(startTicks);
  const end = parseI64(endTicks);
  if (end <= start) {
    throw new Error(`viewport requires endTicks > startTicks (${startTicks}..${endTicks})`);
  }
  return { startTicks: start.toString(10), endTicks: end.toString(10) };
}

export function viewportSpan(v: TickViewport): bigint {
  return parseI64(v.endTicks) - parseI64(v.startTicks);
}

/** ticks → px offset from the viewport's left edge. Fractional result. */
export function ticksToPx(ticks: string, v: TickViewport, z: Zoom): number {
  const dt = parseI64(ticks) - parseI64(v.startTicks);
  return Number(dt) / z.ticksPerPixel;
}

/**
 * px offset → absolute ticks. Rounds to the nearest tick; halves round
 * away from zero (documented protocol rounding).
 */
export function pxToTicks(px: number, v: TickViewport, z: Zoom): string {
  const x = px * z.ticksPerPixel;
  const rounded = Math.sign(x) * Math.floor(Math.abs(x) + 0.5);
  return (parseI64(v.startTicks) + BigInt(rounded)).toString(10);
}

/** Pan the viewport by a pixel delta (drag). Returns a new viewport. */
export function panByPx(v: TickViewport, z: Zoom, pxDelta: number): TickViewport {
  const x = pxDelta * z.ticksPerPixel;
  const d = BigInt(Math.sign(x) * Math.floor(Math.abs(x) + 0.5));
  return makeViewport(
    (parseI64(v.startTicks) + d).toString(10),
    (parseI64(v.endTicks) + d).toString(10),
  );
}

/**
 * Zoom around an anchor pixel: the tick under anchorPx stays fixed on
 * screen. factor > 1 zooms in (fewer ticks per px); factor < 1 zooms out.
 */
export function zoomAtPx(
  v: TickViewport,
  z: Zoom,
  factor: number,
  anchorPx: number,
): { viewport: TickViewport; zoom: Zoom } {
  const anchorTicks = pxToTicks(anchorPx, v, z);
  const tpp = clampTicksPerPixel(z.ticksPerPixel / factor);
  // anchorPx = (anchorTicks - startTicks)/tpp → startTicks = anchorTicks - anchorPx*tpp
  const start = parseI64(anchorTicks) - BigInt(Math.floor(anchorPx * tpp));
  const span = viewportSpan(v);
  const newSpan = BigInt(Math.round(Number(span) * (tpp / z.ticksPerPixel)));
  const end = start + (newSpan > 0n ? newSpan : 1n);
  return { viewport: makeViewport(start.toString(10), end.toString(10)), zoom: { ticksPerPixel: tpp } };
}

/** Fit [lo,hi] ticks into widthPx pixels. */
export function fitRange(
  loTicks: string,
  hiTicks: string,
  widthPx: number,
): { viewport: TickViewport; zoom: Zoom } {
  if (widthPx <= 0) throw new Error('fitRange requires widthPx > 0');
  const v = makeViewport(loTicks, hiTicks);
  return {
    viewport: v,
    zoom: { ticksPerPixel: clampTicksPerPixel(Number(viewportSpan(v)) / widthPx) },
  };
}

/** Format helpers for labels (bars/beats assume constant 4/4 display). */
export function ticksToBarsBeats(ticks: string): { bar: number; beat: number; rest: bigint } {
  const t = parseI64(ticks);
  const bar = t / TICKS_PER_BAR_44;
  const rem = t % TICKS_PER_BAR_44;
  const beat = rem / TICKS_PER_QUARTER;
  return { bar: Number(bar), beat: Number(beat), rest: rem % TICKS_PER_QUARTER };
}
