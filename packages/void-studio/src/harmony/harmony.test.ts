// W14 harmony coverage — TIME-05 + T59 harmonic controls.
//
// Chord regions gate NEW input via a reversible pitch constraint, and
// the explicit harmonic-follow transform edits only the selected notes
// under one transaction id (undone through the normal UndoOp).

import { describe, expect, it } from 'vitest';
import { FakeTransport, VoidClient, type CommandReceipt } from 'void-client';
import {
  formatChordSymbol,
  parseChordSymbol,
  pitchClasses,
  SCALES,
  scaleSet,
} from './chords';
import { identifyChord, isConfident } from './identify';
import {
  chordAt,
  emptyChordTrack,
  parseChordTrack,
  removeRegion,
  serializeChordTrack,
  upsertRegion,
  type ChordRegion,
} from './track';
import {
  governingSet,
  makeConstraint,
  snapPitchToSet,
  type HarmonyConstraintSpec,
} from './constraint';
import { applyHarmonicFollow, planHarmonicFollow, undoHarmonicFollow } from './follow';
import { createChordTrackStore } from './store';

const region = (id: string, start: string, len: string, sym: string): ChordRegion => ({
  regionId: id,
  startTicks: start,
  lengthTicks: len,
  chord: parseChordSymbol(sym)!,
  voicing: 'close',
});

function client(transport: FakeTransport): VoidClient {
  let n = 0;
  return new VoidClient({ transport, ids: () => `id-${++n}` });
}

describe('chord symbols — parse/format/intervals', () => {
  it('parses the required set: major/minor/sus/dim/7ths (and more)', () => {
    expect(parseChordSymbol('C')).toEqual({ rootPc: 0, quality: 'maj' });
    expect(parseChordSymbol('F#m')).toEqual({ rootPc: 6, quality: 'min' });
    expect(parseChordSymbol('Bb7')).toEqual({ rootPc: 10, quality: '7', flat: true });
    expect(parseChordSymbol('Csus4')).toEqual({ rootPc: 0, quality: 'sus4' });
    expect(parseChordSymbol('Edim')).toEqual({ rootPc: 4, quality: 'dim' });
    expect(parseChordSymbol('Gm7')).toEqual({ rootPc: 7, quality: 'min7' });
    expect(parseChordSymbol('Dø7')).toEqual({ rootPc: 2, quality: 'min7b5' });
    expect(parseChordSymbol('not-a-chord')).toBeNull();
    expect(parseChordSymbol('H')).toBeNull(); // H is not a root in this spec
  });

  it('round-trips through formatChordSymbol', () => {
    for (const s of ['C', 'F#m', 'Bb7', 'Cmaj7', 'Am', 'Gsus4']) {
      expect(formatChordSymbol(parseChordSymbol(s)!)).toBe(s);
    }
  });

  it('pitch-class sets are real semitone sets', () => {
    const cmaj7 = pitchClasses(parseChordSymbol('Cmaj7')!);
    expect([...cmaj7].sort((a, b) => a - b)).toEqual([0, 4, 7, 11]);
    const bbdim = pitchClasses(parseChordSymbol('Bbdim')!);
    expect([...bbdim].sort((a, b) => a - b)).toEqual([1, 4, 10]);
  });

  it('scale sets transpose correctly', () => {
    const dMajor = scaleSet(2, SCALES.major);
    expect(dMajor.has(2)).toBe(true); // D
    expect(dMajor.has(6)).toBe(true); // F#
    expect(dMajor.has(5)).toBe(false); // F natural
  });
});

describe('identifyChord — honest ambiguity (INTEL-02)', () => {
  it('ranks the obvious triad first and keeps the ambiguity visible', () => {
    const guesses = identifyChord([60, 64, 67]); // C E G
    expect(guesses[0].confidence).toBeGreaterThan(0);
    const symbols = guesses.map((g) => formatChordSymbol(g.chord));
    expect(symbols[0]).toBe('C'); // exact coverage+precision wins
    // C-E-G also completes to larger chords (Am7, Fmaj9…) — honest
    // ambiguity stays listed instead of pretending certainty.
    expect(guesses.length).toBeGreaterThan(1);
    expect(guesses.every((g) => g.extra.length === 0)).toBe(true);
  });
  it('a single note is low confidence — suggestion only', () => {
    const guesses = identifyChord([60]);
    for (const g of guesses) expect(isConfident(g)).toBe(false);
  });
});

describe('chord track regions (TIME-05)', () => {
  it('chordAt hits inside a region, null outside', () => {
    let t = emptyChordTrack();
    t = upsertRegion(t, region('r1', '0', '960000', 'C'));
    expect(chordAt(t, '480000')?.chord.quality).toBe('maj');
    expect(chordAt(t, '960000')).toBeNull();
    expect(chordAt(t, '960001')).toBeNull();
  });

  it('upsert trims/splits overlapping regions deterministically', () => {
    let t = emptyChordTrack();
    t = upsertRegion(t, region('r1', '0', '960000', 'C'));
    t = upsertRegion(t, region('r2', '480000', '480000', 'G'));
    // r1 is split: [0,480k)=C, [480k,960k)=G; left fragment keeps r1 id.
    expect(t.regions).toHaveLength(2);
    expect(chordAt(t, '240000')?.regionId).toBe('r1');
    expect(chordAt(t, '720000')?.regionId).toBe('r2');
    // A region fully covering r1+r2 replaces them.
    t = upsertRegion(t, region('r3', '0', '1920000', 'F'));
    expect(t.regions).toHaveLength(1);
    expect(chordAt(t, '1200000')?.chord.quality).toBe('maj');
  });

  it('serialize/parse round-trips and skips garbage rows', () => {
    let t = emptyChordTrack();
    t = upsertRegion(t, region('r1', '0', '960000', 'Cm7'));
    const back = parseChordTrack(serializeChordTrack(t));
    expect(back.regions).toHaveLength(1);
    expect(back.regions[0].chord).toEqual({ rootPc: 0, quality: 'min7', flat: undefined });
    // (flat is only present on flat-spelled roots — Cm7 carries none)
    expect(parseChordTrack('not json').regions).toHaveLength(0);
    expect(
      parseChordTrack('{"regions":[{"chord":"bogus"}]}').regions,
    ).toHaveLength(0);
  });

  it('removeRegion deletes by id', () => {
    let t = upsertRegion(emptyChordTrack(), region('r1', '0', '960000', 'C'));
    t = removeRegion(t, 'r1');
    expect(t.regions).toHaveLength(0);
  });
});

describe('harmonic constraint for input (T57/T59)', () => {
  const track = (() => {
    let t = emptyChordTrack();
    t = upsertRegion(t, region('r1', '0', '960000', 'C')); // C E G
    t = upsertRegion(t, region('r2', '960000', '960000', 'Dm')); // D F A
    return t;
  })();

  it('snapPitchToSet snaps to the nearest chord tone, ties prefer down', () => {
    const c = pitchClasses(parseChordSymbol('C')!); // {0,4,7}
    expect(snapPitchToSet(61, c)).toBe(60); // C# → C
    expect(snapPitchToSet(62, c)).toBe(60); // D: 60 or 64 — tie → down
    expect(snapPitchToSet(60, c)).toBe(60); // already a tone
  });

  it('mode=chord snaps to the region covering the onset', () => {
    const spec: HarmonyConstraintSpec = { mode: 'chord', track };
    const constrain = makeConstraint(spec)!;
    expect(constrain(61, '480000')).toBe(60); // in C region → C
    expect(constrain(61, '1440000')).toBe(62); // in Dm region → D
  });

  it('mode=off is a passthrough — makeConstraint returns null', () => {
    expect(makeConstraint({ mode: 'off', track })).toBeNull();
  });

  it('uncovered onsets fall back to the scale (or pass through without one)', () => {
    const spec: HarmonyConstraintSpec = {
      mode: 'chord',
      track,
      fallbackScale: { rootPc: 0, scale: 'major' },
    };
    // 1920000 is past every region → C-major scale snap (F#→G… wait 66=F#).
    expect(governingSet('1920000', spec)!.has(7)).toBe(true);
    const free: HarmonyConstraintSpec = { mode: 'chord', track };
    expect(governingSet('1920000', free)).toBeNull();
    expect(makeConstraint(free)!(66, '1920000')).toBe(66);
  });
});

describe('harmonic follow — explicit transform + normal undo (T59)', () => {
  const track = (() => {
    let t = emptyChordTrack();
    t = upsertRegion(t, region('r1', '0', '960000', 'C'));
    return t;
  })();

  it('plan only contains selected, off-chord notes in covered regions', () => {
    const plan = planHarmonicFollow(
      [
        { noteId: 'n1', clipId: 'c1', pitch: 61, startTicks: '0' }, // C# → C
        { noteId: 'n2', clipId: 'c1', pitch: 64, startTicks: '480000' }, // in tune
        { noteId: 'n3', clipId: 'c1', pitch: 61, startTicks: '1920000' }, // uncovered
      ],
      track,
      { mode: 'chord' },
      () => 'tx-1',
    );
    expect(plan.changes).toEqual([
      { noteId: 'n1', clipId: 'c1', fromPitch: 61, toPitch: 60 },
    ]);
    expect(plan.skipped.map((s) => s.noteId)).toEqual(['n2', 'n3']);
    expect(plan.ops).toHaveLength(1);
    expect(plan.ops[0]).toEqual({
      SetNoteOp: { clip_id: 'c1', note_id: 'n1', pitch: 60 },
    });
  });

  it('apply sends SetNoteOps under one transaction id; undo is a normal UndoOp', async () => {
    const t = new FakeTransport();
    t.respond('send_command', (args) => ({
      kind: 'CommandReceipt' as const,
      command_id: 'c',
      transaction_id: (args as { dto: { transaction_id: string } }).dto.transaction_id,
      status: 'APPLIED' as const,
      error: 'NONE' as const,
      revision: '3',
    }));
    const c = client(t);
    const plan = planHarmonicFollow(
      [
        { noteId: 'n1', clipId: 'c1', pitch: 61, startTicks: '0' },
        { noteId: 'n2', clipId: 'c1', pitch: 62, startTicks: '0' }, // D→E? D→60? D=62→60 or 64: tie → down (60? no—)
      ],
      track,
      { mode: 'chord' },
      () => 'tx-follow',
    );
    // D(62) over C chord {0,4,7}: nearest = 60? |62-60|=2, |62-64|=2 → tie down → 60.
    const { errorText } = await applyHarmonicFollow(c, plan);
    expect(errorText).toBe('');
    const calls = t.calls.filter((x) => x.cmd === 'send_command');
    expect(calls).toHaveLength(2);
    for (const call of calls) {
      const dto = (call.args as { dto: { transaction_id: string; op: object } }).dto;
      expect(dto.transaction_id).toBe('tx-follow');
      expect(Object.keys(dto.op)).toEqual(['SetNoteOp']);
    }
    const before = t.calls.length;
    await undoHarmonicFollow(c, plan);
    const last = t.calls[t.calls.length - 1];
    expect(t.calls.length).toBe(before + 1);
    expect(
      (last.args as { dto: { op: { UndoOp: { transaction_id: string } } } }).dto.op.UndoOp
        .transaction_id,
    ).toBe('tx-follow');
  });
});

describe('chord track store', () => {
  it('upsert/remove/select + constraint mode', () => {
    const st = createChordTrackStore();
    st.getState().actions.upsertSymbol({
      regionId: 'r1',
      startTicks: '0',
      lengthTicks: '960000',
      chord: parseChordSymbol('C')!,
    });
    st.getState().actions.select('r1');
    st.getState().actions.setConstraintMode('chord');
    expect(st.getState().track.regions).toHaveLength(1);
    expect(st.getState().selectedRegionId).toBe('r1');
    expect(st.getState().constraintMode).toBe('chord');
    st.getState().actions.remove('r1');
    expect(st.getState().selectedRegionId).toBeNull();
  });
});
