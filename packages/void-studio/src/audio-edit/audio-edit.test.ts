import { describe, expect, it } from 'vitest';
import { i64str } from 'void-client';
import {
  bankOnMove,
  bankOnRemove,
  bankOnSplit,
  bankOnTrim,
  deleteMarker,
  insertMarker,
  markersFromOnsets,
  markersOnSplit,
  nearestMarker,
  shiftTransientMarkers,
  type MarkerBank,
} from './transients';
import {
  divRoundTiesAway,
  mapOutputToSource,
  mapSourceToOutput,
  outputLength,
  varispeed,
  varispeedCents,
  varispeedFromString,
  varispeedSpec,
} from './varispeed';
import {
  correctNote,
  correctTo,
  effectiveCents,
  isUnedited,
  pitchEditFromDetection,
  pitchEditSpec,
  resetAll,
  resetNote,
  setFormant,
  shiftNote,
} from './pitch';
import { renderTail, renderTailParams } from './tails';

describe('transient markers', () => {
  const CLIP = 'clip-1';
  const LEN = i64str(960 * 4);
  let n = 0;
  const mint = () => `m-${++n}`;

  it('builds markers from detected onsets, skipping out-of-bounds and dupes', () => {
    const ms = markersFromOnsets(
      LEN,
      [
        { ticks: '480', strength: 0.9 },
        { ticks: '1920', strength: 0.4 },
        { ticks: '-10' },
        { ticks: '999999' },
        { ticks: '480' },
      ],
      mint,
    );
    expect(ms.map((m) => m.ticks)).toEqual(['480', '1920']);
    expect(ms[0].strength).toBe(0.9);
  });

  it('inserts in order, deletes by id, rejects dup ticks and out-of-bounds', () => {
    let bank: MarkerBank = {};
    bank = insertMarker(bank, CLIP, LEN, { id: 'a', ticks: '960' });
    bank = insertMarker(bank, CLIP, LEN, { id: 'b', ticks: '480' });
    expect(bank[CLIP].map((m) => m.id)).toEqual(['b', 'a']);
    expect(() =>
      insertMarker(bank, CLIP, LEN, { id: 'c', ticks: '480' }),
    ).toThrow(/already exists/);
    expect(() =>
      insertMarker(bank, CLIP, LEN, { id: 'c', ticks: '99999' }),
    ).toThrow(/bounds/);
    bank = deleteMarker(bank, CLIP, 'b');
    expect(bank[CLIP]).toHaveLength(1);
    expect(() => deleteMarker(bank, CLIP, 'nope')).toThrow(/not on clip/);
  });

  it('shifts markers inside a region, refuses out-of-bounds shifts', () => {
    let bank: MarkerBank = {};
    bank = insertMarker(bank, CLIP, LEN, { id: 'a', ticks: '480' });
    bank = insertMarker(bank, CLIP, LEN, { id: 'b', ticks: '1440' });
    const moved = shiftTransientMarkers(
      bank,
      CLIP,
      LEN,
      { startTicks: '0', lengthTicks: '960' },
      '240',
    );
    expect(moved[CLIP].map((m) => m.ticks)).toEqual(['720', '1440']);
    expect(() =>
      shiftTransientMarkers(bank, CLIP, LEN, { startTicks: '0', lengthTicks: '960' }, '-1000'),
    ).toThrow(/bounds/);
  });

  it('markers survive split — left keeps pre-cut, right re-zeros (T73/T74 invariant)', () => {
    const markers = [
      { id: 'a', ticks: '480' },
      { id: 'b', ticks: '1440' },
      { id: 'c', ticks: '2400' },
    ];
    const { left, right } = markersOnSplit(markers, '1920');
    expect(left.map((m) => m.ticks)).toEqual(['480', '1440']);
    expect(right.map((m) => ({ id: m.id, t: m.ticks }))).toEqual([
      { id: 'c', t: '480' },
    ]);
    const bank = bankOnSplit({ [CLIP]: markers }, CLIP, 'clip-2', '1920');
    expect(bank[CLIP].map((m) => m.ticks)).toEqual(['480', '1440']);
    expect(bank['clip-2'].map((m) => m.ticks)).toEqual(['480']);
  });

  it('markers survive a move untouched (clip-local coordinates)', () => {
    const bank: MarkerBank = { [CLIP]: [{ id: 'a', ticks: '480' }] };
    expect(bankOnMove(bank)[CLIP][0].ticks).toBe('480');
  });

  it('markers survive a trim — dropped outside, re-zeroed inside', () => {
    const bank: MarkerBank = {
      [CLIP]: [
        { id: 'a', ticks: '480' },
        { id: 'b', ticks: '1440' },
        { id: 'c', ticks: '2400' },
      ],
    };
    const next = bankOnTrim(bank, CLIP, '960', '960');
    expect(next[CLIP].map((m) => m.ticks)).toEqual(['480']);
    expect(next[CLIP][0].id).toBe('b');
  });

  it('drops markers when the clip is removed', () => {
    const bank: MarkerBank = { [CLIP]: [{ id: 'a', ticks: '480' }] };
    expect(bankOnRemove(bank, CLIP)).toEqual({});
  });

  it('nearestMarker snaps to the closest anchor', () => {
    const ms = [
      { id: 'a', ticks: '480' },
      { id: 'b', ticks: '1440' },
    ];
    expect(nearestMarker(ms, '900')?.id).toBe('a');
    expect(nearestMarker(ms, '1000')?.id).toBe('b');
  });
});

describe('varispeed', () => {
  it('normalises rates and enforces the 1/8..8x bound', () => {
    expect(varispeed(4n, 2n)).toEqual({ num: 2n, den: 1n });
    expect(() => varispeed(0n, 1n)).toThrow(/positive/);
    expect(() => varispeed(9n, 1n)).toThrow(/above 8x/);
    expect(() => varispeed(1n, 9n)).toThrow(/below 1\/8x/);
    expect(varispeedFromString('3/2')).toEqual({ num: 3n, den: 2n });
  });

  it('ties-away-from-zero rounding is symmetric', () => {
    expect(divRoundTiesAway(5n, 4n)).toBe(1n);
    expect(divRoundTiesAway(-5n, 4n)).toBe(-1n);
    expect(divRoundTiesAway(6n, 4n)).toBe(2n); // 1.5 -> 2
    expect(divRoundTiesAway(-6n, 4n)).toBe(-2n);
  });

  it('maps output->source and source->output consistently', () => {
    const r = varispeed(2n, 1n);
    expect(mapOutputToSource(100n, r)).toBe(200n);
    expect(mapSourceToOutput(200n, r)).toBe(100n);
    expect(outputLength(48000n, r)).toBe(24000n);
    const half = varispeed(1n, 2n);
    expect(mapOutputToSource(100n, half)).toBe(50n);
    expect(outputLength(48000n, half)).toBe(96000n);
  });

  it('round-trips source->output->source at rational rates', () => {
    const r = varispeed(3n, 2n);
    for (const f of [0n, 1n, 7n, 1000n, 48000n]) {
      expect(mapOutputToSource(mapSourceToOutput(f * r.num, r), r)).toBe(f * r.num);
    }
  });

  it('pitch coupling: 2x is +1200c, 1/2x is -1200c', () => {
    expect(varispeedCents(varispeed(2n, 1n))).toBeCloseTo(1200);
    expect(varispeedCents(varispeed(1n, 2n))).toBeCloseTo(-1200);
    expect(varispeedCents(varispeed(1n, 1n))).toBeCloseTo(0);
  });

  it('spec carries mode + rational rate; time-only reports 0c', () => {
    const spec = varispeedSpec(varispeed(3n, 2n), 'time-only');
    expect(spec.mode).toBe('time-only');
    expect(spec.rate).toBe('3/2');
    expect(spec.cents).toBe(0);
  });
});

describe('pitch edit model (T74 nondestructive)', () => {
  const detected = [
    { id: 'n1', startTicks: '0', lengthTicks: '480', detectedCents: 6000 },
    { id: 'n2', startTicks: '480', lengthTicks: '480', detectedCents: 5853 },
  ];

  it('builds an unedited model from detection', () => {
    const edit = pitchEditFromDetection('c1', 'sha-x', detected);
    expect(isUnedited(edit)).toBe(true);
    expect(edit.notes[0].detectedCents).toBe(6000);
    expect(effectiveCents(edit.notes[0])).toBe(6000);
  });

  it('layers correction + shift; effective = detected + both', () => {
    let edit = pitchEditFromDetection('c1', 'sha-x', detected);
    edit = correctNote(edit, 'n2', 147); // detuned -147c -> correct up
    edit = shiftNote(edit, 'n2', 200); // +2 semitones
    edit = setFormant(edit, 'n2', -50);
    const n2 = edit.notes.find((n) => n.id === 'n2')!;
    expect(effectiveCents(n2)).toBe(6200);
    expect(n2.detectedCents).toBe(5853); // original retained
  });

  it('correctTo snaps to an absolute target pitch', () => {
    const edit = correctTo(pitchEditFromDetection('c1', 'sha-x', detected), 'n2', 5900);
    const n2 = edit.notes.find((n) => n.id === 'n2')!;
    expect(n2.correctionCents).toBe(47);
    expect(effectiveCents(n2)).toBe(5900);
  });

  it('resetNote reverses exactly to source — the nondestructive invariant', () => {
    let edit = pitchEditFromDetection('c1', 'sha-x', detected);
    edit = correctNote(edit, 'n1', 300);
    edit = shiftNote(edit, 'n1', -1200);
    edit = setFormant(edit, 'n1', 80);
    edit = resetNote(edit, 'n1');
    const n1 = edit.notes.find((n) => n.id === 'n1')!;
    expect(n1.correctionCents).toBe(0);
    expect(n1.shiftCents).toBe(0);
    expect(n1.formantCents).toBe(0);
    expect(n1.detectedCents).toBe(6000);
    expect(isUnedited(edit)).toBe(true);
  });

  it('resetAll returns the whole edit to detection', () => {
    let edit = pitchEditFromDetection('c1', 'sha-x', detected);
    edit = correctNote(edit, 'n1', 100);
    edit = shiftNote(edit, 'n2', -700);
    edit = resetAll(edit);
    expect(isUnedited(edit)).toBe(true);
  });

  it('the spec serialises detected + edits for the renderer', () => {
    let edit = pitchEditFromDetection('c1', 'sha-x', detected);
    edit = correctNote(edit, 'n1', 100);
    const spec = pitchEditSpec(edit);
    expect(spec.notes[0]).toMatchObject({
      id: 'n1',
      detectedCents: 6000,
      correctionCents: 100,
      shiftCents: 0,
    });
    expect(spec.assetSha256).toBe('sha-x');
  });
});

describe('render tails', () => {
  it('accepts the three tail policies and validates bounds', () => {
    expect(renderTail({ tail: { mode: 'none' } }).tail.mode).toBe('none');
    expect(renderTail({ tail: { mode: 'milliseconds', ms: 1500 } }).tail.mode).toBe(
      'milliseconds',
    );
    expect(() => renderTail({ tail: { mode: 'milliseconds', ms: 99999 } })).toThrow(
      /bound/,
    );
    expect(() => renderTail({ tail: { mode: 'ticks', ticks: '-5' } })).toThrow(
      />= 0/,
    );
  });

  it('tail fade must fit inside the tail', () => {
    const spec = {
      tail: { mode: 'ticks', ticks: '960' } as const,
      fadeOut: { shape: 'equal-power' as const, lengthTicks: '960' },
    };
    expect(renderTail(spec).fadeOut?.lengthTicks).toBe('960');
    expect(() =>
      renderTail({
        tail: { mode: 'ticks', ticks: '960' },
        fadeOut: { shape: 'equal-power', lengthTicks: '961' },
      }),
    ).toThrow(/exceeds/);
    expect(() =>
      renderTail({
        tail: { mode: 'none' },
        fadeOut: { shape: 'equal-power', lengthTicks: '120' },
      }),
    ).toThrow(/non-none/);
  });

  it('params carry the void-export tail wire shape + proposed fade fields', () => {
    const p = renderTailParams({
      tail: { mode: 'ticks', ticks: '1920' },
      fadeOut: { shape: 'linear', lengthTicks: '480' },
    });
    expect(p.tailPolicy).toEqual({ mode: 'ticks', ticks: '1920' });
    expect(p.tailFadeShape).toBe('linear');
    expect(p.tailFadeTicks).toBe('480');
  });
});
