// W14 gestures coverage — T57 core.
//
// T57: "Draw a melodic contour and tap off-grid rhythms, toggle scale
// constraint/quantize strength, preview and commit" → "Raw intention is
// retained; quantization is reversible and accepted notes can be
// edited/undone through normal paths."

import { describe, expect, it } from 'vitest';
import { FakeTransport, VoidClient, type CommandReceipt } from 'void-client';
import { contourToRawNotes, type ContourMapping } from './contour';
import { commitGesture, undoGestureCommit, GestureCommitError } from './commit';
import { stepEntryKey, stepEntrySet, initialStepEntry } from './keyboard';
import { msToTicks, onsetsToRawNotes, tapsToRawNotes } from './rhythm';
import { createGestureSession } from './session';
import {
  defaultTransform,
  PPM,
  type RawGestureNote,
} from './types';
import { quantizeOnset, renderGesture, swingOnset, jitter, JITTER_TIMING_LANE } from './transform';

const GRID = '240000'; // 16th at 960k/quarter

function raw(partial: Partial<RawGestureNote> & { index: number }): RawGestureNote {
  return {
    rawStartTicks: '0',
    rawPitch: 60,
    rawVelocity: 100,
    lengthTicks: GRID,
    ...partial,
  };
}

function client(transport: FakeTransport): VoidClient {
  let n = 0;
  return new VoidClient({ transport, ids: () => `id-${++n}` });
}

function applied(transport: FakeTransport): void {
  transport.respond('send_command', (args) => {
    const dto = (args as { dto: { transaction_id: string } }).dto;
    const receipt: CommandReceipt = {
      kind: 'CommandReceipt',
      command_id: 'c',
      transaction_id: dto.transaction_id,
      status: 'APPLIED',
      error: 'NONE',
      revision: '2',
    };
    return receipt;
  });
}

describe('quantizeOnset (real grid + strength)', () => {
  it('full strength snaps to nearest grid point', () => {
    expect(quantizeOnset(130_000n, 240_000n, PPM)).toBe(240_000n); // nearer up
    expect(quantizeOnset(90_000n, 240_000n, PPM)).toBe(0n); // nearer down
  });
  it('partial strength interpolates toward the grid', () => {
    // t=120k off a 240k grid at 50% → moves halfway to the nearest line.
    expect(quantizeOnset(180_000n, 240_000n, 500_000)).toBe(210_000n);
  });
  it('zero strength is a no-op (raw onset kept verbatim)', () => {
    expect(quantizeOnset(123_456n, 240_000n, 0)).toBe(123_456n);
  });
});

describe('swingOnset + humanize', () => {
  it('delays only the second onset of each pair', () => {
    // swing 2/3: second eighth lands at 2/3 of the pair instead of 1/2.
    const straight = 500_000;
    const swung = 666_667;
    expect(swingOnset(0n, 240_000n, swung)).toBe(0n); // downbeat stays
    const moved = swingOnset(240_000n, 240_000n, swung);
    expect(moved).toBe(320_000n); // 480k × 2/3 ≈ 320k
    expect(swingOnset(240_000n, 240_000n, straight)).toBe(240_000n);
  });
  it('jitter is seeded-deterministic per (seed,index,lane)', () => {
    const a = jitter('s1', 3, JITTER_TIMING_LANE, 5_000n);
    const b = jitter('s1', 3, JITTER_TIMING_LANE, 5_000n);
    const c = jitter('s2', 3, JITTER_TIMING_LANE, 5_000n);
    expect(a).toBe(b);
    expect(a).not.toBe(c);
    expect(Math.abs(Number(a))).toBeLessThanOrEqual(5_000);
  });
});

describe('tapsToRawNotes — raw timing retained', () => {
  it('keeps exact inter-onset timing; first tap anchors origin', () => {
    const notes = tapsToRawNotes(
      [{ tMs: 100 }, { tMs: 400 }, { tMs: 1100 }],
      { bpm: 120, originTicks: '960000', pitch: 36, length: 'gap' },
    );
    expect(notes).toHaveLength(3);
    // 300ms @120bpm = 300·120·16 = 576000 ticks; 1000ms = 1920000.
    expect(notes[0].rawStartTicks).toBe('960000');
    expect(notes[1].rawStartTicks).toBe('1536000');
    expect(notes[2].rawStartTicks).toBe('2880000');
    // gap lengths run to the next onset.
    expect(notes[0].lengthTicks).toBe('576000');
  });
  it('onsetsToRawNotes is the numeric alternative (beats)', () => {
    const notes = onsetsToRawNotes([0, 1.5, 2], 'beats', { bpm: 120, pitch: 38 });
    expect(notes.map((n) => n.rawStartTicks)).toEqual([
      '0',
      '1440000',
      '1920000',
    ]);
  });
});

describe('renderGesture — reversible transforms (T57)', () => {
  const phrase = [
    raw({ index: 0, rawStartTicks: '100000', rawPitch: 63, rawVelocity: 90 }),
    raw({ index: 1, rawStartTicks: '340000', rawPitch: 65, rawVelocity: 80 }),
  ];
  it('raw fields are preserved in rendered notes', () => {
    const t = defaultTransform(GRID);
    const out = renderGesture(phrase, t);
    expect(out[0].rawStartTicks).toBe('100000');
    expect(out[0].rawPitch).toBe(63);
    expect(out[1].rawVelocity).toBe(80);
  });
  it('strength 0 returns the raw onsets verbatim (reversible)', () => {
    const strong = renderGesture(phrase, { ...defaultTransform(GRID), quantizeStrengthPpm: PPM });
    const off = renderGesture(phrase, { ...defaultTransform(GRID), quantizeStrengthPpm: 0 });
    expect(strong[0].startTicks).not.toBe('100000');
    expect(off.map((n) => n.startTicks)).toEqual(['100000', '340000']);
  });
  it('pitch constraint snaps rendered pitch only', () => {
    const snapToC = (p: number) => 60; // trivial constraint for the test
    const out = renderGesture(phrase, defaultTransform(GRID), snapToC);
    expect(out[0].pitch).toBe(60);
    expect(out[0].rawPitch).toBe(63);
  });
});

describe('contourToRawNotes', () => {
  const m: ContourMapping = {
    xToTicks: (x) => String(Math.round(x * 1_920_000)), // 2 bars
    yToPitch: (y) => 48 + y * 24,
  };
  it('resamples the path into cells and merges same-pitch runs', () => {
    // A flat top-half line → constant pitch 60 → merged single note.
    const pts = Array.from({ length: 20 }, (_, i) => ({
      tMs: i * 10,
      x: (i / 19) * 0.99,
      y: 0.5,
    }));
    const notes = contourToRawNotes(pts, m, { cellTicks: '240000', velocity: 96 });
    expect(notes.length).toBeGreaterThanOrEqual(1);
    expect(notes[0].rawPitch).toBe(60);
    const total = notes.reduce((s, n) => s + BigInt(n.lengthTicks), 0n);
    expect(total).toBeGreaterThan(0n);
    // merged run → all same pitch, contiguous.
    for (const n of notes) expect(n.rawPitch).toBe(60);
  });
  it('produces distinct notes when pitch changes across cells', () => {
    const pts = [
      { tMs: 0, x: 0, y: 0.2 },
      { tMs: 10, x: 0.01, y: 0.2 },
      { tMs: 20, x: 0.3, y: 0.9 },
      { tMs: 30, x: 0.31, y: 0.9 },
    ];
    const notes = contourToRawNotes(pts, m, { cellTicks: '240000' });
    const pitches = notes.map((n) => n.rawPitch);
    expect(new Set(pitches).size).toBeGreaterThan(1);
  });
});

describe('step-entry keyboard path (GEST-06 parity)', () => {
  it('arrows move the cursor; Enter places a note and advances', () => {
    let s = initialStepEntry('240000');
    s = stepEntrySet(s, { pitch: 64 });
    s = stepEntryKey(s, 'ArrowRight')!; // cursor → +240000
    const placed = stepEntryKey(s, 'Enter')!;
    expect(placed.notes).toHaveLength(1);
    expect(placed.notes[0].rawStartTicks).toBe('240000');
    expect(placed.notes[0].rawPitch).toBe(64);
    expect(placed.cursorTicks).toBe('480000'); // advanced by note length
    // Undo-last via Backspace.
    const removed = stepEntryKey(placed, 'Backspace')!;
    expect(removed.notes).toHaveLength(0);
  });
});

describe('session: arm → capture → preview → commit → undo (T57)', () => {
  function tappedSession() {
    const s = createGestureSession();
    expect(s.getState().actions.arm('rhythm', { trackId: 'trk-1', clipId: 'clip-1' })).toBe(true);
    expect(s.getState().actions.beginCapture()).toBe(true);
    s.getState().actions.addTap({ tMs: 0 });
    s.getState().actions.addTap({ tMs: 100 });
    s.getState().actions.addTap({ tMs: 500 });
    expect(
      s.getState().actions.endCapture({
        rhythm: { bpm: 120, pitch: 38, length: '240000' },
      }),
    ).toBe(true);
    return s;
  }

  it('unarmed capture is refused (gestures never steal input)', () => {
    const s = createGestureSession();
    expect(s.getState().actions.beginCapture()).toBe(false);
    s.getState().actions.addTap({ tMs: 0 });
    expect(s.getState().taps).toHaveLength(0);
  });

  it('preview re-renders from raw when the transform changes (T57)', () => {
    const s = tappedSession();
    // Default transform has strength 0 → preview IS the raw timing.
    const rawOnsets = s.getState().raw.map((n) => n.rawStartTicks);
    expect(s.getState().preview.map((n) => n.startTicks)).toEqual(rawOnsets);
    // Toggle quantize on → preview snaps (192000 → 240000 on the grid).
    s.getState().actions.setTransform({ quantizeStrengthPpm: PPM });
    const snapped = s.getState().preview.map((n) => n.startTicks);
    expect(snapped).toEqual(['0', '240000', '960000']);
    // Toggle back off → exact raw onsets return: quantization is reversible.
    s.getState().actions.setTransform({ quantizeStrengthPpm: 0 });
    expect(s.getState().preview.map((n) => n.startTicks)).toEqual(rawOnsets);
    expect(s.getState().raw.map((n) => n.rawStartTicks)).toEqual(rawOnsets);
  });

  it('constraint toggle snaps preview pitch without touching raw', () => {
    const s = tappedSession();
    s.getState().actions.setConstraint(() => 40);
    expect(s.getState().preview.every((n) => n.pitch === 40)).toBe(true);
    expect(s.getState().raw.every((n) => n.rawPitch === 38)).toBe(true);
    s.getState().actions.setConstraint(null);
    expect(s.getState().preview.every((n) => n.pitch === 38)).toBe(true);
  });

  it('commit sends InsertNoteOps under ONE transactionId; undo is a normal UndoOp (GEST-05)', async () => {
    const t = new FakeTransport();
    applied(t);
    const c = client(t);
    const s = tappedSession();
    let mint = 0;
    const plan = await commitGesture(s, c, () => `mint-${++mint}`);
    expect(plan).not.toBeNull();
    const calls = t.calls.filter((x) => x.cmd === 'send_command');
    expect(calls.length).toBe(s.getState().raw.length);
    const txIds = new Set(
      calls.map((x) => (x.args as { dto: { transaction_id: string } }).dto.transaction_id),
    );
    expect(txIds.size).toBe(1);
    expect(txIds.has(plan!.transactionId)).toBe(true);
    for (const call of calls) {
      const op = (call.args as { dto: { op: Record<string, unknown> } }).dto.op;
      expect(Object.keys(op)).toEqual(['InsertNoteOp']);
    }
    expect(s.getState().phase).toBe('committed');
    // Undo via the normal path — one UndoOp with the gesture's txId.
    const before = t.calls.length;
    await undoGestureCommit(s, c);
    const last = t.calls[t.calls.length - 1];
    expect(t.calls.length).toBe(before + 1);
    const undoOp = (last.args as { dto: { op: { UndoOp: { transaction_id: string } } } }).dto.op;
    expect(undoOp.UndoOp.transaction_id).toBe(plan!.transactionId);
  });

  it('a failed receipt surfaces verbatim and does not mark committed', async () => {
    const t = new FakeTransport();
    t.respond('send_command', (args) => ({
      kind: 'CommandReceipt' as const,
      command_id: 'c',
      transaction_id: (args as { dto: { transaction_id: string } }).dto.transaction_id,
      status: 'REJECTED' as const,
      error: 'CLIP_NOT_FOUND' as const,
      message: 'no such clip',
    }));
    const c = client(t);
    const s = tappedSession();
    let mint = 0;
    await expect(commitGesture(s, c, () => `mint-${++mint}`)).rejects.toThrow(
      GestureCommitError,
    );
    expect(s.getState().lastError).toContain('CLIP_NOT_FOUND');
    expect(s.getState().phase).not.toBe('committed');
    expect(s.getState().lastCommit).toBeNull();
  });

  it('release clears capture+preview — nothing left pending/on', () => {
    const s = tappedSession();
    s.getState().actions.release('focus-lost');
    const st = s.getState();
    expect(st.phase).toBe('idle');
    expect(st.preview).toHaveLength(0);
    expect(st.taps).toHaveLength(0);
    expect(st.raw).toHaveLength(0);
    // The pad's target survives for re-arm UX (documented release
    // contract) — pending input and ghost layer are fully gone.
    expect(st.target).toEqual({ trackId: 'trk-1', clipId: 'clip-1' });
  });
});
