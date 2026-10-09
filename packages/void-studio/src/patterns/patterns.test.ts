// W14 patterns coverage — T59 core.
//
// T59: "Build drum pattern, slice licensed sample, preview loop at
// project tempo, apply chord constraints and undo" → "Result stays
// inspectable/editable; source sample is immutable and harmonic/tempo
// conversions have clear controls."
// PAT-01: "Reloading a seeded pattern preserves its output; editing a
// row does not shift others."

import { describe, expect, it } from 'vitest';
import {
  bindSliceToPad,
  chokedBy,
  defaultPadBank,
  padToNote,
} from './pads';
import {
  builtinLoops,
  previewLoop,
  searchLoops,
} from './loops';
import {
  sliceByCount,
  sliceByPoints,
  SliceError,
  sliceMapToClipOps,
  sliceToClipOp,
} from './slices';
import {
  defaultStep,
  makeRow,
  patternCycleTicks,
  patternToInsertOps,
  renderPattern,
  setRowLength,
  setStep,
  stepRoll,
  toggleStep,
  type StepPattern,
} from './stepPattern';
import { createPatternEditorStore } from './store';

function mintSeq(prefix = 'm') {
  let n = 0;
  return () => `${prefix}-${++n}`;
}

function twoRowPattern(): StepPattern {
  return {
    patternId: 'p1',
    name: 'kit',
    stepTicks: '240000',
    seed: 'seed-A',
    rows: [
      makeRow('row-kick', 36, 4, 'kick'),
      makeRow('row-hat', 42, 8, 'hat'),
    ],
  };
}

describe('step-pattern editing is row-scoped and deterministic (PAT-01)', () => {
  it('toggle/setStep only touch the addressed row', () => {
    let p = twoRowPattern();
    p = toggleStep(p, 'row-kick', 0);
    expect(p.rows[0].steps[0].on).toBe(true);
    expect(p.rows[1].steps.every((s) => !s.on)).toBe(true);
    p = setStep(p, 'row-hat', 3, { on: true, velocity: 90 });
    expect(p.rows[1].steps[3]).toMatchObject({ on: true, velocity: 90 });
    expect(p.rows[0].steps[0].on).toBe(true);
  });

  it('stepRoll keys on (seed,rowId,globalStep) — editing a row does not shift others', () => {
    // PAT-01 acceptance: probabilities roll against the ROW IDENTITY,
    // never the row's position, so row edits keep other rows' output.
    const a = stepRoll('s', 'row-kick', 5);
    const b = stepRoll('s', 'row-hat', 5);
    expect(a).toBe(stepRoll('s', 'row-kick', 5));
    expect(a).not.toBe(b);
    // Rendering p1 then inserting a new row must not change row-hat hits.
    const p1 = twoRowPattern();
    p1.rows[0].steps[0].probabilityPpm = 500_000;
    p1.rows[0].steps[0].on = true;
    p1.rows[1].steps[2].probabilityPpm = 500_000;
    p1.rows[1].steps[2].on = true;
    const hatBefore = renderPattern(p1).filter((n) => n.rawPitch === 42);
    const p2 = { ...p1, rows: [makeRow('row-snare', 38, 4, 'snare'), ...p1.rows] };
    const hatAfter = renderPattern(p2).filter((n) => n.rawPitch === 42);
    expect(hatAfter.map((n) => n.rawStartTicks)).toEqual(
      hatBefore.map((n) => n.rawStartTicks),
    );
  });

  it('render is deterministic across runs (seeded probability)', () => {
    const p = twoRowPattern();
    p.rows[0].steps = p.rows[0].steps.map((s) => ({
      ...s,
      on: true,
      probabilityPpm: 600_000,
    }));
    const r1 = renderPattern(p, { cycles: 2 });
    const r2 = renderPattern(p, { cycles: 2 });
    expect(r1.map((n) => `${n.rawStartTicks}:${n.rawPitch}:${n.rawVelocity}`)).toEqual(
      r2.map((n) => `${n.rawStartTicks}:${n.rawPitch}:${n.rawVelocity}`),
    );
  });

  it('tie extends a note instead of emitting the next onset', () => {
    const row = makeRow('r', 36, 2);
    row.steps[0] = { ...defaultStep(), on: true };
    row.steps[1] = { ...defaultStep(), on: true, tie: true };
    const notes = renderPattern({ ...twoRowPattern(), rows: [row] });
    expect(notes).toHaveLength(1);
    expect(notes[0].lengthTicks).toBe('480000');
  });

  it('repeats ratchet within the step', () => {
    const row = makeRow('r', 36, 2);
    row.steps[0] = { ...defaultStep(), on: true, repeats: 3 };
    const notes = renderPattern({ ...twoRowPattern(), rows: [row] });
    expect(notes).toHaveLength(3);
    const onsets = notes.map((n) => BigInt(n.rawStartTicks));
    expect(onsets).toEqual([0n, 80_000n, 160_000n]); // 240000/3 = 80000
  });

  it('polyrhythm: cycle = longest row', () => {
    const p = twoRowPattern(); // 4-step kick + 8-step hat
    expect(patternCycleTicks(p)).toBe(1_920_000n);
  });

  it('setRowLength pads with off steps or truncates', () => {
    let p = twoRowPattern();
    p = setRowLength(p, 'row-kick', 6);
    expect(p.rows[0].steps).toHaveLength(6);
    p = setRowLength(p, 'row-kick', 2);
    expect(p.rows[0].steps).toHaveLength(2);
  });

  it('patternToInsertOps shares one transaction id across ops', () => {
    let p = twoRowPattern();
    p = toggleStep(toggleStep(p, 'row-kick', 0), 'row-hat', 2);
    const { transactionId, ops, noteIds } = patternToInsertOps('clip-9', p, mintSeq());
    expect(ops.length).toBe(noteIds.length);
    expect(ops.length).toBeGreaterThan(0);
    for (const op of ops) {
      expect(Object.keys(op)).toEqual(['InsertNoteOp']);
      expect('InsertNoteOp' in op && op.InsertNoteOp.clip_id).toBe('clip-9');
    }
    expect(typeof transactionId).toBe('string');
  });
});

describe('quick-slice — view+region spec, source immutable (T59)', () => {
  it('sliceByCount covers the whole asset, remainder distributed', () => {
    const m = sliceByCount('asset-1', '1000001', 4, mintSeq('s'));
    expect(m.slices).toHaveLength(4);
    const total = m.slices.reduce((s, x) => s + BigInt(x.lengthTicks), 0n);
    expect(total).toBe(1_000_001n);
    // First slice starts at 0, last ends exactly at asset length.
    expect(m.slices[0].startTicks).toBe('0');
    const last = m.slices[3];
    expect(BigInt(last.startTicks) + BigInt(last.lengthTicks)).toBe(1_000_001n);
    for (const s of m.slices) expect(s.assetId).toBe('asset-1');
  });

  it('sliceByPoints validates boundaries strictly inside (0,len)', () => {
    const ok = sliceByPoints('a', '100', ['25', '60'], mintSeq());
    expect(ok.slices.map((s) => s.startTicks)).toEqual(['0', '25', '60']);
    expect(ok.slices.map((s) => s.lengthTicks)).toEqual(['25', '35', '40']);
    expect(() => sliceByPoints('a', '100', ['0'], mintSeq())).toThrow(SliceError);
    expect(() => sliceByPoints('a', '100', ['100'], mintSeq())).toThrow(SliceError);
    expect(() => sliceByPoints('a', '100', ['-5'], mintSeq())).toThrow(SliceError);
    expect(() => sliceByPoints('a', '100', ['150'], mintSeq())).toThrow(SliceError);
    // Unsorted/duplicate input is normalized, not an error.
    const norm = sliceByPoints('a', '100', ['70', '30', '30'], mintSeq());
    expect(norm.slices.map((s) => s.startTicks)).toEqual(['0', '30', '70']);
  });

  it('sliceToClipOp emits an InsertAudioClipOp region ref — same asset, windowed', () => {
    const m = sliceByCount('asset-7', '960000', 2, mintSeq());
    const op = sliceToClipOp(m.slices[1], {
      clipId: 'clip-new',
      trackId: 'trk',
      atTicks: '480000',
    });
    expect(op).toEqual({
      InsertAudioClipOp: {
        clip_id: 'clip-new',
        track_id: 'trk',
        asset_id: 'asset-7',
        start_ticks: '480000',
        length_ticks: '480000',
        offset_ticks: '480000',
      },
    });
  });

  it('sliceMapToClipOps lays slices consecutively; asset untouched', () => {
    const m = sliceByCount('asset-3', '300000', 3, mintSeq());
    const { ops } = sliceMapToClipOps(m, { trackId: 't', atTicks: '0' }, mintSeq('c'));
    expect(ops).toHaveLength(3);
    let at = 0n;
    let offset = 0n;
    for (const op of ops) {
      if (!('InsertAudioClipOp' in op)) throw new Error('expected clip op');
      const c = op.InsertAudioClipOp;
      expect(BigInt(c.start_ticks)).toBe(at);
      expect(BigInt(c.offset_ticks ?? '0')).toBe(offset); // window into source
      expect(c.asset_id).toBe('asset-3'); // source asset never duplicated
      at += 100_000n;
      offset += 100_000n;
    }
  });
});

describe('drum pads', () => {
  it('choke group: open and closed hat choke each other', () => {
    const pads = defaultPadBank();
    const open = pads.find((p) => p.label.toLowerCase().includes('open'))!;
    const others = chokedBy(pads, open);
    expect(others.length).toBeGreaterThan(0);
    for (const o of others) expect(o.chokeGroup).toBe(open.chokeGroup);
    expect(others).not.toContainEqual(open);
  });

  it('bindSliceToPad + padToNote produces a raw note at the pad pitch', () => {
    const pads = defaultPadBank();
    const bound = bindSliceToPad(pads[0], { assetId: 'asset-x', sliceId: 'slice-1' });
    expect(bound.assetId).toBe('asset-x');
    expect(bound.sliceId).toBe('slice-1');
    const n = padToNote(bound, { atTicks: '0', lengthTicks: '240000', velocity: 110 });
    expect(n.rawPitch).toBe(bound.pitch);
    expect(n.rawVelocity).toBe(110);
    // Original pad untouched (pure).
    expect(pads[0].assetId).toBeUndefined();
  });
});

describe('loop templates — deterministic browser + tempo preview (T59)', () => {
  it('builtinLoops is a fixed deterministic list', () => {
    const a = builtinLoops();
    const b = builtinLoops();
    expect(a.map((t) => t.id)).toEqual(b.map((t) => t.id));
    expect(a.length).toBeGreaterThanOrEqual(4);
  });

  it('searchLoops filters by name/tag deterministically', () => {
    const all = builtinLoops();
    const son = searchLoops(all, { text: 'clave' });
    expect(son.some((t) => t.id.includes('son'))).toBe(true);
    const empty = searchLoops(all, { text: 'zzz-nonexistent' });
    expect(empty).toHaveLength(0);
  });

  it('previewLoop renders at PROJECT tempo; durationMs follows bpm', () => {
    const t = builtinLoops()[0];
    const slow = previewLoop(t, { bpm: 60, cycles: 1 });
    const fast = previewLoop(t, { bpm: 120, cycles: 1 });
    // Ticks are tempo-agnostic — same note set; wall-clock halves.
    expect(slow.durationTicks).toBe(fast.durationTicks);
    expect(slow.durationMs).toBeCloseTo(fast.durationMs * 2, 6);
    expect(fast.notes.length).toBeGreaterThan(0);
    // 2 cycles doubles the rendered span deterministically.
    const two = previewLoop(t, { bpm: 120, cycles: 2 });
    expect(BigInt(two.durationTicks)).toBe(BigInt(fast.durationTicks) * 2n);
  });
});

describe('pattern editor store', () => {
  it('applyTemplate deep-clones — editing the pattern never mutates the template', () => {
    const st = createPatternEditorStore();
    const t = builtinLoops()[0];
    st.getState().actions.applyTemplate(t);
    const firstRow = st.getState().pattern!.rows[0];
    const before = t.pattern.rows[0].steps[0].on;
    st.getState().actions.setFocus(firstRow.rowId, 0);
    st.getState().actions.toggleFocusedStep();
    // The store's clone changed; the shipped template is untouched.
    expect(st.getState().pattern!.rows[0].steps[0].on).toBe(!before);
    expect(t.pattern.rows[0].steps[0].on).toBe(before);
  });

  it('focus moves by delta within grid bounds; toggle is row-scoped', () => {
    const st = createPatternEditorStore();
    st.getState().actions.loadPattern(twoRowPattern());
    st.getState().actions.setFocus('row-hat', 0);
    st.getState().actions.moveFocus(0, 3);
    expect(st.getState().focus).toEqual({ rowId: 'row-hat', stepIndex: 3 });
    st.getState().actions.moveFocus(0, 9); // clamps at the row length
    expect(st.getState().focus!.stepIndex).toBeLessThanOrEqual(7);
    st.getState().actions.toggleFocusedStep();
    expect(st.getState().pattern!.rows[1].steps[st.getState().focus!.stepIndex].on).toBe(true);
    expect(st.getState().pattern!.rows[0].steps.every((s) => !s.on)).toBe(true);
  });
});
