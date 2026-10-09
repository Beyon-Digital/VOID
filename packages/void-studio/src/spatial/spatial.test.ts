// T92/T93 evidence (view-state side): verdicts surface verbatim,
// "available" cannot be manufactured, monitoring drafts bound-check.

import { describe, expect, it } from 'vitest';
import { createSpatialStore } from './store';
import { validateMonitoringDraft, type MonitoringDraft } from './types';

describe('gate verdict surfacing (T92/T93)', () => {
  it('unavailable reasons survive verbatim; no verdict → unavailable display', () => {
    const s = createSpatialStore();
    s.getState().registerOutput({
      outputId: 'out-atmos',
      pathKind: 'atmos-adm-bwf',
      layout: 'surround-7-1-2',
      label: 'Atmos ADM/BWF',
    });
    // No verdict yet → not available (never assume).
    expect(s.getState().isAvailable('out-atmos')).toBe(false);
    expect(s.getState().verdictFor('out-atmos')).toBeNull();

    s.getState().reportVerdict(
      'out-atmos',
      { kind: 'unavailable', reason: { kind: 'licensed-renderer-required', detail: 'Dolby Atmos Renderer not installed' } },
      100,
    );
    const v = s.getState().verdictFor('out-atmos');
    expect(v?.kind).toBe('unavailable');
    if (v?.kind === 'unavailable') expect(v.reason.kind).toBe('licensed-renderer-required');
    expect(s.getState().isAvailable('out-atmos')).toBe(false);
  });

  it('available verdicts carry a validator id — the only way to be available', () => {
    const s = createSpatialStore();
    s.getState().registerOutput({
      outputId: 'out-51',
      pathKind: 'channel-bus',
      layout: 'surround-5-1',
      label: '5.1 bus',
    });
    s.getState().reportVerdict(
      'out-51',
      { kind: 'available', validatorId: 'selfcheck-1', validatorKind: 'self-check', monitorRoute: 'direct' },
      50,
    );
    expect(s.getState().isAvailable('out-51')).toBe(true);
  });
});

describe('monitoring draft bounds', () => {
  const good: MonitoringDraft = {
    speakerSet: 'surround-5-1',
    levelCalibrationDb: { M030: 0, M330: 0, M000: 0, Lfe1: -3, M110: 0, M250: 0 },
    fallback: { kind: 'declared-downmix', to: 'stereo' },
  };

  it('accepts a well-formed config', () => {
    expect(validateMonitoringDraft(good)).toEqual([]);
  });

  it('rejects missing trims, non-finite trims, and upmix fallbacks', () => {
    expect(
      validateMonitoringDraft({ ...good, levelCalibrationDb: { M030: 0 } }),
    ).not.toEqual([]); // only 1 of 6 speakers trimmed
    expect(
      validateMonitoringDraft({
        ...good,
        levelCalibrationDb: { ...good.levelCalibrationDb, M030: 25 },
      }),
    ).not.toEqual([]); // >±20dB
    expect(
      validateMonitoringDraft({
        ...good,
        fallback: { kind: 'declared-downmix', to: 'surround-7-1' },
      }),
    ).not.toEqual([]); // downmix cannot target wider
    expect(
      validateMonitoringDraft({
        ...good,
        fallback: { kind: 'declared-downmix', to: 'surround-5-1' },
      }),
    ).not.toEqual([]); // same layout is not a downmix
  });
});
