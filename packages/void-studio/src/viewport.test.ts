import { describe, expect, it } from 'vitest';
import {
  fitRange,
  makeViewport,
  panByPx,
  pxToTicks,
  ticksToBarsBeats,
  ticksToPx,
  viewportSpan,
  zoomAtPx,
} from './viewport';

const V = { startTicks: '0', endTicks: '15360000' }; // 4 bars of 4/4
const Z = { ticksPerPixel: 3840 }; // 250 px/quarter

describe('viewport construction', () => {
  it('accepts end > start and normalizes decimals', () => {
    const v = makeViewport('0', '3840000');
    expect(v.endTicks).toBe('3840000');
    expect(viewportSpan(v)).toBe(3840000n);
  });

  it('rejects empty/inverted ranges', () => {
    expect(() => makeViewport('10', '10')).toThrow(/endTicks > startTicks/);
    expect(() => makeViewport('20', '10')).toThrow();
  });

  it('allows negative start (pickup region per contract)', () => {
    const v = makeViewport('-480000', '3840000');
    expect(v.startTicks).toBe('-480000');
  });
});

describe('ticks<->px', () => {
  it('round-trips through pxToTicks/ticksToPx', () => {
    expect(ticksToPx('960000', V, Z)).toBe(250);
    expect(pxToTicks(250, V, Z)).toBe('960000');
  });

  it('rounds to nearest tick, ties away from zero', () => {
    const z = { ticksPerPixel: 2 }; // 1px = 2 ticks; 0.25px = 0.5 tick → away from 0
    expect(pxToTicks(0.25, { startTicks: '0', endTicks: '100' }, z)).toBe('1');
    expect(pxToTicks(-0.25, { startTicks: '0', endTicks: '100' }, z)).toBe('-1');
    expect(pxToTicks(0.26, { startTicks: '0', endTicks: '100' }, z)).toBe('1');
    expect(pxToTicks(0.24, { startTicks: '0', endTicks: '100' }, z)).toBe('0');
    // negative non-tie rounds to nearest, not away blindly
    const z2 = { ticksPerPixel: 10 };
    expect(pxToTicks(-0.23, { startTicks: '0', endTicks: '1000' }, z2)).toBe('-2');
  });

  it('handles large int64 tick values exactly', () => {
    const big = '9007199254740992'; // 2^53 — beyond exact f64 ints
    const v = makeViewport(big, (BigInt(big) + 3840000n).toString(10));
    expect(viewportSpan(v)).toBe(3840000n);
    // full-range sanity: span of an int64-sized viewport
    const wide = makeViewport('-9223372036854775808', '9223372036854775807');
    expect(viewportSpan(wide)).toBe(18446744073709551615n);
    const at = pxToTicks(0, wide, { ticksPerPixel: 1e9 });
    expect(BigInt(at)).toBe(-9223372036854775808n);
  });
});

describe('pan/zoom', () => {
  it('panByPx shifts both edges equally', () => {
    const v = panByPx(V, Z, 100); // +100px → +384000 ticks
    expect(v.startTicks).toBe('384000');
    expect(v.endTicks).toBe('15744000');
    expect(viewportSpan(v)).toBe(viewportSpan(V));
  });

  it('zoomAtPx keeps the anchor tick fixed', () => {
    const anchorPx = 250; // shows tick 960000
    const r = zoomAtPx(V, Z, 2, anchorPx); // zoom in ×2
    expect(r.zoom.ticksPerPixel).toBe(1920);
    // anchor still at px 250:
    const anchorTicks = pxToTicks(anchorPx, r.viewport, r.zoom);
    expect(anchorTicks).toBe('960000');
  });

  it('zoom clamps to bounds', () => {
    const r = zoomAtPx(V, Z, 1e12, 0);
    expect(r.zoom.ticksPerPixel).toBeGreaterThan(0);
    const r2 = zoomAtPx(V, Z, 1e-12, 0);
    expect(r2.zoom.ticksPerPixel).toBeLessThanOrEqual(1e9);
  });

  it('fitRange picks zoom covering the range', () => {
    const r = fitRange('0', '15360000', 800);
    expect(r.zoom.ticksPerPixel).toBeCloseTo(19200);
  });
});

describe('labels', () => {
  it('ticksToBarsBeats decomposes 4/4 positions', () => {
    expect(ticksToBarsBeats('0')).toEqual({ bar: 0, beat: 0, rest: 0n });
    expect(ticksToBarsBeats('3840000')).toEqual({ bar: 1, beat: 0, rest: 0n });
    expect(ticksToBarsBeats('4800000')).toEqual({ bar: 1, beat: 1, rest: 0n });
    expect(ticksToBarsBeats('5280000')).toEqual({
      bar: 1,
      beat: 1,
      rest: 480000n,
    });
  });
});
