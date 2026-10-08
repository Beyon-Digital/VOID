import { describe, expect, it } from 'vitest';
import {
  attachVisualMediaOp,
  bindLayerAnchorOp,
  createVisualsStore,
  layerFromSnapshot,
  setLayerTransformOp,
  setOutputRouteOp,
  setTransitionOp,
  setVisualAnchorOp,
  takeTransitionOp,
  transitionFromSnapshot,
  visualRedoOp,
  visualUndoOp,
} from './index';
import { assertViewStateOnly } from '../store';
import { createStudioStore } from '../store';

describe('visual ops builders', () => {
  const sha = 'ab'.repeat(32);

  it('attach media enforces immutable sha + safe rel_path', () => {
    expect(() => attachVisualMediaOp('l', 'a', 'zz', 'image', 'x.png', 0)).toThrow(/sha256/);
    expect(() => attachVisualMediaOp('l', 'a', sha, 'image', '../escape.png', 0)).toThrow(/rel_path/);
    expect(() => attachVisualMediaOp('l', 'a', sha, 'image', '/abs.png', 0)).toThrow(/rel_path/);
    const op = attachVisualMediaOp('l', 'a', sha.toUpperCase(), 'image', 'media/x.png', 4_800_000);
    expect(op).toEqual({
      AttachVisualMediaOp: {
        layer_id: 'l',
        asset_id: 'a',
        sha256: sha,
        media_kind: 'image',
        rel_path: 'media/x.png',
        duration_ticks: '4800000', // i64 as decimal string
      },
    });
  });

  it('transform rejects non-finite and out-of-range opacity', () => {
    const base = { x: 0, y: 0, scale_x: 1, scale_y: 1, rotation_rad: 0, opacity: 1 };
    expect(() => setLayerTransformOp('l', { ...base, x: NaN })).toThrow(/finite/);
    expect(() => setLayerTransformOp('l', { ...base, opacity: 1.5 })).toThrow(/opacity/);
    expect(setLayerTransformOp('l', { ...base, opacity: 0.25 })).toEqual({
      SetLayerTransformOp: { layer_id: 'l', transform: { ...base, opacity: 0.25 } },
    });
  });

  it('output route enforces rational fps + dim bounds', () => {
    expect(() => setOutputRouteOp('program', 'offscreen', '', 0, 0, 30, 1, true)).toThrow(/dims/);
    expect(() => setOutputRouteOp('program', 'offscreen', '', 1920, 1080, 0, 0, true)).toThrow(/rational/);
    const op = setOutputRouteOp('program', 'offscreen', '', 1920, 1080, 30_000, 1_001, true);
    expect(op).toEqual({
      SetOutputRouteOp: {
        channel: 'program',
        target: 'offscreen',
        display_id: '',
        width: 1920,
        height: 1080,
        fps_num: 30000,
        fps_den: 1001,
        readback: true,
      },
    });
  });

  it('anchor + transition + undo payload shapes match fbs rev0', () => {
    expect(setVisualAnchorOp('a1', 'beat', 1_920_000)).toEqual({
      SetVisualAnchorOp: {
        anchor_id: 'a1',
        kind: 'beat',
        position_ticks: '1920000',
        position_sample: '-1',
        timecode_ns: '-1',
      },
    });
    expect(bindLayerAnchorOp('l', 'a1', '')).toEqual({
      BindLayerAnchorOp: { layer_id: 'l', in_anchor_id: 'a1', out_anchor_id: '' },
    });
    expect(setTransitionOp('program', 'wipe', 960_000, 'next_bar', 0.785)).toEqual({
      SetTransitionOp: {
        channel: 'program',
        kind: 'wipe',
        duration_ticks: '960000',
        quantize: 'next_bar',
        wipe_angle: 0.785,
      },
    });
    expect(takeTransitionOp('program')).toEqual({ TakeTransitionOp: { channel: 'program' } });
    expect(visualUndoOp('mix-1')).toEqual({ VisualUndoOp: { transaction_id: 'mix-1' } });
    expect(visualRedoOp('mix-1')).toEqual({ VisualRedoOp: { transaction_id: 'mix-1' } });
  });
});

describe('visuals view-state store', () => {
  it('projects snapshot layers into per-channel order', () => {
    const store = createVisualsStore();
    const l1 = layerFromSnapshot({
      id: 'l1', kind: 'GENERATOR', channel: 'PROGRAM', index: 0, name: 'bars',
      transform: { opacity: 0.9 },
    })!;
    const l2 = layerFromSnapshot({
      id: 'l2', kind: 'IMAGE', channel: 'PREVIEW', index: 0, name: 'img', asset_id: 'a', sha256: 'cd'.repeat(32),
    })!;
    expect(l1.kind).toBe('generator');
    expect(l1.opacity).toBe(0.9);
    store.getState().actions.applySnapshot({ layers: [l1, l2], anchors: [], transitions: [], outputs: [] });
    const st = store.getState();
    expect(st.order.program).toEqual(['l1']);
    expect(st.order.preview).toEqual(['l2']);
    expect(st.layers.l2.mediaSha256).toBe('cd'.repeat(32));
  });

  it('tracks frame meta + drop counters without pixel buffers', () => {
    const store = createVisualsStore();
    store.getState().actions.noteFrame({
      channel: 'program',
      clockSequence: 9,
      positionTicks: 960_000,
      timelineSample: 24_000,
      deviceSampleCounter: 48_000,
      renderUs: 1_200,
      frameSha256: 'ef'.repeat(32),
    });
    store.getState().actions.noteCounters({ droppedClocks: 3, producedFrames: 9 });
    const st = store.getState();
    expect(st.lastFrame.program?.positionTicks).toBe(960_000);
    expect(st.dropCounters.droppedClocks).toBe(3);
    expect(st.dropCounters.producedFrames).toBe(9);
  });

  it('transition snapshot parses armed/in-flight', () => {
    const t = transitionFromSnapshot('program', {
      armed: true, in_flight: true, kind: 'FADE', duration_ticks: 480_000, quantize: 'NEXT_BEAT', progress: 0.5,
    })!;
    expect(t.kind).toBe('fade');
    expect(t.quantize).toBe('next_beat');
    expect(t.inFlight).toBe(true);
  });

  it('visual state never enters the shared studio store', () => {
    // assertViewStateOnly must still pass — the visuals store is a
    // separate store; nothing bleeds into studioStore's top keys.
    const studio = createStudioStore();
    expect(() => assertViewStateOnly(studio.getState())).not.toThrow();
  });
});
