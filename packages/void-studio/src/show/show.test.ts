// T95/T96 evidence: pairing state machine, armed gate on generated
// actions, PANIC idempotence + sync-death independence, mapping
// bounds + revoke-closes-all.

import { describe, expect, it } from 'vitest';
import { createShowStore } from './store';
import type { Cue, MappingChain, OutputPair } from './index';

const cue = (id: string, over: Partial<Cue> = {}): Cue => ({
  cueId: id,
  name: id,
  preWaitMs: 0,
  postWaitMs: 0,
  autoFollow: false,
  rehearse: false,
  actions: [],
  ...over,
});

const pair = (id: string, kind: OutputPair['target']['kind'] = 'dmx'): OutputPair => ({
  pairId: id,
  target: { targetId: id, kind, label: id, address: '1' },
  state: 'paired',
  scope: `show:${kind}`,
});

describe('pairing state machine', () => {
  it('unpaired → paired → armed, disarm-safe, strict steps', () => {
    const s = createShowStore();
    s.getState().pair(pair('lx-1'));
    expect(s.getState().pairs['lx-1'].state).toBe('paired');
    expect(s.getState().arm('lx-1')).toBe(true);
    expect(s.getState().pairs['lx-1'].state).toBe('armed');
    // Armed pair cannot unpair without disarm.
    expect(s.getState().unpair('lx-1')).toBe(false);
    expect(s.getState().disarm('lx-1')).toBe(true);
    expect(s.getState().unpair('lx-1')).toBe(true);
    expect(s.getState().pairs['lx-1']).toBeUndefined();
  });

  it('arm requires paired', () => {
    const s = createShowStore();
    expect(s.getState().arm('ghost')).toBe(false);
  });
});

describe('generated-instruction gate (T95)', () => {
  it('cue actions to unarmed targets are dropped and recorded', () => {
    const s = createShowStore();
    s.getState().pair(pair('dmx-out'));
    // paired but NOT armed
    s.getState().addCue(
      cue('c1', {
        actions: [
          { kind: 'set', targetId: 'dmx-out', value: 1 },
          { kind: 'transport', arg: 'stop' }, // no target → allowed intent
        ],
      }),
    );
    s.getState().selectCue('c1');
    const rec = s.getState().go(0)!;
    expect(rec).not.toBeNull();
    expect(rec.droppedUnarmed).toHaveLength(1);
    expect(rec.droppedUnarmed[0].targetId).toBe('dmx-out');
    expect(rec.intents).toHaveLength(1);
    expect(rec.intents[0].kind).toBe('transport');
  });

  it('armed target receives intents', () => {
    const s = createShowStore();
    s.getState().pair(pair('dmx-out'));
    s.getState().arm('dmx-out');
    s.getState().addCue(cue('c1', { actions: [{ kind: 'set', targetId: 'dmx-out', value: 0.7 }] }));
    s.getState().selectCue('c1');
    const rec = s.getState().go(0)!;
    expect(rec.intents).toHaveLength(1);
    expect(rec.droppedUnarmed).toHaveLength(0);
  });

  it('queued generated actions respect the armed gate on flush', () => {
    const s = createShowStore();
    s.getState().pair(pair('unarmed-pair'));
    s.getState().pair(pair('armed-pair'));
    s.getState().arm('armed-pair');
    s.getState().enqueue({ queueId: 'q1', origin: 'ai', action: { kind: 'set', targetId: 'unarmed-pair', value: 1 } });
    s.getState().enqueue({ queueId: 'q2', origin: 'ai', action: { kind: 'set', targetId: 'armed-pair', value: 1 } });
    const dispatched = s.getState().flushQueue();
    expect(dispatched.map((q) => q.queueId)).toEqual(['q2']);
  });
});

describe('cue list semantics', () => {
  it('preWait defers fire; autoFollow advances past rehearse cues', () => {
    const s = createShowStore();
    s.getState().addCue(cue('a', { preWaitMs: 500 }));
    s.getState().addCue(cue('r', { rehearse: true }));
    s.getState().addCue(cue('b'));
    s.getState().selectCue('a');
    expect(s.getState().go(0)).toBeNull(); // deferred by preWait
    expect(s.getState().runnerPhase).toBe('preWait');
  });

  it('autoFollow skips rehearse markers', () => {
    const s = createShowStore();
    s.getState().addCue(cue('a', { autoFollow: true }));
    s.getState().addCue(cue('r', { rehearse: true }));
    s.getState().addCue(cue('b'));
    s.getState().selectCue('a');
    s.getState().go(0);
    s.getState().tick(100);
    expect(s.getState().selectedCueId).toBe('b');
  });
});

describe('PANIC (T95)', () => {
  it('releases all output state, is idempotent, works when sync is dead', () => {
    const s = createShowStore();
    s.getState().setSyncLiveness('dead'); // PANIC must not consult this
    s.getState().pair(pair('dmx-a'));
    s.getState().pair(pair('osc-b', 'osc'));
    s.getState().arm('dmx-a');
    s.getState().arm('osc-b');
    s.getState().addCue(cue('c1', { actions: [{ kind: 'set', targetId: 'dmx-a', value: 1 }] }));
    s.getState().selectCue('c1');
    s.getState().go(0);
    s.getState().enqueue({ queueId: 'q1', origin: 'timed', action: { kind: 'set', targetId: 'dmx-a', value: 1 } });

    const rec = s.getState().panic(1000);
    expect(rec.panicCount).toBe(1);
    expect(rec.releasedPairs.sort()).toEqual(['dmx-a', 'osc-b']);
    expect(rec.droppedQueued).toEqual(['q1']);
    expect(rec.blackoutIntents.sort()).toEqual(['dmx', 'osc']);
    // No residual armed state.
    const st = s.getState();
    expect(Object.values(st.pairs).every((p) => p.state === 'paired')).toBe(true);
    expect(st.queue).toHaveLength(0);
    expect(st.runnerPhase).toBe('idle');
    expect(st.runningCueId).toBeNull();

    // Idempotent: second panic releases nothing further.
    const rec2 = s.getState().panic(1001);
    expect(rec2.panicCount).toBe(2);
    expect(rec2.releasedPairs).toHaveLength(0);
    expect(rec2.droppedQueued).toHaveLength(0);
  });
});

describe('mapping engine (T96)', () => {
  const chain = (over: Partial<MappingChain> = {}): MappingChain => ({
    chainId: 'ch1',
    sourceId: 'src-1',
    label: 'fader→dmx',
    steps: [
      { kind: 'filter', min: 0, max: 1 },
      { kind: 'scale', curve: 'linear', invert: false, outMin: 0, outMax: 255 },
    ],
    targets: [{ targetId: 'dmx.1.1', kind: 'dmx-channel' }],
    enabled: true,
    ...over,
  });

  it('runs bounded transforms and reports typed drops', () => {
    const s = createShowStore();
    expect(s.getState().upsertChain(chain())).toEqual([]);
    const r = s.getState().dispatchEvent({ sourceId: 'src-1', value: 0.5, atMs: 0 });
    expect(r).toHaveLength(1);
    expect(r[0].ok).toBe(true);
    expect(r[0].changes[0].value).toBeCloseTo(127.5, 1);

    // Filtered event produces a typed drop, not a clamped write.
    const r2 = s.getState().dispatchEvent({ sourceId: 'src-1', value: 1.5, atMs: 0 });
    expect(r2[0].ok).toBe(false);
    expect(r2[0].drop).toBe('filtered');
  });

  it('rejects unbounded/over-length chains', () => {
    const s = createShowStore();
    const tooLong = chain({ steps: new Array(9).fill({ kind: 'clamp', min: 0, max: 1 }) });
    expect(s.getState().upsertChain(tooLong)).not.toEqual([]);
    const badRange = chain({ steps: [{ kind: 'filter', min: 1, max: 0 }] });
    expect(s.getState().upsertChain(badRange)).not.toEqual([]);
  });

  it('revoke-pairing closes every target on bound chains', () => {
    const s = createShowStore();
    s.getState().upsertChain(chain({ chainId: 'c1', targets: [{ targetId: 't1', kind: 'param' }, { targetId: 't2', kind: 'osc-address' }] }));
    s.getState().upsertChain(chain({ chainId: 'c2', sourceId: 'other', targets: [{ targetId: 't3', kind: 'param' }] }));
    const closed = s.getState().revoke('src-1');
    expect(closed.map((c) => c.targetId).sort()).toEqual(['t1', 't2']);
    // Events on the revoked source drive nothing — typed 'disabled'
    // drop, zero changes.
    const r = s.getState().dispatchEvent({ sourceId: 'src-1', value: 0.5, atMs: 0 });
    expect(r).toHaveLength(1);
    expect(r[0].ok).toBe(false);
    expect(r[0].drop).toBe('disabled');
    expect(r[0].changes).toHaveLength(0);
    // Other source's chain untouched.
    expect(s.getState().dispatchEvent({ sourceId: 'other', value: 0.5, atMs: 0 })[0].ok).toBe(true);
  });
});
