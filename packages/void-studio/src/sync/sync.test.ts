// T94 evidence (view-state side): single-master conflicts surface,
// drift health derivations, dead-sync liveness.

import { describe, expect, it } from 'vitest';
import { createSyncStore, SYNC_LOST_SILENCE_MS } from './store';
import type { SyncSourceDescriptor } from './types';

const src = (id: string, kind: SyncSourceDescriptor['kind'] = 'midiClockIn'): SyncSourceDescriptor => ({
  sourceId: id,
  kind,
  label: id,
});

describe('master arbitration view (T94)', () => {
  it('a competing claim is surfaced as a conflict, never applied', () => {
    const s = createSyncStore();
    s.getState().declareSource(src('internal', 'internal'));
    s.getState().declareSource(src('mtc-a', 'mtcIn'));
    s.getState().reportMasterClaimed('internal', 0);
    s.getState().reportConflict('internal', 'mtc-a', 10);
    // View keeps truth: master is still internal; the conflict is open.
    expect(s.getState().masterSourceId).toBe('internal');
    expect(s.getState().conflicts).toHaveLength(1);
    expect(s.getState().conflicts[0].attemptedSourceId).toBe('mtc-a');
  });

  it('switch-at-next-stop resolves on transport stop', () => {
    const s = createSyncStore();
    s.getState().reportMasterClaimed('internal', 0);
    s.getState().reportConflict('internal', 'mtc-a', 10);
    s.getState().reportSwitchScheduled('internal', 'mtc-a', 20);
    expect(s.getState().masterSourceId).toBe('internal');
    expect(s.getState().pendingSwitchTo).toBe('mtc-a');
    s.getState().transportStopped(30);
    expect(s.getState().masterSourceId).toBe('mtc-a');
    expect(s.getState().conflicts).toHaveLength(0);
  });

  it('resolve requests are intents, not local decisions', () => {
    const s = createSyncStore();
    const op = s.getState().requestResolve('internal', 'mtc-a', 'keep-held');
    expect(op.op).toBe('sync-request-resolve');
  });
});

describe('sync health view (T94)', () => {
  it('no master → no-master; drifting report → drifting; silence → lost', () => {
    const s = createSyncStore();
    expect(s.getState().health(0).kind).toBe('no-master');

    s.getState().declareSource(src('clk', 'midiClockIn'));
    s.getState().reportMasterClaimed('clk', 0);
    s.getState().reportTraffic('clk', 0);
    expect(s.getState().health(100).kind).toBe('locked');

    s.getState().reportDrift({
      sourceId: 'clk',
      expected: '1000',
      received: '1100',
      errorPpm: 100_000,
      windowSeconds: 10,
      withinTolerance: false,
      jumpDetected: false,
      atMs: 100,
    });
    expect(s.getState().health(200).kind).toBe('drifting');

    // Silence longer than the lost threshold → lost (sync dead is a
    // visible state, not an assumed lock).
    expect(s.getState().health(SYNC_LOST_SILENCE_MS + 1).kind).toBe('lost');
  });
});
