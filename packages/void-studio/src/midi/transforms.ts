// Pure MIDI transforms (W18; MIDI-05, T71): transpose, velocity scale,
// time shift, retrograde. Every function is pure, validates, and keeps
// articulation/channel/expression intact — they ride the note object.
// Ticks stay decimal-string int64; offsets use bigint.

import { i64str, parseI64 } from 'void-client';
import type { MidiNote } from './types';
import { PITCH_MAX, PITCH_MIN, VELOCITY_MAX, VELOCITY_MIN } from './types';
import { MidiModelError } from './events';

export interface TransposeResult {
  notes: MidiNote[];
  /** Semitone shift actually applied per note after clamping. */
  applied: Map<string, number>;
}

/**
 * Transpose by `semitones`. Notes clamp at the MIDI edges and the
 * clamped amount is reported — the caller must NOT silently pretend a
 * +12 landed on an already-top note.
 */
export function transpose(notes: MidiNote[], semitones: number): TransposeResult {
  if (!Number.isInteger(semitones)) {
    throw new MidiModelError(`semitones must be an integer, got ${semitones}`);
  }
  const applied = new Map<string, number>();
  const out = notes.map((n) => {
    const raw = n.pitch + semitones;
    const pitch = Math.min(PITCH_MAX, Math.max(PITCH_MIN, raw));
    applied.set(n.id, pitch - n.pitch);
    return { ...n, pitch };
  });
  return { notes: out, applied };
}

/**
 * Scale velocities by `ratio` (≥ 0), rounding half-up, clamped to
 * [1,127]. Returns the per-note actual ratio for audit.
 */
export function scaleVelocity(
  notes: MidiNote[],
  ratio: number,
): { notes: MidiNote[]; applied: Map<string, number> } {
  if (!Number.isFinite(ratio) || ratio < 0) {
    throw new MidiModelError(`velocity ratio must be >= 0, got ${ratio}`);
  }
  const applied = new Map<string, number>();
  const out = notes.map((n) => {
    const raw = Math.round(n.velocity * ratio);
    const velocity = Math.min(VELOCITY_MAX, Math.max(VELOCITY_MIN, raw));
    applied.set(n.id, velocity / n.velocity);
    return { ...n, velocity };
  });
  return { notes: out, applied };
}

/** Shift note positions by `offsetTicks` (may be negative). */
export function shiftTime(notes: MidiNote[], offsetTicks: string): MidiNote[] {
  const d = parseI64(offsetTicks);
  return notes.map((n) => ({
    ...n,
    startTicks: i64str(parseI64(n.startTicks) + d),
  }));
}

/**
 * Mirror notes around `pivotTicks`: note starting at `pivot+d` after the
 * transform starts at `pivot-d-length` (the note's *end* mirrors to its
 * start). Preserves articulation and expression.
 */
export function retrograde(notes: MidiNote[], pivotTicks: string): MidiNote[] {
  const pivot = parseI64(pivotTicks);
  return notes.map((n) => {
    const end = parseI64(n.startTicks) + parseI64(n.lengthTicks);
    return { ...n, startTicks: i64str(2n * pivot - end) };
  });
}

/** Humanize: deterministic pseudo-random timing/velocity jitter. */
export function humanize(
  notes: MidiNote[],
  opts: { timeTicks?: string; velocity?: number; seed?: number } = {},
): MidiNote[] {
  const timeAmp = parseI64(opts.timeTicks ?? '0');
  const velAmp = opts.velocity ?? 0;
  if (timeAmp < 0n) throw new MidiModelError('humanize timeTicks must be >= 0');
  if (velAmp < 0) throw new MidiModelError('humanize velocity must be >= 0');
  let state = (opts.seed ?? 1) >>> 0 || 1;
  const rand = (): number => {
    // xorshift32 — deterministic across runs for round-trip tests.
    state ^= state << 13;
    state ^= state >>> 17;
    state ^= state << 5;
    return (state >>> 0) / 0xffffffff;
  };
  return notes.map((n) => {
    const dt = BigInt(Math.round((rand() * 2 - 1) * Number(timeAmp)));
    const dv = Math.round((rand() * 2 - 1) * velAmp);
    return {
      ...n,
      startTicks: i64str(parseI64(n.startTicks) + dt),
      velocity: Math.min(VELOCITY_MAX, Math.max(VELOCITY_MIN, n.velocity + dv)),
    };
  });
}
