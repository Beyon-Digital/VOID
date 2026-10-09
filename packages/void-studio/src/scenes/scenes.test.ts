import { describe, expect, it } from 'vitest';
import {
  captureToArrangementOps,
  createSceneStore,
  launchScene,
  makeCapture,
  makeGrid,
  makeLaunchState,
  patternToClipOps,
  quantizeLaunchAt,
  recordEvent,
  sceneTransportOps,
  setSlot,
  slotKey,
  stopAll,
  type Scene,
} from './index';
import { makeRow, setStep, type StepPattern } from '../patterns/stepPattern';
import type { ClipView } from '../timeline/geometry';

const BAR = '3840000';
let seq = 0;
const mint = () => `m${++seq}`;

const sceneA: Scene = {
  sceneId: 'scA', name: 'A',
  region: { startTicks: '0', lengthTicks: BAR },
};
const sceneB: Scene = { sceneId: 'scB', name: 'B' };

function gridWithSlots() {
  let g = makeGrid([sceneA, sceneB], ['trk1', 'trk2']);
  g = setSlot(g, {
    slotId: 's-a1', sceneId: 'scA', trackId: 'trk1',
    content: { kind: 'clip', clipId: 'clip-1' },
  });
  g = setSlot(g, {
    slotId: 's-b1', sceneId: 'scB', trackId: 'trk1',
    content: { kind: 'clip', clipId: 'clip-2' },
  });
  g = setSlot(g, {
    slotId: 's-b2', sceneId: 'scB', trackId: 'trk2',
    content: { kind: 'empty' },
  });
  return g;
}

function midiClip(partial: Partial<ClipView> & { clipId: string }): ClipView {
  return {
    trackId: 'trk1', kind: 'MIDI',
    startTicks: '0', lengthTicks: BAR, offsetTicks: '0',
    ...partial,
  } as ClipView;
}

describe('scene grid + quantized launch (PAT-02/03)', () => {
  it('quantizeLaunchAt lands on the next bar/beat/custom boundary', () => {
    expect(quantizeLaunchAt('1000000', 'bar')).toBe(BAR);
    expect(quantizeLaunchAt(BAR, 'bar')).toBe(BAR); // on boundary fires now
    expect(quantizeLaunchAt('1000000', 'beat')).toBe('1920000');
    expect(quantizeLaunchAt('100', { kind: 'custom', ticks: '400' })).toBe('400');
    expect(quantizeLaunchAt('77', 'immediate')).toBe('77');
    // 3/4 meter: a bar is 2880000
    expect(
      quantizeLaunchAt('1000000', 'bar', [{ atTicks: '0', numerator: 3, denominator: 4 }]),
    ).toBe('2880000');
  });

  it('launchScene marks slots pending at the quantized boundary and stops the old scene', () => {
    const g = gridWithSlots();
    let st = makeLaunchState();
    const r1 = launchScene(g, st, 'scB', '100', 'immediate');
    st = r1.state;
    expect(st.playingSceneId).toBe('scB');
    expect(st.slots['s-b1']?.phase).toBe('pending');
    // settle at the boundary
    st = { ...st, slots: { ...st.slots } };
    const settled = launchScene(g, st, 'scA', '50', 'immediate');
    expect(settled.state.playingSceneId).toBe('scA');
    expect(settled.state.slots['s-a1']?.phase).toBe('pending');
    expect(settled.state.slots['s-b1']?.phase).toBe('stopping');
  });

  it('stopAll queues every playing/pending slot to stop at the boundary', () => {
    const g = gridWithSlots();
    let st = launchScene(g, makeLaunchState(), 'scA', '0', 'immediate').state;
    // force playing
    st = {
      ...st,
      slots: { 's-a1': { phase: 'playing', sinceTicks: '0' } },
      playingSceneId: 'scA',
    };
    const r = stopAll(st, '100', 'bar');
    expect(r.atTicks).toBe(BAR);
    expect(r.state.slots['s-a1']?.phase).toBe('stopping');
    expect(r.state.playingSceneId).toBeNull();
  });

  it('sceneTransportOps maps a bound scene to SEEK+SET_CYCLE', () => {
    const ops = sceneTransportOps(sceneA);
    expect(ops).toEqual([
      { op: 'SEEK', position_ticks: '0' },
      { op: 'SET_CYCLE', cycle_start_ticks: '0', cycle_end_ticks: BAR },
    ]);
    expect(sceneTransportOps(sceneB)).toEqual([]);
  });

  it('store launch/settle/stopAll drive slot phases', () => {
    const store = createSceneStore({ scenes: [sceneA], trackIds: ['trk1'] });
    store.getState().actions.setSlot({
      slotId: 'sl', sceneId: 'scA', trackId: 'trk1',
      content: { kind: 'clip', clipId: 'c' },
    });
    const at = store.getState().actions.launch('scA', '10', 'immediate');
    expect(at).toBe('10');
    store.getState().actions.settle('10');
    expect(store.getState().launch.slots['sl']?.phase).toBe('playing');
    store.getState().actions.stopAll('20', 'immediate');
    expect(store.getState().launch.slots['sl']?.phase).toBe('stopping');
  });
});

describe('capture-to-arrangement + pattern variation (PAT-04/05)', () => {
  const pattern: StepPattern = (() => {
    const row = makeRow('r1', 36, 4, 'kick');
    const p: StepPattern = {
      patternId: 'pat-1', name: 'drums', stepTicks: '240000', seed: 's1', rows: [row],
    };
    return setStep(p, 'r1', 0, { on: true, velocity: 100 });
  })();

  it('captureToArrangementOps materializes clip cells and pattern cells with notes', () => {
    const rec = recordEvent(makeCapture('cap1', '0'), {
      sceneId: 'scA',
      atTicks: BAR,
      slots: [
        { trackId: 'trk1', content: { kind: 'clip', clipId: 'clip-src' } },
        { trackId: 'trk2', content: { kind: 'pattern', patternId: 'pat-1' } },
        { trackId: 'trk3', content: { kind: 'empty' } },
      ],
    });
    const plan = captureToArrangementOps(rec, {
      resolveClip: (id) =>
        id === 'clip-src' ? midiClip({ clipId: 'clip-src' }) : undefined,
      resolvePattern: (id) => (id === 'pat-1' ? pattern : undefined),
      patternClipLengthTicks: BAR,
    }, mint);
    const midiInserts = plan.ops.filter((o) => 'InsertMidiClipOp' in o);
    const noteInserts = plan.ops.filter((o) => 'InsertNoteOp' in o);
    expect(midiInserts).toHaveLength(2); // clip cell + pattern cell
    expect(noteInserts.length).toBeGreaterThan(0);
    expect(plan.skipped).toEqual([{ trackId: 'trk3', kind: 'empty' }]);
    expect(Object.keys(plan.noteIdsByClip)).toHaveLength(1);
    // everything under one transaction
    expect(plan.transactionId).toBeTruthy();
  });

  it('skips cells whose media is not materialized (honest — no invented audio)', () => {
    const rec = recordEvent(makeCapture('cap', '0'), {
      sceneId: 'scA', atTicks: '0',
      slots: [{ trackId: 't', content: { kind: 'clip', clipId: 'missing' } }],
    });
    const plan = captureToArrangementOps(rec, { resolveClip: () => undefined }, mint);
    expect(plan.ops).toHaveLength(0);
    expect(plan.skipped).toEqual([{ trackId: 't', kind: 'clip' }]);
  });

  it('patternToClipOps emits InsertMidiClipOp + InsertNoteOps in one transaction', () => {
    const plan = patternToClipOps('trk1', pattern, '0', BAR, mint);
    const kinds = plan.ops.map((o) => Object.keys(o)[0]);
    expect(kinds[0]).toBe('InsertMidiClipOp');
    expect(kinds.slice(1).every((k) => k === 'InsertNoteOp')).toBe(true);
    expect(plan.noteIds.length).toBe(plan.ops.length - 1);
  });
});

describe('slot keying', () => {
  it('slotKey is scene:track', () => {
    expect(slotKey('s', 't')).toBe('s:t');
  });
});
