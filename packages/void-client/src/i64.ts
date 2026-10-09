// int64 helpers for the decimal-string wire format.
//
// CONTRACTS.md: large integers are decimal strings in JSON/TypeScript DTOs
// and native int64 in binary messages. Anything beyond
// Number.MAX_SAFE_INTEGER MUST go through as a string or bigint — a JS
// number would silently round.

const I64_MIN = -(2n ** 63n);
const I64_MAX = 2n ** 63n - 1n;
const U64_MAX = 2n ** 64n - 1n;
const DEC_RE = /^-?\d+$/;

export class I64Error extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = 'I64Error';
  }
}

/**
 * Coerce a wire int64 field to its decimal-string form.
 * - string: validated to be canonical decimal (no spaces, no exponent).
 * - bigint: always safe.
 * - number: must be an integer within i64 range; callers should prefer
 *   string/bigint for values that may exceed 2^53 — a number is accepted
 *   only when it is exactly representable.
 */
export function i64str(v: string | number | bigint): string {
  if (typeof v === 'bigint') {
    rangeCheck(v, true);
    return v.toString(10);
  }
  if (typeof v === 'number') {
    if (!Number.isFinite(v) || !Number.isInteger(v)) {
      throw new I64Error(`i64 number must be a finite integer, got ${v}`);
    }
    if (!Number.isSafeInteger(v)) {
      throw new I64Error(
        `i64 number ${v} exceeds safe integer range; pass a string or bigint`,
      );
    }
    return String(v);
  }
  const s = v.trim();
  if (!DEC_RE.test(s)) {
    throw new I64Error(`invalid i64 decimal string "${v}"`);
  }
  rangeCheck(BigInt(s), true);
  return s;
}

/** Unsigned variant (revision, engine_epoch, sequence fields). */
export function u64str(v: string | number | bigint): string {
  if (typeof v === 'bigint') {
    rangeCheck(v, false);
    return v.toString(10);
  }
  if (typeof v === 'number') {
    if (!Number.isFinite(v) || !Number.isInteger(v) || v < 0) {
      throw new I64Error(`u64 number must be a non-negative integer, got ${v}`);
    }
    if (!Number.isSafeInteger(v)) {
      throw new I64Error(
        `u64 number ${v} exceeds safe integer range; pass a string or bigint`,
      );
    }
    return String(v);
  }
  const s = v.trim();
  if (!DEC_RE.test(s) || s.startsWith('-')) {
    throw new I64Error(`invalid u64 decimal string "${v}"`);
  }
  rangeCheck(BigInt(s), false);
  return s;
}

/** Parse a wire decimal string back to bigint (exact). */
export function parseI64(s: string): bigint {
  const t = s.trim();
  if (!DEC_RE.test(t)) {
    throw new I64Error(`invalid i64 decimal string "${s}"`);
  }
  return BigInt(t);
}

/** Parse for display when the value is known to be small (throws otherwise). */
export function i64ToNumber(s: string): number {
  const v = parseI64(s);
  if (v > BigInt(Number.MAX_SAFE_INTEGER) || v < BigInt(-Number.MAX_SAFE_INTEGER)) {
    throw new I64Error(`i64 "${s}" exceeds safe JS number range`);
  }
  return Number(v);
}

function rangeCheck(v: bigint, signed: boolean): void {
  if (signed) {
    if (v < I64_MIN || v > I64_MAX) {
      throw new I64Error(`value ${v} out of int64 range`);
    }
  } else {
    if (v < 0n || v > U64_MAX) {
      throw new I64Error(`value ${v} out of uint64 range`);
    }
  }
}
