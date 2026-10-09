// S09 model tests — output state machine (UI-T23), failure honesty
// (UI-T24), layer projection, asset filtering, and clock honesty.

import { describe, expect, it } from 'vitest';
import type { VisualFrameMeta, VisualLayerView } from 'void-studio';
import type { ClockSnapshot } from 'void-client';
import {
  assetBlobsByRole,
  clockLine,
  deriveOutputView,
  dropCountersLine,
  frameMetaLine,
  framesDroppedWhileArmed,
  layerWindowTicks,
  loopWindowTicks,
  mediaTypeIsAudio,
  mediaTypeIsVisual,
  orderedChannelLayers,
  outputStateLabel,
  timelineSpanTicks,
  visualAssetRows,
  INITIAL_OUTPUT_INTENT,
} from './model';
import { createOutputIntentStore } from './stores';

const layer = (over: Partial<VisualLayerView>): VisualLayerView => ({
  layerId: 'l1',
  kind: 'generator',
  name: 'Orbit',
  channel: 'preview',
  index: 0,
  visible: true,
  blend: 'normal',
  opacity: 1,
  x: 0,
  y: 0,
  scaleX: 1,
  scaleY: 1,
  rotationRad: 0,
  inTicks: -1,
  outTicks: -1,
  inAnchorId: null,
  outAnchorId: null,
  assetId: null,
  mediaSha256: null,
  generatorPreset: 'plasma',
  ...over,
});

const frame = (over: Partial<VisualFrameMeta> = {}): VisualFrameMeta => ({
  channel: 'program',
  clockSequence: 3,
  positionTicks: 960000,
  timelineSample: 48000,
  deviceSampleCounter: 96000,
  renderUs: 4120,
  frameSha256: 'abcdef0123456789'.repeat(4),
  ...over,
});

describe('deriveOutputView — UI-T23 state machine', () => {
  it('starts disarmed (previewOnly) with no target', () => {
    const v = deriveOutputView({ attached: true, intent: INITIAL_OUTPUT_INTENT, lastProgramFrame: null });
    expect(v.state).toBe('previewOnly');
    expect(v.framesFlowing).toBe(false);
  });

  it('unavailable wins over everything when the engine is detached', () => {
    const v = deriveOutputView({
      attached: false,
      intent: { selectedTarget: 'offscreen', armed: true, blackout: true, note: null },
      lastProgramFrame: frame(),
    });
    expect(v.state).toBe('unavailable');
  });

  it('target selection alone never arms', () => {
    const v = deriveOutputView({
      attached: true,
      intent: { ...INITIAL_OUTPUT_INTENT, selectedTarget: 'offscreen' },
      lastProgramFrame: null,
    });
    expect(v.state).toBe('displaySelected');
  });

  it('armed without frames is armed, not live', () => {
    const v = deriveOutputView({
      attached: true,
      intent: { selectedTarget: 'offscreen', armed: true, blackout: false, note: null },
      lastProgramFrame: null,
    });
    expect(v.state).toBe('armed');
    expect(v.reason).toContain('armed');
    expect(v.framesFlowing).toBe(false);
  });

  it('armed + real program frame = live', () => {
    const v = deriveOutputView({
      attached: true,
      intent: { selectedTarget: 'offscreen', armed: true, blackout: false, note: null },
      lastProgramFrame: frame(),
    });
    expect(v.state).toBe('live');
    expect(v.framesFlowing).toBe(true);
  });

  it('blackout overrides live output', () => {
    const v = deriveOutputView({
      attached: true,
      intent: { selectedTarget: 'offscreen', armed: true, blackout: true, note: null },
      lastProgramFrame: frame(),
    });
    expect(v.state).toBe('blackout');
    expect(v.framesFlowing).toBe(false);
  });
});

describe('output intent store — UI-T23 transitions', () => {
  it('changing the target drops the arm (re-arming is explicit)', () => {
    const st = createOutputIntentStore();
    st.getState().actions.selectTarget('offscreen');
    st.getState().actions.setArmed(true);
    expect(st.getState().armed).toBe(true);
    st.getState().actions.selectTarget('window');
    expect(st.getState().armed).toBe(false);
    expect(st.getState().selectedTarget).toBe('window');
  });

  it('blackout toggles independently of arm state', () => {
    const st = createOutputIntentStore();
    st.getState().actions.setBlackout(true);
    expect(st.getState().blackout).toBe(true);
    st.getState().actions.setBlackout(false);
    expect(st.getState().blackout).toBe(false);
  });
});

describe('layer projection', () => {
  const state = {
    layers: {
      a: layer({ layerId: 'a', index: 1 }),
      b: layer({ layerId: 'b', index: 0, kind: 'video', name: 'Film' }),
    },
    order: { preview: ['b', 'a'], program: [] },
  };

  it('orders by the channel stack order, not index fields', () => {
    const ls = orderedChannelLayers(state, 'preview');
    expect(ls.map((l) => l.layerId)).toEqual(['b', 'a']);
    expect(orderedChannelLayers(state, 'program')).toEqual([]);
  });

  it('untrimmed layers span the default end; trimmed keep their window', () => {
    expect(layerWindowTicks(layer({}), 3840000)).toEqual({ startTicks: 0, endTicks: 3840000 });
    expect(layerWindowTicks(layer({ inTicks: 960000, outTicks: 1920000 }), 3840000)).toEqual({
      startTicks: 960000,
      endTicks: 1920000,
    });
  });

  it('timeline span covers real layer windows with a floor', () => {
    expect(timelineSpanTicks([])).toBe(960000 * 16);
    expect(timelineSpanTicks([layer({ outTicks: 960000 * 32 })])).toBe(960000 * 32);
  });
});

describe('asset filtering', () => {
  const mk = (mediaType: string, state = 'present', sha?: string) => ({
    object_id: `o-${mediaType}`,
    summary_json: JSON.stringify({
      display_name: `n-${mediaType}`,
      media_type: mediaType,
      state,
      sha256: sha ?? 'a'.repeat(64),
    }),
  });

  it('keeps only image/video media types', () => {
    const rows = visualAssetRows([mk('audio'), mk('image/png'), mk('video'), mk('midi')] as never);
    expect(rows.map((r) => r.mediaType)).toEqual(['image/png', 'video']);
  });

  it('splits present blob assets into video/audio roles for AV export', () => {
    const { video, audio } = assetBlobsByRole([
      mk('image'),
      mk('audio/wav'),
      mk('video', 'missing', undefined),
    ] as never);
    expect(video.map((r) => r.mediaType)).toEqual(['image']);
    expect(audio.map((r) => r.mediaType)).toEqual(['audio/wav']);
  });

  it('media-type guards are defensive', () => {
    expect(mediaTypeIsVisual(undefined)).toBe(false);
    expect(mediaTypeIsVisual('IMAGE/JPEG')).toBe(true);
    expect(mediaTypeIsAudio('audio/opus')).toBe(true);
    expect(mediaTypeIsAudio('video')).toBe(false);
  });
});

describe('honesty lines', () => {
  const clock = (over: Partial<ClockSnapshot> = {}): ClockSnapshot => ({
    kind: 'ClockSnapshot',
    project_id: 'p',
    engine_epoch: '1',
    timeline_sample: '48000',
    device_sample_counter: '96000',
    sample_rate: 48000,
    transport: 'PLAYING',
    loop_start_ticks: '0',
    loop_end_ticks: '3840000',
    tempo_map_revision: '0',
    sequence: '7',
    host_clock_ns: '0',
    ...over,
  });

  it('clock line is verbatim telemetry, never a JS clock', () => {
    expect(clockLine(undefined)).toContain('no clock');
    expect(clockLine(clock())).toContain('48000 samples @ 48000 Hz');
    expect(clockLine(clock())).toContain('playing');
  });

  it('loop window comes from real clock ticks only', () => {
    expect(loopWindowTicks(clock())).toEqual({ start: 0, end: 3840000 });
    expect(loopWindowTicks(clock({ loop_end_ticks: '0' }))).toBeNull();
    expect(loopWindowTicks(undefined)).toBeNull();
  });

  it('frame line carries identity + timing, no pixels', () => {
    const line = frameMetaLine(frame());
    expect(line).toContain('abcdef012345');
    expect(line).toContain('4120µs');
    expect(line).toContain('clock seq 3');
  });

  it('drop counters read verbatim and flag UI-T24 pressure', () => {
    const c = { droppedClocks: 0, droppedFrames: 2, skippedRenders: 1, producedFrames: 40 };
    expect(dropCountersLine(c)).toContain('dropped 2');
    expect(framesDroppedWhileArmed(c)).toBe(true);
    expect(framesDroppedWhileArmed({ ...c, droppedFrames: 0, skippedRenders: 0 })).toBe(false);
  });

  it('state labels are the contract vocabulary', () => {
    expect(outputStateLabel('previewOnly')).toBe('OUTPUT DISARMED');
    expect(outputStateLabel('blackout')).toBe('BLACKOUT');
  });
});
