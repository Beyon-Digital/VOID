import { describe, expect, it } from 'vitest';
import { formatBarBeat, quantizeTicks, TICKS_PER_QUARTER } from './ticks';

describe('ticks', () => {
  it('formats bar.beat for positive ticks', () => {
    expect(formatBarBeat(0n)).toBe('0.1');
    expect(formatBarBeat(TICKS_PER_QUARTER * 4n)).toBe('1.1');
    expect(formatBarBeat(TICKS_PER_QUARTER)).toBe('0.2');
    expect(formatBarBeat(TICKS_PER_QUARTER + 120n)).toBe('0.2+120');
  });

  it('keeps the sign on pickup ticks', () => {
    const s = formatBarBeat(-TICKS_PER_QUARTER);
    expect(s.startsWith('-')).toBe(true);
  });

  it('quantizes to nearest step, ties away from zero', () => {
    expect(quantizeTicks(10n, 4n)).toBe('12');
    expect(quantizeTicks(-10n, 4n)).toBe('-12');
    expect(quantizeTicks(9n, 4n)).toBe('8');
    expect(quantizeTicks('960000', '960000')).toBe('960000');
  });

  it('handles decimal-string ticks beyond Number.MAX_SAFE_INTEGER', () => {
    const big = '9223372036854775800';
    expect(formatBarBeat(big)).toMatch(/^-?\d+\.\d/);
  });
});
