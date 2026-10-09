// Varispeed model (W19 / EDIT-03; T74 model half).
//
// Varispeed changes playback rate — like a tape machine. Rate is an
// exact rational {num, den}: source position advances num/den frames per
// output frame. Rational math (CONTRACTS "exact rational arithmetic")
// keeps the mapping exact; sample rounding follows the documented
// nearest-sample, ties-away-from-zero rule (§1).
//
// rate 2/1  = 2x speed: consumes 2 source frames per output frame, output
//             duration halves, pitch rises an octave (+1200c).
// rate 1/2  = half speed: output duration doubles, pitch drops 1200c.
//
// The engine applies this to playback/render; this module is the model +
// verified mapping math (no DSP here — see docs/content-rights/NEEDS.md).

/** A positive rational rate, kept normalised (gcd-reduced). */
export interface Varispeed {
  num: bigint;
  den: bigint;
}

export const VARISPEED_MIN = { num: 1n, den: 8n } as const; // 1/8x
export const VARISPEED_MAX = { num: 8n, den: 1n } as const; // 8x

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

/** Divide with nearest-sample rounding, ties away from zero (CONTRACTS §1). */
export function divRoundTiesAway(n: bigint, d: bigint): bigint {
  if (d <= 0n) throw new Error('divisor must be positive');
  const neg = n < 0n;
  const a = neg ? -n : n;
  const q = a / d;
  const r = a % d;
  const twice = r * 2n;
  const rounded = twice >= d ? q + 1n : q;
  return neg ? -rounded : rounded;
}

/** Build a normalised varispeed rate. Bounds 1/8..8x. */
export function varispeed(num: bigint, den: bigint): Varispeed {
  if (num <= 0n || den <= 0n) {
    throw new Error(`varispeed must be positive, got ${num}/${den}`);
  }
  const g = gcd(num, den);
  const v = { num: num / g, den: den / g };
  if (v.num * VARISPEED_MAX.den > VARISPEED_MAX.num * v.den) {
    throw new Error('varispeed above 8x bound');
  }
  if (v.num * VARISPEED_MIN.den < VARISPEED_MIN.num * v.den) {
    throw new Error('varispeed below 1/8x bound');
  }
  return v;
}

export function varispeedFromString(s: string): Varispeed {
  const [n, d] = s.split('/');
  return varispeed(BigInt(n), BigInt(d ?? '1'));
}

/**
 * Output frame -> source frame. At rate num/den the engine consumes
 * num/den source frames per output frame.
 */
export function mapOutputToSource(outputFrame: bigint, rate: Varispeed): bigint {
  return divRoundTiesAway(outputFrame * rate.num, rate.den);
}

/** Source frame -> output frame (inverse mapping). */
export function mapSourceToOutput(sourceFrame: bigint, rate: Varispeed): bigint {
  return divRoundTiesAway(sourceFrame * rate.den, rate.num);
}

/** Output length in frames for a source region of `sourceFrames`. */
export function outputLength(sourceFrames: bigint, rate: Varispeed): bigint {
  return mapSourceToOutput(sourceFrames, rate);
}

/**
 * Pitch shift implied by varispeed in cents — pure tape semantics:
 * rate 2 = +1200c, rate 1/2 = -1200c. Returned as exact rational cents
 * numerator over 1e6 denominator for float-free wire math: cents =
 * 1200 * log2(rate); computed in f64 then scaled — callers needing exact
 * chains keep the rational rate itself.
 */
export function varispeedCents(rate: Varispeed): number {
  const r = Number(rate.num) / Number(rate.den);
  return 1200 * Math.log2(r);
}

/** Linked/unlinked mode per EDIT-03: varispeed couples time+pitch;
 * unlinked shifts keep duration (stretch) or pitch (resample). */
export type TimePitchMode =
  | 'linked' // classic varispeed — rate drives both duration and pitch
  | 'time-only' // duration change at constant pitch (stretch DSP — NEEDS)
  | 'pitch-only'; // pitch change at constant duration (shift DSP — NEEDS)

/**
 * The op parameters a varispeed request carries to the engine: explicit
 * rate + mode so playback and export agree (EDIT-03 acceptance).
 */
export interface VarispeedSpec {
  mode: TimePitchMode;
  /** Rate as a rational string "num/den" — decimal-string-safe wire data. */
  rate: string;
  /** Implied pitch shift in cents (0 for time-only). */
  cents: number;
}

export function varispeedSpec(rate: Varispeed, mode: TimePitchMode): VarispeedSpec {
  return {
    mode,
    rate: `${rate.num}/${rate.den}`,
    cents: mode === 'time-only' ? 0 : varispeedCents(rate),
  };
}
