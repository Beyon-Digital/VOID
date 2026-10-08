// W14 scenario coverage — the TEST_MATRIX rows T57/T58/T59 end to end
// through the real store + commit path over FakeTransport.
//
// Each scenario names the matrix line it exercises; module-level tests
// cover the units. This file proves they compose.

import { describe, expect, it } from 'vitest';
import { FakeTransport, VoidClient, type CommandReceipt } from 'void-client';
import { commitGesture, undoGestureCommit } from './gestures/commit';
import { createGestureSession } from './gestures/session';
import { PPM } from './gestures/types';
import { makeConstraint, type HarmonyConstraintSpec } from './harmony/constraint';
import { emptyChordTrack, upsertRegion, type ChordRegion } from './harmony/track';
import { parseChordSymbol } from './harmony/chords';
import { createLearnStore } from './learn/store';
import { builtinLoops, previewLoop } from './patterns/loops';
import { sliceByCount, sliceMapToClipOps } from './patterns/slices';
import { patternToInsertOps, toggleStep, type StepPattern } from './patterns/stepPattern';

function client(transport: FakeTransport): VoidClient {
  let n = 0;
  return new VoidClient({ transport, ids: () => `id-${++n}` });
}

function applied(transport: FakeTransport): void {
  transport.respond('send_command', (args) => ({
    kind: 'CommandReceipt' as const,
    command_id: 'c',
    transaction_id: (args as { dto: { transaction_id: string } }).dto.transaction_id,
    status: 'APPLIED' as const,
    error: 'NONE' as const,
    revision: '9',
  }));
}

function mint() {
  let n = 0;
  return () => `mint-${++n}`;
}

describe('T57 — contour + off-grid taps, constraint/quantize toggles, preview, commit', () => {
  it('raw retained, quantization reversible, commit+undo through normal paths', async () => {
    const t = new FakeTransport();
    applied(t);
    const c = client(t);

    // Chord constraint armed: notes land on C-major triad tones.
    const track = upsertRegion(emptyChordTrack(), {
      regionId: 'r1',
      startTicks: '0',
      lengthTicks: '3840000',
      chord: parseChordSymbol('C')!,
      voicing: 'close',
    } satisfies ChordRegion);
    const spec: HarmonyConstraintSpec = { mode: 'chord', track };

    const s = createGestureSession();
    s.getState().actions.arm('rhythm', { trackId: 'trk', clipId: 'clip' });
    s.getState().actions.beginCapture();
    // Off-grid taps (quantization only moves them at render time).
    for (const tMs of [0, 137, 411, 723]) s.getState().actions.addTap({ tMs });
    s.getState().actions.endCapture({
      rhythm: { bpm: 120, pitch: 61, length: '240000' }, // C# — snaps under constraint
    });
    const rawOnsets = s.getState().raw.map((n) => n.rawStartTicks);
    const rawPitch = s.getState().raw[0].rawPitch;
    expect(rawPitch).toBe(61); // intention retained verbatim

    // Toggle constraint ON → preview snaps pitch to chord tones only.
    s.getState().actions.setConstraint(makeConstraint(spec));
    const snapped = s.getState().preview.map((n) => n.pitch);
    for (const p of snapped) expect([60, 64, 67]).toContain(p);
    // Toggle quantize ON → onsets move toward the grid.
    s.getState().actions.setTransform({ quantizeStrengthPpm: PPM });
    const onsets = s.getState().preview.map((n) => n.startTicks);
    expect(onsets.every((o) => BigInt(o) % 240_000n === 0n)).toBe(true);
    // …and back OFF → raw onsets return (reversible).
    s.getState().actions.setTransform({ quantizeStrengthPpm: 0 });
    expect(s.getState().preview.map((n) => n.startTicks)).toEqual(rawOnsets);

    // Commit → InsertNoteOps under one transaction id.
    const plan = await commitGesture(s, c, mint());
    expect(plan!.ops.length).toBe(4);
    const tx = new Set(
      t.calls.map(
        (x) => (x.args as { dto: { transaction_id: string } }).dto.transaction_id,
      ),
    );
    expect(tx.size).toBe(1);
    // Undo through the normal path reverses the phrase.
    await undoGestureCommit(s, c);
    const undo = t.calls[t.calls.length - 1];
    expect(
      (undo.args as { dto: { op: { UndoOp: { transaction_id: string } } } }).dto.op
        .UndoOp.transaction_id,
    ).toBe(plan!.transactionId);
  });
});

describe('T58 — map a control, navigate, disconnect + focus-loss, panic', () => {
  it('only the armed target responds; release/panic prevents held notes', () => {
    const st = createLearnStore();

    // MIDI-learn: capture a control for 'filter.cutoff'.
    st.getState().actions.beginLearn({ kind: 'param', id: 'filter.cutoff' });
    st.getState().actions.dispatch({
      control: { kind: 'cc', channel: 0, id: '20' },
      value: 0,
      phase: 'down',
      atMs: 1,
    });
    expect(st.getState().mappings).toHaveLength(1);

    // Map a touch-axis control to a different param too.
    st.getState().actions.upsertMapping({
      mappingId: 'axis-y',
      control: { kind: 'axis', id: 'y' },
      target: { kind: 'param', id: 'synth.res' },
      curve: 'linear',
      min: 0,
      max: 1,
      invert: false,
      outOfRange: 'clamp',
    });

    // Navigate elsewhere (nothing armed) → controls change nothing.
    const roam = st.getState().actions.dispatch({
      control: { kind: 'cc', channel: 0, id: '20' },
      value: 0.9,
      phase: 'value',
      atMs: 2,
    });
    expect(roam.changes).toHaveLength(0);
    expect(roam.ignored).toBe('no-arm');

    // Arm cutoff → CC20 moves it; the axis stays silent.
    st.getState().actions.arm({ kind: 'param', id: 'filter.cutoff' });
    const hit = st.getState().actions.dispatch({
      control: { kind: 'cc', channel: 0, id: '20' },
      value: 0.6,
      phase: 'value',
      atMs: 3,
    });
    expect(hit.changes).toEqual([
      { paramId: 'filter.cutoff', value: 0.6, mappingId: expect.any(String), atMs: 3 },
    ]);
    const nav = st.getState().actions.dispatch({
      control: { kind: 'axis', id: 'y' },
      value: 0.9,
      phase: 'value',
      atMs: 4,
    });
    expect(nav.changes).toHaveLength(0);
    expect(nav.ignored).toBe('target-not-armed');

    // Held momentary control + disconnect → released, nothing left on.
    st.getState().actions.dispatch({
      control: { kind: 'note', channel: 0, id: '60' },
      value: 1,
      phase: 'down',
      atMs: 5,
    });
    const rel = st.getState().actions.releaseAll('disconnect');
    expect(rel.released.map((c) => c.id)).toContain('60');
    expect(st.getState().held).toEqual({});

    // Re-arm and hold again, then focus-loss panic → priority release.
    st.getState().actions.arm({ kind: 'param', id: 'filter.cutoff' });
    st.getState().actions.dispatch({
      control: { kind: 'note', channel: 0, id: '64' },
      value: 1,
      phase: 'down',
      atMs: 6,
    });
    const pan = st.getState().actions.panic();
    expect(pan.released.map((c) => c.id)).toContain('64');
    expect(st.getState().panicCount).toBe(1);
    expect(st.getState().armedTarget).toBeNull();
  });
});

describe('T59 — drum pattern + slice + loop preview + chord constraints + undo', () => {
  it('pattern→ops, slices→clip ops, loop preview at tempo, constraint+undo', async () => {
    const t = new FakeTransport();
    applied(t);
    const c = client(t);

    // Build a drum pattern (kick + hat), materialize through normal ops.
    let pattern: StepPattern = {
      patternId: 'p1',
      name: 'groove',
      stepTicks: '240000',
      seed: 't59',
      rows: [
        { rowId: 'kick', pitch: 36, label: 'kick', steps: [
          { on: true, velocity: 110, gatePpm: 1_000_000, repeats: 1, probabilityPpm: 1_000_000, tie: false },
          { on: false, velocity: 100, gatePpm: 1_000_000, repeats: 1, probabilityPpm: 1_000_000, tie: false },
          { on: true, velocity: 110, gatePpm: 1_000_000, repeats: 1, probabilityPpm: 1_000_000, tie: false },
          { on: false, velocity: 100, gatePpm: 1_000_000, repeats: 1, probabilityPpm: 1_000_000, tie: false },
        ] },
        { rowId: 'hat', pitch: 42, label: 'hat', steps: [
          { on: true, velocity: 90, gatePpm: 500_000, repeats: 1, probabilityPpm: 1_000_000, tie: false },
          { on: true, velocity: 70, gatePpm: 500_000, repeats: 1, probabilityPpm: 1_000_000, tie: false },
          { on: true, velocity: 90, gatePpm: 500_000, repeats: 1, probabilityPpm: 1_000_000, tie: false },
          { on: true, velocity: 70, gatePpm: 500_000, repeats: 1, probabilityPpm: 1_000_000, tie: false },
        ] },
      ],
    };
    // Editing a row leaves others inspectable (row-scoped edit).
    pattern = toggleStep(pattern, 'hat', 3);
    expect(pattern.rows[1].steps[3].on).toBe(false);
    expect(pattern.rows[0].steps[0].on).toBe(true);

    const { transactionId, ops } = patternToInsertOps('clip-1', pattern, mint());
    expect(ops).toHaveLength(5); // 2 kicks + 3 hats
    for (const op of ops) await c.sendCommand(op, { transactionId });
    const insertCalls = t.calls.filter((x) => x.cmd === 'send_command');
    expect(insertCalls).toHaveLength(5);

    // Slice a licensed sample — source stays immutable (region refs only).
    const map = sliceByCount('asset-licensed', '1920000', 4, mint());
    const clipOps = sliceMapToClipOps(map, { trackId: 'drums', atTicks: '0' }, mint());
    expect(clipOps.ops).toHaveLength(4);
    for (const op of clipOps.ops) {
      if (!('InsertAudioClipOp' in op)) throw new Error('expected clip op');
      expect(op.InsertAudioClipOp.asset_id).toBe('asset-licensed'); // never duplicated/copied
    }

    // Preview a loop at project tempo — pure render, zero sends.
    const before = t.calls.length;
    const loop = previewLoop(builtinLoops()[0], { bpm: 128, cycles: 2 });
    expect(t.calls.length).toBe(before);
    expect(loop.durationMs).toBeGreaterThan(0);
    expect(BigInt(loop.durationTicks)).toBeGreaterThan(0n);

    // Harmonic constraint applied to new input stays reversible.
    const track = upsertRegion(emptyChordTrack(), {
      regionId: 'r1',
      startTicks: '0',
      lengthTicks: '3840000',
      chord: parseChordSymbol('Cmaj7')!,
      voicing: 'close',
    } satisfies ChordRegion);
    const constrain = makeConstraint({ mode: 'chord', track })!;
    expect([0, 4, 7, 11]).toContain(constrain(68, '0') % 12);

    // Undo the pattern insert through the normal path — inspectable.
    const undoReceipt = await c.undo(transactionId);
    expect(undoReceipt.status).toBe('APPLIED');
    const last = t.calls[t.calls.length - 1];
    expect(
      (last.args as { dto: { op: { UndoOp: { transaction_id: string } } } }).dto.op
        .UndoOp.transaction_id,
    ).toBe(transactionId);
  });
});
