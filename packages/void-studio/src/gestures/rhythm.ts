// Tap-rhythm capture (GEST-01): tap timestamps → raw note onsets.
//
// Taps land wherever the user put them — off-grid by nature. Their ms
// timestamps convert to exact tick onsets via the session tempo and the
// RAW ticks are kept verbatim; quantize/swing/humanize stay reversible
// transforms on top, never destructive rewrites (T57 acceptance).

import { TICKS_PER_QUARTER } from '../viewport';
import {
  clampGesturePitch,
  clampGestureVelocity,
  type RawGestureNote,
  type TapEvent,
} from './types';

/**
 * ms → ticks at `bpm`: ticks = ms · bpm · TICKS_PER_QUARTER / 60000.
 * Reduced to ms · bpm · 16 exactly (960000/60000); rounded to nearest
 * tick, ties away from zero.
 */
export function msToTicks(ms: number, bpm: number): bigint {
  if (!Number.isFinite(ms) || !Number.isFinite(bpm) || bpm <= 0) return 0n;
  const x = ms * bpm * 16;
  const rounded = Math.sign(x) * Math.floor(Math.abs(x) + 0.5);
  return BigInt(rounded);
}

/** ticks → ms at `bpm` (inverse of msToTicks, for display). */
export function ticksToMs(ticks: bigint, bpm: number): number {
  if (!Number.isFinite(bpm) || bpm <= 0) return 0;
  return Number(ticks) / (bpm * 16);
}

export interface RhythmOptions {
  /** Tempo the capture clock converts through (project bpm). */
  bpm: number;
  /** Tick where the phrase starts — the first tap's origin offset. */
  originTicks?: string;
  /** Pitch for tapped notes (drum lane / monotone input). */
  pitch?: number;
  /** Fallback velocity when a tap carries none (1..127). */
  velocity?: number;
  /**
   * Note length policy: fixed ticks, or 'gap' = run to the next tap.
   * 'gap' falls back to `gapFallbackTicks` for the final tap.
   */
  length?: string | 'gap';
  /** Used when length='gap' and no following tap exists. */
  gapFallbackTicks?: string;
}

/**
 * Taps → raw notes. The first tap anchors the phrase at originTicks;
 * subsequent taps keep their exact relative timing. Velocity comes from
 * the tap itself or the capture default. Pitch is uniform (the contour
 * gesture owns pitch; rhythm owns time).
 */
export function tapsToRawNotes(
  taps: TapEvent[],
  opts: RhythmOptions,
): RawGestureNote[] {
  if (taps.length === 0) return [];
  const sorted = [...taps]
    .filter((t) => Number.isFinite(t.tMs))
    .sort((a, b) => a.tMs - b.tMs);
  if (sorted.length === 0) return [];
  const origin = BigInt(opts.originTicks ?? '0');
  const t0 = sorted[0].tMs;
  const pitch = clampGesturePitch(opts.pitch ?? 60);
  const vel0 = clampGestureVelocity(opts.velocity ?? 100);
  const onsets = sorted.map((t) => origin + msToTicks(t.tMs - t0, opts.bpm));
  const fixedLen = opts.length !== undefined && opts.length !== 'gap'
    ? BigInt(opts.length)
    : undefined;
  const gapFallback = BigInt(opts.gapFallbackTicks ?? '240000');
  return onsets.map((onset, i) => {
    const next = onsets[i + 1];
    const gap = next !== undefined ? next - onset : gapFallback;
    const len = fixedLen ?? (gap > 0n ? gap : gapFallback);
    return {
      index: i,
      lengthTicks: (len > 0n ? len : gapFallback).toString(10),
      rawStartTicks: onset.toString(10),
      rawPitch: pitch,
      rawVelocity: clampGestureVelocity(sorted[i].velocity ?? vel0),
    };
  });
}

/**
 * Numeric/keyboard alternative for tapping (GEST-06): an explicit onset
 * list. `unit` selects the entry unit — 'ms' (raw capture clock), 'beats'
 * (converted through bpm), or 'ticks' (direct).
 */
export function onsetsToRawNotes(
  onsets: number[] | string[],
  unit: 'ms' | 'beats' | 'ticks',
  opts: Omit<RhythmOptions, 'originTicks'> & { originTicks?: string },
): RawGestureNote[] {
  const origin = BigInt(opts.originTicks ?? '0');
  const pitch = clampGesturePitch(opts.pitch ?? 60);
  const vel = clampGestureVelocity(opts.velocity ?? 100);
  const ticks = onsets
    .map((v) => {
      if (unit === 'ticks') return BigInt(String(v).trim());
      const n = Number(v);
      if (!Number.isFinite(n)) return null;
      if (unit === 'ms') return msToTicks(n, opts.bpm);
      return BigInt(Math.round(n * Number(TICKS_PER_QUARTER))); // beats
    })
    .filter((t): t is bigint => t !== null)
    .sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  const gapFallback = BigInt(opts.gapFallbackTicks ?? '240000');
  const fixedLen = opts.length !== undefined && opts.length !== 'gap'
    ? BigInt(opts.length)
    : undefined;
  return ticks.map((t, i) => {
    const next = ticks[i + 1];
    const gap = next !== undefined ? next - t : gapFallback;
    const len = fixedLen ?? (gap > 0n ? gap : gapFallback);
    return {
      index: i,
      lengthTicks: (len > 0n ? len : gapFallback).toString(10),
      rawStartTicks: (origin + t).toString(10),
      rawPitch: pitch,
      rawVelocity: vel,
    };
  });
}
