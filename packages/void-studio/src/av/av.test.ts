// T87–T89 TS half — spec/codec/job/results/store parity with
// crates/void-av: rational frame math vs the same fixture, codec
// gating, envelope shape, drift parsing, view-state folds.

import { describe, expect, it } from 'vitest';
import {
  avDurationSeconds,
  avFramePlan,
  frameIndexAtSample,
  frameStartSample,
  framesCoveringSamples,
  NTSC_30,
  placeCue,
  samplesForFrames,
  validateAvExportSpec,
  AvSpecError,
  type AvExportSpecDto,
} from './spec';
import { avCodecGate, selectableAvCodecs, AV_CODEC_MATRIX } from './codec';
import { avExportJobEnvelope } from './job';
import { avAudioDriftSamples, parseAvExportResult, avExportListRequest } from './results';
import { createAvExportPanelStore, avSelectableCodecs as selCodecs } from './store';

const JID = '11111111-1111-4111-8111-111111111111';
const PID = '22222222-2222-4222-8222-222222222222';
const CID = '33333333-3333-4333-8333-333333333333';
const SHA = 'a'.repeat(64);

function spec(partial: Partial<AvExportSpecDto> = {}): AvExportSpecDto {
  return {
    jobId: JID,
    projectId: PID,
    checkpointId: CID,
    sourceRevision: '9',
    sampleRate: 48000,
    channels: 'stereo',
    frameRateNum: '30000',
    frameRateDen: '1001',
    rounding: 'nearest_ties_away',
    rangeSamples: '48000',
    width: 160,
    height: 120,
    codecId: 'ffv1_flac_mkv',
    inputs: [
      { kind: 'checkpoint_file', rel_path: 'video.raw', sha256: SHA, role: 'video' },
      { kind: 'asset_blob', sha256: SHA, role: 'audio' },
    ],
    cues: [{ name: 'boundary', atSamples: '24024' }],
    tail: { mode: 'none' },
    timeoutMs: '30000',
    maxOutputBytes: '10485760',
    outputName: 'viz-v1',
    ...partial,
  };
}

describe('T87 rational frame math (TS twin of void_av::rate)', () => {
  it('matches the crate fixture: 48000*1001/30000 → 1601.6', () => {
    const cases: Array<[bigint, bigint]> = [
      [0n, 0n], [1n, 1601n], [2n, 3203n], [5n, 8008n],
      [10n, 16016n], [15n, 24024n], [30n, 48048n],
      [60n, 96096n], [300n, 480480n], [1799n, 2881278n],
    ];
    for (const [f, s] of cases) {
      expect(frameStartSample(NTSC_30, f, 48000)).toBe(s);
      expect(frameIndexAtSample(NTSC_30, s, 48000)).toBe(f);
    }
    // Fractional-boundary starts map to the frame they open.
    expect(frameIndexAtSample(NTSC_30, 1601n, 48000)).toBe(1n);
    expect(frameIndexAtSample(NTSC_30, 1600n, 48000)).toBe(0n);
  });
  it('one minute of audio = 1799 frames, never 1800', () => {
    expect(framesCoveringSamples(NTSC_30, 2_880_000n, 48000)).toBe(1799n);
    expect(samplesForFrames(NTSC_30, 1799n, 48000, 'floor')).toBe(2881278n);
  });
  it('cue at frame boundary places offset 0; mid-frame inside tolerance', () => {
    expect(placeCue(NTSC_30, 24024n, 48000)).toMatchObject({ frameIndex: '15', offsetSamples: '0', withinOneFrame: true });
    const mid = placeCue(NTSC_30, 25000n, 48000);
    expect(mid.frameIndex).toBe('15');
    expect(BigInt(mid.offsetSamples)).toBeLessThanOrEqual(1602n);
    expect(mid.withinOneFrame).toBe(true);
  });
  it('duration_seconds is the audio program total, exact microseconds', () => {
    expect(avDurationSeconds(48000n, 48000)).toBe('1.000000');
    expect(avDurationSeconds(48001n, 48000)).toBe('1.000021');
  });
  it('frame plan mirrors the crate: 48000 samples → 30 NTSC frames', () => {
    const plan = avFramePlan(spec());
    expect(plan.videoTotalFrames).toBe('30');
    expect(plan.audioTotalSamples).toBe('48000');
    expect(plan.cues[0].withinOneFrame).toBe(true);
  });
  it('Frames tail extends video only; Samples tail extends audio', () => {
    const byFrames = avFramePlan(spec({ tail: { mode: 'frames', frames: '5' } }));
    expect(byFrames.videoTotalFrames).toBe('35');
    expect(byFrames.audioTotalSamples).toBe('48000');
    const bySamples = avFramePlan(spec({ tail: { mode: 'samples', samples: '4800' } }));
    expect(bySamples.audioTotalSamples).toBe('52800');
    expect(bySamples.videoTotalFrames).toBe('33'); // ceil(52800·30000/48048000)
  });
});

describe('T88 spec validation + codec gating', () => {
  it('rejects traversal/flag-like output names and non-hex shas', () => {
    expect(() => validateAvExportSpec(spec({ outputName: '-i' }))).toThrow(AvSpecError);
    expect(() => validateAvExportSpec(spec({ outputName: 'a;b' }))).toThrow(AvSpecError);
    expect(() =>
      validateAvExportSpec(spec({ inputs: [{ kind: 'checkpoint_file', rel_path: '../x', sha256: SHA, role: 'video' }, { kind: 'lavfi_test', src: 'sine', role: 'audio' }] })),
    ).toThrow(AvSpecError);
    expect(() =>
      validateAvExportSpec(spec({ inputs: [{ kind: 'asset_blob', sha256: 'nope', role: 'audio' }, { kind: 'lavfi_test', src: 'testsrc2', role: 'video' }] })),
    ).toThrow(AvSpecError);
  });
  it('rejects lavfi sources with shell-ish characters', () => {
    expect(() =>
      validateAvExportSpec(spec({ inputs: [
        { kind: 'lavfi_test', src: 'testsrc2; rm -rf /', role: 'video' },
        { kind: 'lavfi_test', src: 'sine', role: 'audio' },
      ] })),
    ).toThrow(AvSpecError);
  });
  it('gate: missing ffmpeg/encoder/unknown id → not selectable (typed reason)', () => {
    expect(avCodecGate('ffv1_flac_mkv', false, () => true).reason).toBe('ffmpeg binary not present');
    expect(avCodecGate('ffv1_flac_mkv', true, (n) => n !== 'flac').reason).toBe('ffmpeg build lacks flac');
    expect(avCodecGate('bogus', true, () => true).reason).toBe("unknown codec 'bogus'");
    expect(avCodecGate('ffv1_flac_mkv', true, () => true).selectable).toBe(true);
  });
  it('matrix: rights documented for every row', () => {
    for (const c of AV_CODEC_MATRIX) {
      expect(c.licenseNote.length).toBeGreaterThan(0);
      expect(['cleared', 'development_only']).toContain(c.rights);
    }
  });
  it('selectable list only contains genuinely encodable rows', () => {
    const sel = selectableAvCodecs(true, (n) => n !== 'libvpx-vp9' && n !== 'libopus' && n !== 'prores_ks' && n !== 'pcm_s24le');
    expect(sel.map((c) => c.id).sort()).toEqual(['ffv1_flac_mkv', 'h264_aac_mp4']);
  });
});

describe('T89 envelope + result parsing + store folds', () => {
  it('envelope carries void-av runtime + av scope token + real inputs only', () => {
    const env = avExportJobEnvelope(spec({ inputs: [...spec().inputs, { kind: 'lavfi_test', src: 'testsrc2', role: 'video' }] }), SHA, 'c'.repeat(64));
    expect(env.runtimeId).toBe('void-av');
    expect(env.kind).toBe('av_export');
    expect(env.outputScopeToken).toBe(`av:${JID}`);
    expect(env.inputs).toEqual([`video.raw:${SHA}`, SHA]);
    expect(env.inputs).not.toContain('testsrc2');
  });
  it('result card parses; drift surfaced, never a bit-identical claim', () => {
    const card = {
      kind: 'AvExportResult', jobId: JID, status: 'succeeded', file: 'viz-v1.mkv',
      sha256: SHA, codecId: 'ffv1_flac_mkv', videoFrames: '30', audioSamples: '48007',
      verify: { videoFramesExpected: '30', videoFramesMeasured: '30', audioSamplesExpected: '48000', audioSamplesMeasured: '48007', fpsMatchesSpec: true },
    };
    const r = parseAvExportResult(card);
    expect(r).not.toBeNull();
    expect(avAudioDriftSamples(r!)).toBe(7n);
    expect(parseAvExportResult({ kind: 'Other' })).toBeNull();
    expect(avExportListRequest(PID, 'req-1').view).toBe('AV_EXPORT_LIST');
  });
  it('store: gate blocks buildSpec when codec unavailable; folds real events', () => {
    const st = createAvExportPanelStore();
    st.getState().actions.setDraft({
      jobId: JID, projectId: PID, checkpointId: CID,
      sourceRevision: '9', sampleRate: 48000, rangeSamples: '48000',
      inputs: spec().inputs,
    });
    expect(st.getState().actions.buildSpec()).toBeNull();
    expect(st.getState().errors[0]).toContain('ffmpeg');
    st.getState().actions.setEncoderAvailability(true, ['ffv1', 'flac']);
    const built = st.getState().actions.buildSpec();
    expect(built?.codecId).toBe('ffv1_flac_mkv');
    expect(selCodecs(st.getState()).map((c) => c.id)).toEqual(['ffv1_flac_mkv']);
    st.getState().actions.onJobEvent({ kind: 'JobEvent', job_id: JID, status: 'running', percent: 40 });
    st.getState().actions.onJobEvent({ kind: 'JobEvent', job_id: JID, status: 'succeeded' });
    expect(st.getState().jobs[JID].status).toBe('succeeded');
    // result cards fold through read items
    st.getState().actions.onReadItems([{ data: { kind: 'AvExportResult', jobId: JID, status: 'succeeded' } } as never]);
    expect(st.getState().results).toHaveLength(1);
  });
});
