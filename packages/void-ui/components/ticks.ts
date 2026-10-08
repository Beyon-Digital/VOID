// Tick formatting shared by ruler/surface labels. Mirrors CONTRACTS.md:
// 960,000 ticks per quarter note, signed int64 carried as a decimal string.

export const TICKS_PER_QUARTER = 960000n;

/** Format a tick as "B.bb.qq" (bar.beat.quarter-fraction), sign preserved. */
export function formatBarBeat(ticks: bigint | number | string, beatsPerBar = 4): string {
  const t = BigInt(ticks);
  const neg = t < 0n;
  const abs = neg ? -t : t;
  const perBar = TICKS_PER_QUARTER * BigInt(beatsPerBar);
  const bar = abs / perBar;
  const rem = abs % perBar;
  const beat = rem / TICKS_PER_QUARTER;
  const frac = rem % TICKS_PER_QUARTER;
  return `${neg ? '-' : ''}${bar}.${beat + 1n}${frac === 0n ? '' : `+${frac}`}`;
}

/** Quantize a tick to the nearest multiple of `step` (ties away from zero). */
export function quantizeTicks(ticks: bigint | number | string, step: bigint | number | string): string {
  const t = BigInt(ticks);
  const s = BigInt(step);
  if (s === 0n) return t.toString();
  const q = (t + (t < 0n ? -(s / 2n) : s / 2n)) / s;
  return (q * s).toString();
}
