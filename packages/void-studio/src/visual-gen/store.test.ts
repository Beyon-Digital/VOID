import { describe, expect, it } from 'vitest';
import { createVisGenStore } from './store';
import { DEFAULT_CAMERA_POLICY_VIEW } from './model';

const PROV = {
  jobId: 'job-1',
  generatorId: 'visfx-fake-worker',
  generatorVersion: '1.0.0',
  modelId: null,
  runtimeId: 'local:visual-gen',
  runtimeSha256: 'a'.repeat(64),
  seed: '42',
  documentSha256: 'b'.repeat(64),
  documentAsset: 'assets/sha256/doc',
};

const RESULT = {
  layers: [
    {
      name: 'gen-0',
      channel: 'preview' as const,
      kind: 'generator' as const,
      preset: 'plasma',
      artifact: null,
      blend: 'normal',
      opacity: 1,
    },
  ],
  actionCount: 0,
  artifacts: [
    {
      filename: 'scene.json',
      sha256: 'c'.repeat(64),
      bytes: 512,
      assetRel: 'assets/sha256/cc.json',
    },
  ],
  provenance: PROV,
};

describe('visual-gen store (W23 view-state)', () => {
  it('request → telemetry → ready → accept lifecycle', () => {
    const s = createVisGenStore();
    s.getState().actions.requestRecord({
      recordId: 'r1',
      jobId: 'j1',
      kind: 'scene',
      description: 'plasma reactive layer',
    });
    expect(s.getState().records.r1.status).toBe('pending');
    expect(s.getState().order).toEqual(['r1']);

    expect(
      s.getState().actions.applyJobEvent('j1', {
        status: 'running',
        percent: 55,
      }),
    ).toBe(true);
    expect(s.getState().records.r1.percent).toBe(55);
    expect(s.getState().records.r1.status).toBe('pending');
    // unknown job → not folded
    expect(
      s.getState().actions.applyJobEvent('j-other', { status: 'running' }),
    ).toBe(false);

    s.getState().actions.setReady('r1', RESULT);
    expect(s.getState().records.r1.status).toBe('ready');
    expect(s.getState().records.r1.provenance?.documentSha256).toBe(
      PROV.documentSha256,
    );

    s.getState().actions.accept('r1', 'tx-9');
    const rec = s.getState().records.r1;
    expect(rec.status).toBe('accepted');
    expect(rec.transactionId).toBe('tx-9');
  });

  it('reject + stale mark terminal transitions', () => {
    const s = createVisGenStore();
    s.getState().actions.requestRecord({
      recordId: 'r1',
      jobId: 'j1',
      kind: 'scene',
    });
    s.getState().actions.reject('r1', 'dismissed');
    expect(s.getState().records.r1.status).toBe('rejected');
    // terminal: late telemetry/results don't resurrect it
    s.getState().actions.applyJobEvent('j1', { status: 'running' });
    expect(s.getState().records.r1.status).toBe('rejected');
    s.getState().actions.setReady('r1', RESULT);
    expect(s.getState().records.r1.status).toBe('rejected');

    s.getState().actions.requestRecord({
      recordId: 'r2',
      jobId: 'j2',
      kind: 'scene',
    });
    s.getState().actions.markStale('r2', 'context_changed');
    expect(s.getState().records.r2.status).toBe('stale');
    expect(s.getState().records.r2.staleCause).toBe('context_changed');
  });

  it('compare solos an artifact without mutating others', () => {
    const s = createVisGenStore();
    s.getState().actions.requestRecord({
      recordId: 'r1',
      jobId: 'j1',
      kind: 'scene',
    });
    s.getState().actions.setReady('r1', RESULT);
    s.getState().actions.setCompare('r1', RESULT.artifacts[0].sha256);
    expect(s.getState().records.r1.comparingSha256).toBe(
      RESULT.artifacts[0].sha256,
    );
    s.getState().actions.setCompare('r1', null);
    expect(s.getState().records.r1.comparingSha256).toBeNull();
  });

  it('camera consent gate: deny/revoke disarms + releases', () => {
    const s = createVisGenStore();
    const a = s.getState().actions;
    expect(s.getState().camera.consent).toBe('not_asked');
    a.setConsent('granted');
    expect(s.getState().camera.consent).toBe('granted');
    a.setCalibration({
      calibratedAt: '2026-10-08T00:00:00Z',
      trackerModelId: 'hand-v1',
      trackerModelSha256: 'd'.repeat(64),
      measuredConfidence: 0.9,
      measuredLatencyMs: 33,
      bounds: [0.1, 0.1, 0.9, 0.9],
    });
    expect(s.getState().camera.session).toBe('armed');
    a.setClutch(true);
    a.setHeldCount(3);
    a.noteDroppedFrames(7);
    a.setConsent('revoked');
    const cam = s.getState().camera;
    expect(cam.session).toBe('idle');
    expect(cam.clutchDown).toBe(false);
    expect(cam.heldCount).toBe(0);
    expect(cam.droppedFrames).toBe(7); // evidence counter kept
  });

  it('release-all clears held controls and records the cause', () => {
    const s = createVisGenStore();
    const a = s.getState().actions;
    a.setHeldCount(5);
    a.setClutch(true);
    a.noteReleaseAll('tracking_lost');
    const cam = s.getState().camera;
    expect(cam.heldCount).toBe(0);
    expect(cam.clutchDown).toBe(false);
    expect(cam.lastReleaseCause).toBe('tracking_lost');
  });

  it('default camera policy is max-privacy (T86)', () => {
    expect(DEFAULT_CAMERA_POLICY_VIEW.asserts.noRawRetention).toBe(true);
    expect(DEFAULT_CAMERA_POLICY_VIEW.asserts.noNetworkUpload).toBe(true);
    expect(DEFAULT_CAMERA_POLICY_VIEW.asserts.dropOccludedFrames).toBe(true);
    expect(DEFAULT_CAMERA_POLICY_VIEW.clutchRequired).toBe(true);
  });

  it('reset clears records and camera', () => {
    const s = createVisGenStore();
    const a = s.getState().actions;
    a.requestRecord({ recordId: 'r1', jobId: 'j1', kind: 'scene' });
    a.setConsent('granted');
    a.reset();
    expect(Object.keys(s.getState().records)).toHaveLength(0);
    expect(s.getState().camera.consent).toBe('not_asked');
  });
});
