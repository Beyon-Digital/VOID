// Reversible gesture transform chain (MIDI-02, GEST-05).
//
//   rendered = humanize( swing( quantize( raw ) ) )
//
// Every stage is pure integer tick math (bigint) — no floats, no state.
// The chain ALWAYS re-derives from raw fields, which is what makes the
// transform reversible: strength 100% → 50% recomputes from the captured
// onset, it does not "un-quantize" a quantized value, so nothing ever
// compounds (MIDI-02 acceptance).

import { parseI64 } from 'void-client';
import {
  PPM,
  SWING_STRAIGHT_PPM,
  clampGesturePitch,
  clampGestureVelocity,
  type GestureNote,
  type PitchConstraint,
  type RawGestureNote,
  type RhythmTransform,
} from './types';

/** a*b/c rounded to nearest, ties away from zero (protocol rounding). */
export function mulDivRound(a: bigint, b: bigint, c: bigint): bigint {
  if (c === 0n) throw new Error('mulDivRound: divisor 0');
  const num = a * b;
  const sign = num < 0n ? -1n : 1n;
  const mag = num < 0n ? -num : num;
  return sign * ((mag + c / 2n) / c);
}

/** Nearest grid multiple to `t`; ties round away from zero. */
export function snapToGrid(t: bigint, grid: bigint): bigint {
  if (grid <= 0n) return t;
  const q = t / grid;
  const r = t % grid;
  const bump = (r < 0n ? -r : r) * 2n >= grid ? (r < 0n ? -1n : 1n) : 0n;
  return (q + bump) * grid;
}

/**
 * Quantize onset toward the grid by strength ppm.
 * strength 0 → raw; 1_000_000 → exactly on grid; in between is a real
 * interpolation `t + (snap - t) * s / PPM` — partial strength is musically
 * meaningful (tightens timing without snapping flat).
 */
export function quantizeOnset(t: bigint, grid: bigint, strengthPpm: number): bigint {
  const s = Math.min(PPM, Math.max(0, Math.round(strengthPpm)));
  if (grid <= 0n || s === 0) return t;
  const delta = snapToGrid(t, grid) - t;
  return t + mulDivRound(delta, BigInt(s), BigInt(PPM));
}

/**
 * Swing: delay onsets that fall in the second half of each two-step
 * pair so they land at `swingPpm` of the pair instead of the midpoint.
 * Implemented as a rigid offset of the off-beat window — the same model
 * step-sequencer swing uses; exact and reversible.
 */
export function swingOnset(t: bigint, grid: bigint, swingPpm: number): bigint {
  const s = Math.min(PPM, Math.max(0, Math.round(swingPpm)));
  if (grid <= 0n || s === SWING_STRAIGHT_PPM) return t;
  const pair = 2n * grid;
  // phase inside the pair, folded for negative times
  const phase = ((t % pair) + pair) % pair;
  if (phase < grid) return t; // on-beat window untouched
  const swung = mulDivRound(pair, BigInt(s), BigInt(PPM));
  return t + (swung - grid);
}

// ---------------------------------------------------------------------------
// deterministic humanize — seeded per-note jitter
// ---------------------------------------------------------------------------

/** FNV-1a 32-bit hash of the seed string. */
function hashSeed(seed: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < seed.length; i++) {
    h ^= seed.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}

/** mulberry32 — small seeded PRNG, one stream per note index. */
function stream(seed: string, index: number, lane: number): () => number {
  let a = (hashSeed(seed) + Math.imul(index + 1, 0x9e3779b9) + lane) >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) | 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** Deterministic signed jitter in [-mag, +mag]; stable for (seed,index,lane). */
export function jitter(seed: string, index: number, lane: number, mag: bigint): bigint {
  if (mag <= 0n) return 0n;
  const u = stream(seed, index, lane)(); // [0,1)
  // u*2-1 in (-1,1); scale by mag with integer rounding
  const scaled = mulDivRound(
    BigInt(Math.round(u * 2_000_000) - 1_000_000),
    mag,
    1_000_000n,
  );
  return scaled;
}

export const JITTER_TIMING_LANE = 0;
export const JITTER_VELOCITY_LANE = 1;

// ---------------------------------------------------------------------------
// the chain
// ---------------------------------------------------------------------------

/**
 * Render one raw note through the transform + optional pitch constraint.
 * Timing: quantize → swing → humanize. Velocity: humanize. Pitch:
 * constraint hook (harmony/). Length is never transformed — onsets move,
 * not durations, so overlapping-transform surprises stay out of the way
 * of T57's "raw intention retained".
 */
export function renderNote(
  raw: RawGestureNote,
  t: RhythmTransform,
  constrain?: PitchConstraint,
): GestureNote {
  const grid = parseI64(t.gridTicks);
  const rawStart = parseI64(raw.rawStartTicks);
  let start = quantizeOnset(rawStart, grid, t.quantizeStrengthPpm);
  start = swingOnset(start, grid, t.swingPpm);
  start += jitter(
    t.seed,
    raw.index,
    JITTER_TIMING_LANE,
    parseI64(t.humanizeTimingTicks),
  );
  const velJ = Math.round(
    Number(jitter(t.seed, raw.index, JITTER_VELOCITY_LANE, BigInt(t.humanizeVelocity))),
  );
  const pitch = constrain ? constrain(raw.rawPitch, raw.rawStartTicks) : raw.rawPitch;
  return {
    index: raw.index,
    pitch: clampGesturePitch(pitch),
    velocity: clampGestureVelocity(raw.rawVelocity + velJ),
    startTicks: start.toString(10),
    lengthTicks: raw.lengthTicks,
    rawStartTicks: raw.rawStartTicks,
    rawPitch: raw.rawPitch,
    rawVelocity: raw.rawVelocity,
  };
}

/** Render a whole phrase. Pure — same inputs always give same output. */
export function renderGesture(
  raw: RawGestureNote[],
  t: RhythmTransform,
  constrain?: PitchConstraint,
): GestureNote[] {
  return raw.map((n) => renderNote(n, t, constrain));
}
