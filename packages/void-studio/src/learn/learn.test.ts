// W14 learn coverage — T58 + MIX-07.
//
// T58: "Map touch/trackpad/MIDI control to a parameter, navigate
// elsewhere, disconnect controller and lose focus" → "Only armed target
// responds; navigation gestures do not change music. Panic/release has
// priority and prevents held notes."
// MIX-07: "Every mapped input updates the same parameter; out-of-range
// values are rejected or clamped."

import { describe, expect, it } from 'vitest';
import { applyCurve, mapMacroValue, mapValue, checkMapping, MappingError } from './values';
import { createLearnStore } from './store';
import type { ControlEvent, Macro, Mapping } from './types';

const cc20: Mapping = {
  mappingId: 'm1',
  control: { kind: 'cc', channel: 0, id: '20' },
  target: { kind: 'param', id: 'filter.cutoff' },
  curve: 'linear',
  min: 0,
  max: 1,
  invert: false,
  outOfRange: 'clamp',
};

const cc21: Mapping = {
  ...cc20,
  mappingId: 'm2',
  control: { kind: 'cc', channel: 0, id: '21' },
  target: { kind: 'param', id: 'synth.res' },
};

const ev = (partial: Partial<ControlEvent> = {}): ControlEvent => ({
  control: { kind: 'cc', channel: 0, id: '20' },
  value: 0.5,
  phase: 'value',
  atMs: 0,
  ...partial,
});

describe('value math (MIX-07)', () => {
  it('linear maps 0..1 into [min,max]', () => {
    expect(mapValue(cc20, 0)).toBe(0);
    expect(mapValue(cc20, 0.5)).toBe(0.5);
    expect(mapValue(cc20, 1)).toBe(1);
    const scaled = { ...cc20, min: 20, max: 20000 };
    expect(mapValue(scaled, 0.5)).toBeCloseTo(10010);
  });

  it('curves are real: exponential skews low, logarithmic skews high, toggle thresholds', () => {
    expect(applyCurve(0.5, 'exponential')).toBeCloseTo(0.25);
    expect(applyCurve(0.25, 'logarithmic')).toBeCloseTo(0.5);
    expect(applyCurve(0.4, 'toggle')).toBe(0);
    expect(applyCurve(0.6, 'toggle')).toBe(1);
    expect(applyCurve(0.5, 'linear')).toBe(0.5);
  });

  it('out-of-range policy: clamp bounds, reject drops', () => {
    expect(mapValue({ ...cc20, outOfRange: 'clamp' }, 1.4)).toBe(1);
    expect(mapValue({ ...cc20, outOfRange: 'reject' }, 1.4)).toBeNull();
    expect(mapValue({ ...cc20, outOfRange: 'reject' }, -0.1)).toBeNull();
    expect(mapValue({ ...cc20, outOfRange: 'reject' }, 0.7)).toBeCloseTo(0.7);
  });

  it('invert flips the input direction', () => {
    expect(mapValue({ ...cc20, invert: true }, 0.25)).toBeCloseTo(0.75);
  });

  it('invalid ranges are rejected at bind time', () => {
    expect(() => checkMapping({ ...cc20, min: 2, max: 1 })).toThrow(MappingError);
  });

  it('macro fans out through member curves/ranges', () => {
    const macro: Macro = {
      macroId: 'macro-1',
      name: 'bright',
      members: [
        { paramId: 'cut', curve: 'linear', min: 0, max: 1 },
        { paramId: 'res', curve: 'exponential', min: 0, max: 4 },
      ],
    };
    const out = mapMacroValue(macro, 0.5);
    expect(out[0]).toEqual({ paramId: 'cut', value: 0.5 });
    expect(out[1].paramId).toBe('res');
    expect(out[1].value).toBeCloseTo(1); // 4 × (0.5²)
  });
});

describe('arming — only the armed target responds (T58)', () => {
  it('no arm → control ignored (navigation gestures do not change music)', () => {
    const st = createLearnStore({ mappings: [cc20] });
    const r = st.getState().actions.dispatch(ev());
    expect(r.changes).toHaveLength(0);
    expect(r.ignored).toBe('no-arm');
  });

  it('armed param receives mapped values; unarmed mappings stay silent', () => {
    const st = createLearnStore({ mappings: [cc20, cc21] });
    st.getState().actions.arm({ kind: 'param', id: 'filter.cutoff' });
    const r1 = st.getState().actions.dispatch(ev({ value: 0.25 }));
    expect(r1.changes).toEqual([
      { paramId: 'filter.cutoff', value: 0.25, mappingId: 'm1', atMs: 0 },
    ]);
    const r2 = st.getState().actions.dispatch(ev({ control: cc21.control, value: 0.9 }));
    expect(r2.changes).toHaveLength(0);
    expect(r2.ignored).toBe('target-not-armed');
  });

  it('a control binds exactly one target — rebind replaces (MIX-07)', () => {
    const st = createLearnStore({ mappings: [cc20] });
    st.getState().actions.upsertMapping({
      ...cc20,
      mappingId: 'm9',
      target: { kind: 'param', id: 'other' },
    });
    expect(st.getState().mappings).toHaveLength(1);
    expect(st.getState().mappings[0].target.id).toBe('other');
  });

  it('learn capture binds the next moved control to the learn target', () => {
    const st = createLearnStore();
    st.getState().actions.beginLearn({ kind: 'param', id: 'mix.send' });
    expect(st.getState().phase).toBe('capturing');
    const r = st.getState().actions.dispatch(
      ev({ control: { kind: 'cc', channel: 1, id: '74' }, phase: 'down' }),
    );
    expect(st.getState().phase).toBe('idle');
    expect(st.getState().mappings).toHaveLength(1);
    expect(st.getState().mappings[0].control).toEqual({ kind: 'cc', channel: 1, id: '74' });
    expect(st.getState().mappings[0].target.id).toBe('mix.send');
    expect(r.changes).toHaveLength(0); // capture consumes the event
  });

  it('macro target fan-out reaches member params only when armed', () => {
    const st = createLearnStore({
      mappings: [{ ...cc20, target: { kind: 'macro', id: 'macro-1' } }],
      macros: [
        {
          macroId: 'macro-1',
          name: 'm',
          members: [
            { paramId: 'a', curve: 'linear', min: 0, max: 10 },
            { paramId: 'b', curve: 'linear', min: 0, max: 100 },
          ],
        },
      ],
    });
    st.getState().actions.arm({ kind: 'macro', id: 'macro-1' });
    const r = st.getState().actions.dispatch(ev({ value: 0.5 }));
    expect(r.changes.map((c) => c.paramId)).toEqual(['a', 'b']);
    expect(r.changes.map((c) => c.value)).toEqual([5, 50]);
  });
});

describe('held controls, releaseAll, panic (T58)', () => {
  it('momentary controls are held until up; releaseAll frees them', () => {
    const st = createLearnStore({ mappings: [cc20] });
    st.getState().actions.arm({ kind: 'param', id: 'filter.cutoff' });
    st.getState().actions.dispatch(ev({ phase: 'down' }));
    expect(Object.keys(st.getState().held)).toHaveLength(1);
    const { released } = st.getState().actions.releaseAll('disconnect');
    expect(released).toHaveLength(1);
    expect(Object.keys(st.getState().held)).toHaveLength(0);
    expect(st.getState().armedTarget).toBeNull();
  });

  it('focus-loss disarms + drops learn capture + releases held (all-notes-off)', () => {
    const st = createLearnStore({ mappings: [cc20] });
    st.getState().actions.arm({ kind: 'param', id: 'filter.cutoff' });
    st.getState().actions.beginLearn({ kind: 'param', id: 'x' });
    st.getState().actions.dispatch(ev({ phase: 'down' }));
    st.getState().actions.dispatch(
      ev({ control: { kind: 'cc', channel: 0, id: '30' }, phase: 'down' }),
    );
    const { released } = st.getState().actions.releaseAll('focus-loss');
    expect(released).toHaveLength(2);
    expect(st.getState().held).toEqual({});
    expect(st.getState().armedTarget).toBeNull();
    expect(st.getState().phase).toBe('idle');
    expect(st.getState().learnTarget).toBeNull();
    // Post-release: controls do nothing until the user re-arms.
    const r = st.getState().actions.dispatch(ev({ value: 1 }));
    expect(r.ignored).toBe('no-arm');
  });

  it('panic has priority — idempotent, releases held, disarms, blocks until re-armed', () => {
    const st = createLearnStore({ mappings: [cc20] });
    st.getState().actions.arm({ kind: 'param', id: 'filter.cutoff' });
    st.getState().actions.dispatch(ev({ phase: 'down' }));
    const { released } = st.getState().actions.panic();
    expect(released).toHaveLength(1);
    expect(st.getState().panicCount).toBe(1);
    st.getState().actions.panic(); // safe to hit twice
    expect(st.getState().panicCount).toBe(2);
    const r = st.getState().actions.dispatch(ev({ value: 0.9 }));
    expect(r.changes).toHaveLength(0);
    expect(r.ignored).toBe('no-arm');
    // Explicit re-arm resumes normal control.
    st.getState().actions.arm({ kind: 'param', id: 'filter.cutoff' });
    const r2 = st.getState().actions.dispatch(ev({ value: 0.9 }));
    expect(r2.changes).toHaveLength(1);
  });

  it('up events never write parameters', () => {
    const st = createLearnStore({ mappings: [cc20] });
    st.getState().actions.arm({ kind: 'param', id: 'filter.cutoff' });
    st.getState().actions.dispatch(ev({ phase: 'down' }));
    const r = st.getState().actions.dispatch(ev({ phase: 'up', value: 0 }));
    expect(r.changes).toHaveLength(0);
    expect(Object.keys(st.getState().held)).toHaveLength(0);
  });
});
