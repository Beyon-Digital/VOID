// T70 — automation modes, move-with-region, interpolation boundaries,
// and one-transaction undo of a mixed mixer/arrangement gesture.

import { describe, expect, it } from 'vitest';
import { FakeTransport, VoidClient, type CommandReceipt } from 'void-client';
import {
  abFlipOps,
  advance,
  beginPass,
  commitPass,
  createAutomationStore,
  endPass,
  evalCurve,
  evalLane,
  makeAB,
  makeLane,
  moveRegionAutomation,
  moveValue,
  release,
  sendAsOneTransaction,
  splitAutomationAt,
  touch,
  undoTransaction,
  WriteNotArmed,
  type AutomationLane,
} from './index';
import { setTrackGainOp } from '../mixer/ops';

const lane = (over: Partial<AutomationLane> = {}): AutomationLane =>
  makeLane('l1', { kind: 'trackGain', trackId: 'trk' }, over);

describe('interpolation — linear and stepped boundaries', () => {
  const pts = [
    { ticks: '100', value: 0 },
    { ticks: '200', value: 10 },
    { ticks: '400', value: 20 },
  ];
  it('holds edge values outside the curve', () => {
    expect(evalCurve(pts, '0', 'linear')).toBe(0);
    expect(evalCurve(pts, '99', 'linear')).toBe(0);
    expect(evalCurve(pts, '400', 'linear')).toBe(20);
    expect(evalCurve(pts, '99999', 'linear')).toBe(20);
    expect(evalCurve([], '50', 'linear')).toBeNull();
  });
  it('interpolates inside a segment and lands exactly on knots', () => {
    expect(evalCurve(pts, '150', 'linear')).toBe(5); // midpoint 0→10
    expect(evalCurve(pts, '200', 'linear')).toBe(10); // knot, not slope
    expect(evalCurve(pts, '300', 'linear')).toBe(15); // midpoint 10→20
  });
  it('stepped holds the latest point at or before t', () => {
    expect(evalCurve(pts, '150', 'stepped')).toBe(0);
    expect(evalCurve(pts, '199', 'stepped')).toBe(0);
    expect(evalCurve(pts, '200', 'stepped')).toBe(10); // step AT the knot
    expect(evalCurve(pts, '399', 'stepped')).toBe(10);
  });
  it('evalLane sums base + trim', () => {
    const l = lane({
      base: pts,
      trim: [
        { ticks: '0', value: 2 },
        { ticks: '500', value: 2 },
      ],
    });
    expect(evalLane(l, '300')).toBe(17);
  });
});

describe('write-pass state machine', () => {
  it('read and off modes reject beginPass', () => {
    expect(() => beginPass(lane(), 'read', '0')).toThrow(WriteNotArmed);
    expect(() => beginPass(lane(), 'off', '0')).toThrow(WriteNotArmed);
  });

  it('write mode overwrites the whole pass range', () => {
    let l = lane({
      base: [
        { ticks: '0', value: 1 },
        { ticks: '50', value: 5 },
        { ticks: '100', value: 2 },
        { ticks: '200', value: 9 },
      ],
    });
    const p = beginPass(l, 'write', '40', 7);
    advance(p, '60', 7);
    advance(p, '150', 8);
    endPass(p, '180');
    l = commitPass(l, p);
    // [40,180) replaced; outside points (0,200) survive.
    expect(evalCurve(l.base, '0', 'linear')).toBe(1);
    expect(evalCurve(l.base, '40', 'linear')).toBe(7);
    // written points interpolate linearly between 40:7 and 150:8
    expect(evalCurve(l.base, '100', 'linear')).toBeCloseTo(7.5454, 3);
    expect(evalCurve(l.base, '160', 'linear')).toBeCloseTo(8.2, 3);
    expect(evalCurve(l.base, '200', 'linear')).toBe(9);
  });

  it('touch writes only the touched span and releases', () => {
    let l = lane({
      base: [
        { ticks: '0', value: 0 },
        { ticks: '100', value: 0 },
        { ticks: '200', value: 0 },
      ],
    });
    const p = beginPass(l, 'touch', '0');
    touch(p, '80', 5);
    moveValue(p, '120', 6);
    release(p, '150');
    // after release, advance does nothing (touch released back to curve)
    advance(p, '170', 9);
    endPass(p, '200');
    l = commitPass(l, p);
    expect(evalCurve(l.base, '80', 'linear')).toBe(5);
    expect(evalCurve(l.base, '120', 'linear')).toBe(6);
    // points after release keep the underlying curve, not the drag
    expect(evalCurve(l.base, '170', 'linear')).not.toBe(9);
  });

  it('latch holds the released value to the pass end', () => {
    let l = lane({ base: [{ ticks: '0', value: 1 }] });
    const p = beginPass(l, 'latch', '0');
    touch(p, '100', 4);
    release(p, '150'); // latch keeps writing 4 after release
    endPass(p, '300');
    l = commitPass(l, p);
    expect(evalCurve(l.base, '299', 'linear')).toBe(4);
    expect(l.base[l.base.length - 1]).toEqual({ ticks: '300', value: 4 });
  });

  it('trim writes relative offsets into trim, base untouched', () => {
    const base = [
      { ticks: '0', value: 10 },
      { ticks: '400', value: 10 },
    ];
    let l = lane({ base });
    const p = beginPass(l, 'trim', '0');
    touch(p, '100', -2);
    release(p, '150');
    endPass(p, '300');
    l = commitPass(l, p);
    expect(l.base).toEqual(base); // base curve never moved
    expect(evalCurve(l.trim, '200', 'linear')).toBe(-2);
    expect(evalLane(l, '200')).toBe(8); // 10 + (-2)
  });

  it('commitPass refuses an open pass and a foreign lane', () => {
    const l = lane();
    const p = beginPass(l, 'touch', '0');
    touch(p, '10', 3);
    expect(() => commitPass(l, p)).toThrow(/still open/);
    endPass(p, '20');
    expect(() => commitPass(makeLane('other', { kind: 'trackPan', trackId: 'x' }), p)).toThrow(
      /does not belong/,
    );
  });
});

describe('move-with-region semantics', () => {
  const pts = [
    { ticks: '0', value: 1 },
    { ticks: '100', value: 2 }, // inside region
    { ticks: '200', value: 3 }, // inside region
    { ticks: '400', value: 7 },
  ];
  const region = { startTicks: '50', lengthTicks: '200' }; // [50,250)

  it('shifts inside points, anchors edges, overwrites landing zone', () => {
    let l = lane({ base: pts });
    // region [50,250) moved +350 → landing zone [400,600)
    l = moveRegionAutomation(l, region, '350');
    // original inside points moved +350
    expect(l.base.some((p) => p.ticks === '450' && p.value === 2)).toBe(true);
    expect(l.base.some((p) => p.ticks === '550' && p.value === 3)).toBe(true);
    // edge anchors at the landing zone carry the pre-move edge values:
    // value at 50 was 1.5 (interp 0:1→100:2), at 250 was 4 (200:3→400:7)
    const aStart = l.base.find((p) => p.ticks === '400');
    const aEnd = l.base.find((p) => p.ticks === '600');
    expect(aStart?.value).toBe(1.5);
    expect(aEnd?.value).toBe(4);
    // the outside point at 400 (value 7) sat in the landing zone and
    // was overwritten by the boundary anchor — cut+paste semantics
    expect(aStart?.value).not.toBe(7);
    // outside point at 0 survives
    expect(l.base.some((p) => p.ticks === '0' && p.value === 1)).toBe(true);
  });

  it('moves the trim layer identically', () => {
    let l = lane({
      base: pts,
      trim: [{ ticks: '100', value: -1 }],
    });
    l = moveRegionAutomation(l, region, '500');
    expect(l.trim.some((p) => p.ticks === '600' && p.value === -1)).toBe(true);
  });

  it('splitAutomationAt inserts exact edge values so later moves keep edges', () => {
    let l = lane({
      base: [
        { ticks: '0', value: 0 },
        { ticks: '200', value: 10 },
      ],
    });
    l = splitAutomationAt(l, '100');
    const at = l.base.find((p) => p.ticks === '100');
    expect(at?.value).toBe(5); // linear midpoint, materialized
  });
});

describe('one-transaction mixed ops + single undo (T70)', () => {
  const applied = (t: FakeTransport) =>
    t.respond('send_command', (args) => ({
      kind: 'CommandReceipt' as const,
      command_id: 'c',
      transaction_id: (args as { dto: { transaction_id: string } }).dto
        .transaction_id,
      status: 'APPLIED' as const,
      error: 'NONE' as const,
      revision: '9',
    }));

  it('a mixer+arrangement gesture shares one transactionId; one UndoOp rewinds it', async () => {
    const t = new FakeTransport();
    applied(t);
    let n = 0;
    const c = new VoidClient({ transport: t, ids: () => `id-${++n}` });

    // Mixed gesture: move a clip AND ride a fader — one transaction.
    const tx = {
      transactionId: 'tx-mix-1',
      ops: [
        {
          MoveClipOp: {
            clip_id: 'cl1',
            track_id: 'trk',
            start_ticks: '1920000',
          },
        },
        setTrackGainOp('trk', 0.5),
      ],
    };
    const outcome = await sendAsOneTransaction(c, tx);
    expect(outcome.settled).toBe(true);
    // every send_command call carried the same transaction id
    const sends = t.calls.filter((x) => x.cmd === 'send_command');
    expect(sends.length).toBe(tx.ops.length);
    for (const s of sends) {
      expect(
        (s.args as { dto: { transaction_id: string } }).dto.transaction_id,
      ).toBe('tx-mix-1');
    }

    // ONE undo for the whole mixed gesture
    const r: CommandReceipt = await undoTransaction(c, 'tx-mix-1');
    expect(r.status).toBe('APPLIED');
    const last = t.calls[t.calls.length - 1];
    expect(
      (last.args as { dto: { op: { UndoOp: { transaction_id: string } } } })
        .dto.op.UndoOp.transaction_id,
    ).toBe('tx-mix-1');
  });

  it('a rejected op surfaces in receipts (no silent swallow)', async () => {
    const t = new FakeTransport();
    t.respond('send_command', (args) => ({
      kind: 'CommandReceipt' as const,
      command_id: 'c',
      transaction_id: (args as { dto: { transaction_id: string } }).dto
        .transaction_id,
      status: 'REJECTED' as const,
      error: 'VALIDATION' as const,
      revision: '9',
    }));
    let n = 0;
    const c = new VoidClient({ transport: t, ids: () => `id-${++n}` });
    const out = await sendAsOneTransaction(c, {
      transactionId: 'tx-bad',
      ops: [setTrackGainOp('trk', 0.5)],
    });
    expect(out.settled).toBe(false);
    expect(out.receipts[0].receipt.status).toBe('REJECTED');
  });
});

describe('A/B toggle', () => {
  it('flips produce real param ops; gain matching scales only gain refs', () => {
    const ab = makeAB(
      [
        { ref: { kind: 'trackGain', trackId: 'a' }, value: 1.0 },
        { ref: { kind: 'trackPan', trackId: 'a' }, value: 0 },
      ],
      [
        { ref: { kind: 'trackGain', trackId: 'a' }, value: 0.8 },
        { ref: { kind: 'trackMute', trackId: 'b' }, value: 1 },
      ],
    );
    const ops = abFlipOps(ab, 'b');
    expect(ops).toHaveLength(2);
    expect('SetTrackGainOp' in ops[0]).toBe(true);
    expect('SetTrackMuteOp' in ops[1]).toBe(true);
    // -2 dB loudness match scales the gain ref only
    const matched = abFlipOps(ab, 'b', { matchDb: -2 });
    const g = matched[0] as { SetTrackGainOp: { gain_linear: number } };
    const panOps = abFlipOps(ab, 'b', { matchDb: -2 })[1];
    expect(g.SetTrackGainOp.gain_linear).toBeCloseTo(0.8 * Math.pow(10, -2 / 20), 6);
    expect('SetTrackMuteOp' in panOps).toBe(true); // mute untouched
  });
});

describe('automation feature store (view-state only)', () => {
  it('commits a pass through the store and moves lane curves with a region', () => {
    const s = createAutomationStore();
    s.getState().actions.upsertLane(
      lane({ base: [{ ticks: '0', value: 1 }, { ticks: '500', value: 1 }] }),
    );
    const st = s.getState();
    st.actions.setWriteArmed(true);
    expect(s.getState().writeArmed).toBe(true);
    // drive a pass through the store
    const l = s.getState().lanes['l1'];
    // latch writes the touched value through to the pass end — a flat
    // segment, so any interior evaluation is exactly the held value.
    const p = beginPass(l, 'latch', '0');
    touch(p, '0', 3);
    endPass(p, '250');
    s.getState().actions.setActivePass(p);
    s.getState().actions.commitActivePass();
    expect(evalCurve(s.getState().lanes['l1'].base, '100', 'linear')).toBe(3);
    expect(s.getState().activePass).toBeUndefined();
    // region move hits the lane through the store action
    s.getState().actions.applyRegionMove(
      ['l1'],
      { startTicks: '0', lengthTicks: '200' },
      '1000',
    );
    expect(
      s.getState().lanes['l1'].base.some((p) => p.ticks === '1000'),
    ).toBe(true);
  });
});
