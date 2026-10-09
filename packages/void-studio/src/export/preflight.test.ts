import { describe, expect, it } from 'vitest';
import { runPreflight } from './preflight';

const green = {
  engineAttached: true,
  trackCount: 3,
  busCount: 1,
  assetIds: ['a1', 'a2'],
  clipAssetRefs: ['a1', 'a2'],
  failedDevices: [],
  checkpointId: 'ckpt-1',
  rangeNonEmpty: true,
};

describe('runPreflight', () => {
  it('passes on a healthy project', () => {
    const r = runPreflight(green);
    expect(r.ready).toBe(true);
    expect(r.blockers).toHaveLength(0);
    expect(r.checks.every((c) => c.status === 'pass')).toBe(true);
  });

  it('blocks everything when the engine is detached (UI-T29)', () => {
    const r = runPreflight({ ...green, engineAttached: false });
    expect(r.ready).toBe(false);
    expect(r.blockers.map((b) => b.id)).toEqual(['engine']);
    // Content checks must not fake a pass while detached.
    expect(r.checks.find((c) => c.id === 'assets')?.status).toBe('unavailable');
  });

  it('blocks on missing referenced assets (UI-T29)', () => {
    const r = runPreflight({ ...green, clipAssetRefs: ['a1', 'gone-1', 'gone-2'] });
    expect(r.ready).toBe(false);
    const assets = r.checks.find((c) => c.id === 'assets');
    expect(assets?.status).toBe('fail');
    expect(assets?.blocking).toBe(true);
    expect(assets?.detail).toContain('gone-1');
  });

  it('blocks on failed/disabled devices (UI-T29)', () => {
    const r = runPreflight({
      ...green,
      failedDevices: [{ instanceId: 'p1', name: 'Comp', reason: 'quarantined' }],
    });
    expect(r.ready).toBe(false);
    expect(r.checks.find((c) => c.id === 'plugins')?.status).toBe('fail');
  });

  it('blocks on empty project and empty range', () => {
    const r = runPreflight({ ...green, trackCount: 0, rangeNonEmpty: false });
    expect(r.ready).toBe(false);
    expect(r.blockers.map((b) => b.id).sort()).toEqual(['content', 'output']);
  });

  it('blocks when no durable checkpoint exists', () => {
    const r = runPreflight({ ...green, checkpointId: undefined });
    expect(r.ready).toBe(false);
    expect(r.checks.find((c) => c.id === 'checkpoint')?.blocking).toBe(true);
  });

  it('warns (not blocks) on an in-range-empty output guess', () => {
    const r = runPreflight({ ...green, clipsInRange: 0 });
    const out = r.checks.find((c) => c.id === 'output');
    expect(out?.status).toBe('warn');
    expect(r.ready).toBe(true);
  });
});
