// Tempo/meter map (W17/TIME-03, TIME-04; T67).
//
// Tempo and meter changes are REAL ops (SetTempoOp/SetTimeSignatureOp)
// under an explicit transactionId. Beats<->ticks is fixed: 960000
// ticks per quarter note (CONTRACTS), so tick math never needs the
// tempo map; time-of-day conversion needs a BPM anchor and is the
// engine lane's job (we provide beat-anchored arithmetic only, in
// rational-safe decimal micro-units to keep tests exact).

import { i64str, parseI64 } from 'void-client';
import type { PersistentOp } from 'void-client';
import type { Section } from './types';
import { TICKS_PER_QUARTER } from '../viewport';

/** A tempo-map event (view-model row; applied via SetTempoOp). */
export interface TempoEvent {
  atTicks: string;
  bpm: number;
  /** Linear ramp from this event to the next; engine renders ramps
   *  (step-only wire). Spec-level; NEEDS rev2. */
  ramp?: boolean;
}

/** A meter event (4/4 default). */
export interface MeterEvent {
  atTicks: string;
  numerator: number;
  denominator: number;
}

export class TempoError extends Error {}

/**
 * Build ops to insert a tempo change. One transaction; undo removes
 * it as a unit.
 */
export function insertTempoOp(atTicks: string, bpm: number, mint: () => string) {
  if (!(bpm > 0)) throw new TempoError(`bpm must be > 0, got ${bpm}`);
  const transactionId = mint();
  return {
    transactionId,
    ops: [
      {
        SetTempoOp: { at_ticks: i64str(parseI64(atTicks)), bpm },
      },
    ],
  };
}

export function insertMeterOp(
  atTicks: string,
  numerator: number,
  denominator: number,
  mint: () => string,
) {
  if (numerator <= 0 || denominator <= 0) {
    throw new TempoError(`bad meter ${numerator}/${denominator}`);
  }
  const transactionId = mint();
  return {
    transactionId,
    ops: [
      {
        SetTimeSignatureOp: {
          at_ticks: i64str(parseI64(atTicks)),
          numerator,
          denominator,
        },
      } as PersistentOp,
    ],
  };
}

/**
 * Beats (ticks) -> bar:beat position under a meter map. Pure rational
 * arithmetic — no floats, exact at every boundary.
 */
export function ticksToBarBeat(
  ticks: string,
  meters: MeterEvent[] = [{ atTicks: '0', numerator: 4, denominator: 4 }],
): { bar: number; beatInBar: number; beatFraction: { num: bigint; den: bigint } } {
  const t = parseI64(ticks);
  const sorted = [...meters].sort((a, b) =>
    Number(parseI64(a.atTicks) - parseI64(b.atTicks)),
  );
  let bar = 0;
  let cursor = 0n;
  let meter = sorted[0] ?? { atTicks: '0', numerator: 4, denominator: 4 };
  for (const m of sorted) {
    const mT = parseI64(m.atTicks);
    if (mT <= cursor) {
      meter = m;
      continue;
    }
    if (mT > t) break;
    const span = mT - cursor;
    const ticksPerBeat = TICKS_PER_QUARTER * (4n / BigInt(meter.denominator));
    const ticksPerBar = ticksPerBeat * BigInt(meter.numerator);
    bar += Number(span / ticksPerBar);
    const rem = span % ticksPerBar;
    if (rem !== 0n) {
      // position mid-bar at meter change: the leftover rolls into the
      // next bar of the NEW meter
      cursor = mT - rem;
    } else {
      cursor = mT;
    }
    meter = m;
  }
  const ticksPerBeat = TICKS_PER_QUARTER * (4n / BigInt(meter.denominator));
  const ticksPerBar = ticksPerBeat * BigInt(meter.numerator);
  const into = t - cursor;
  bar += Number(into / ticksPerBar);
  const barRem = into % ticksPerBar;
  const beatInBar = Number(barRem / ticksPerBeat);
  const frac = barRem % ticksPerBeat;
  const g = gcd(frac, ticksPerBeat);
  return {
    bar,
    beatInBar,
    beatFraction: { num: frac / g, den: ticksPerBeat / g },
  };
}

function gcd(a: bigint, b: bigint): bigint {
  let x = a < 0n ? -a : a;
  let y = b < 0n ? -b : b;
  while (y !== 0n) {
    const t = x % y;
    x = y;
    y = t;
  }
  return x === 0n ? 1n : x;
}

/** Bar/beat -> ticks under a meter map (inverse of ticksToBarBeat
 * for whole-beat inputs; fractional beats go through beatsToTicks). */
export function beatsToTicks(beats: number): string {
  return i64str(BigInt(Math.round(beats)) * TICKS_PER_QUARTER);
}

/** Section <-> bars: bars a section spans under a meter map (for the
 * T67 ruler readout). */
export function sectionBars(section: Section, meters?: MeterEvent[]): {
  startBar: number;
  endBar: number;
} {
  const start = ticksToBarBeat(section.startTicks, meters).bar;
  const end = ticksToBarBeat(
    i64str(parseI64(section.startTicks) + parseI64(section.lengthTicks)),
    meters,
  ).bar;
  return { startBar: start, endBar: end };
}
