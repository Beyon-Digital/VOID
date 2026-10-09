// T71 — MIDI event editing, step entry, transforms, articulations,
// MPE flattening with documented MIDI-1.0 losses, and the honest
// MIDI-2.0 declared model.

import { describe, expect, it } from 'vitest';
import {
  allocateZone,
  articulationOf,
  checkModel,
  endChord,
  flattenToMidi1,
  humanize,
  insertNote,
  insertEvent,
  isKeyswitchEvent,
  keyswitchesFor,
  makeCursor,
  MIDI2_TRANSPORT,
  midi2Descriptor,
  MidiModelError,
  moveEvent,
  moveNote,
  quantizeNotes,
  resizeNote,
  retrograde,
  scaleVelocity,
  setArticulation,
  setChord,
  shiftTime,
  STANDARD_ZONE,
  stepBack,
  stepNote,
  stepRest,
  transpose,
  type ArticulationSet,
  type MidiNote,
} from './index';

const note = (id: string, over: Partial<MidiNote> = {}): MidiNote => ({
  id,
  pitch: 60,
  velocity: 100,
  startTicks: '0',
  lengthTicks: '960000',
  ...over,
});

describe('event editor model', () => {
  it('insert/move/resize per event on string-int64 ticks', () => {
    let notes = insertNote([], note('a', { startTicks: '960000' }));
    notes = insertNote(notes, note('b', { startTicks: '0', pitch: 64 }));
    expect(notes.map((n) => n.id)).toEqual(['b', 'a']); // sorted by start
    notes = moveNote(notes, 'b', '480000');
    expect(notes.find((n) => n.id === 'b')!.startTicks).toBe('480000');
    notes = resizeNote(notes, 'a', '480000');
    expect(notes.find((n) => n.id === 'a')!.lengthTicks).toBe('480000');
    expect(() => resizeNote(notes, 'a', '0')).toThrow(MidiModelError);
    expect(() => insertNote(notes, note('b'))).toThrow(/duplicate/);
    expect(() => insertNote(notes, note('c', { pitch: 200 }))).toThrow(
      MidiModelError,
    );
  });

  it('quantize snaps to grid with nearest-ties-away rounding', () => {
    const notes = [
      note('a', { startTicks: '240001' }), // quarter grid = 960000 → ties away → 0? no: 240001 → 0 (nearest multiple 0 vs 960000; 240001 < 480000 → 0)
      note('b', { startTicks: '480000' }), // exact half → ties away → 960000
      note('c', { startTicks: '479999' }), // → 0
      note('keep', { startTicks: '100' }),
    ];
    const q = quantizeNotes(notes, ['a', 'b', 'c'], '960000');
    expect(q.find((n) => n.id === 'a')!.startTicks).toBe('0');
    expect(q.find((n) => n.id === 'b')!.startTicks).toBe('960000');
    expect(q.find((n) => n.id === 'c')!.startTicks).toBe('0');
    expect(q.find((n) => n.id === 'keep')!.startTicks).toBe('100');
    // 50% strength pulls halfway (bigint truncation toward zero:
    // 240001 - 120000.5 -> -120000 truncated -> 120001)
    const half = quantizeNotes(notes, ['a'], '960000', {
      strengthPpm: 500_000,
    });
    expect(half.find((n) => n.id === 'a')!.startTicks).toBe('120001');
  });

  it('channel events edit independently of notes', () => {
    let events = insertEvent([], {
      kind: 'cc',
      id: 'e1',
      startTicks: '100',
      channel: 2,
      controller: 74,
      value: 100,
    });
    events = moveEvent(events, 'e1', '200');
    expect(events[0].startTicks).toBe('200');
    expect(() =>
      insertEvent(events, {
        kind: 'cc',
        id: 'e2',
        startTicks: '0',
        channel: 16, // out of range
        controller: 1,
        value: 1,
      }),
    ).toThrow(MidiModelError);
  });
});

describe('step editor', () => {
  it('step-enter advances; chord stacks then advances once', () => {
    let notes: MidiNote[] = [];
    let c = makeCursor('0', '240000'); // 16th at 960k ticks/qtr
    const r1 = stepNote(notes, c, 60);
    notes = r1.notes;
    c = r1.cursor;
    expect(c.atTicks).toBe('240000');
    expect(notes[0].lengthTicks).toBe('216000'); // 0.9 × step
    // chord mode: entries stack at cursor
    c = setChord(c, true);
    const r2 = stepNote(notes, c, 64);
    notes = r2.notes;
    c = r2.cursor;
    const r3 = stepNote(notes, c, 67);
    notes = r3.notes;
    c = endChord(r3.cursor);
    expect(notes.filter((n) => n.startTicks === '240000')).toHaveLength(2);
    expect(c.atTicks).toBe('480000');
    // rest + back
    c = stepRest(c);
    expect(c.atTicks).toBe('720000');
    c = stepBack(c);
    expect(c.atTicks).toBe('480000');
  });
});

describe('transforms — pure with audit', () => {
  const base = [
    note('lo', { pitch: 120 }),
    note('hi', { pitch: 10, velocity: 10, startTicks: '960000' }),
  ];
  it('transpose clamps at edges and reports what was applied', () => {
    const { notes, applied } = transpose(base, 12);
    expect(notes.find((n) => n.id === 'lo')!.pitch).toBe(127);
    expect(applied.get('lo')).toBe(7); // clamped, not +12
    expect(applied.get('hi')).toBe(12);
  });
  it('velocity scale clamps and reports real ratio', () => {
    const { notes, applied } = scaleVelocity(base, 3);
    expect(notes.find((n) => n.id === 'hi')!.velocity).toBe(30);
    expect(applied.get('hi')).toBe(3);
    expect(notes.find((n) => n.id === 'lo')!.velocity).toBe(127);
    expect(applied.get('lo')).toBeCloseTo(1.27, 5);
    expect(() => scaleVelocity(base, -1)).toThrow(MidiModelError);
  });
  it('time shift handles negative offsets exactly', () => {
    const s = shiftTime(base, '-480000');
    expect(s.find((n) => n.id === 'hi')!.startTicks).toBe('480000');
    expect(s.find((n) => n.id === 'lo')!.startTicks).toBe('-480000');
  });
  it('retrograde mirrors note ends to starts around the pivot', () => {
    const r = retrograde(base, '960000');
    // lo [0,960000) mirrors to [960000,1920000); hi [960000,1920000) → [0,960000)
    expect(r.find((n) => n.id === 'hi')!.startTicks).toBe('0');
    expect(r.find((n) => n.id === 'lo')!.startTicks).toBe('960000');
  });
  it('humanize is deterministic under a seed', () => {
    const a = humanize(base, { timeTicks: '100', velocity: 5, seed: 7 });
    const b = humanize(base, { timeTicks: '100', velocity: 5, seed: 7 });
    expect(a).toEqual(b);
    expect(a.every((n) => n.velocity >= 1 && n.velocity <= 127)).toBe(true);
  });
});

describe('articulations + keyswitch maps', () => {
  const set: ArticulationSet = {
    id: 'strings',
    name: 'Strings',
    channel: 0,
    articulations: [
      { id: 'legato', name: 'Legato', keyswitch: { kind: 'noteOn', pitch: 24 } },
      { id: 'pizz', name: 'Pizzicato', keyswitch: { kind: 'cc', controller: 32, value: 1 } },
    ],
  };
  it('assign validates against the set; articulation survives edits', () => {
    let notes = [note('a')];
    expect(() => setArticulation(set, notes, 'a', 'bogus')).toThrow(
      MidiModelError,
    );
    notes = setArticulation(set, notes, 'a', 'pizz');
    notes = moveNote(notes, 'a', '480000');
    notes = transpose(notes, -12).notes;
    expect(notes[0].articulationId).toBe('pizz');
    expect(notes[0].pitch).toBe(48);
  });
  it('keyswitches emit on articulation change with a clamped lead', () => {
    let notes = [
      note('n1', { startTicks: '960000' }),
      note('n2', { startTicks: '1920000' }),
      note('n3', { startTicks: '2880000' }),
    ];
    notes = setArticulation(set, notes, 'n1', 'legato');
    notes = setArticulation(set, notes, 'n3', 'pizz');
    const ksw = keyswitchesFor(set, notes, { leadTicks: '48000' });
    expect(ksw).toHaveLength(2);
    expect(ksw[0].startTicks).toBe('912000'); // 960000 - 48000
    expect(ksw[0].keyswitch).toEqual({ kind: 'noteOn', pitch: 24 });
    expect(ksw[1].startTicks).toBe('2832000');
    expect(ksw[1].keyswitch).toEqual({ kind: 'cc', controller: 32, value: 1 });
    expect(ksw.every(isKeyswitchEvent)).toBe(true);
    // lead clamps at 0
    const zNotes = setArticulation(
      set,
      [note('z', { startTicks: '100' })],
      'z',
      'legato',
    );
    const early = keyswitchesFor(set, zNotes, {
      leadTicks: '960000',
    });
    expect(early[0].startTicks).toBe('0');
    expect(articulationOf(set, 'pizz')!.name).toBe('Pizzicato');
  });
});

describe('MPE per-note channel model + MIDI 1.0 flatten (T71)', () => {
  const expr = {
    pitchBend: [
      { ticksOffset: '0', value: 0 },
      { ticksOffset: '480000', value: 4096 },
    ],
    pressure: [{ ticksOffset: '240000', value: 90 }],
    cc: [
      {
        controller: 74,
        points: [{ ticksOffset: '480000', value: 64 }],
      },
    ],
  };

  it('overlapping notes get distinct member channels; disjoint reuse', () => {
    const notes = [
      note('a', { startTicks: '0', lengthTicks: '960000' }),
      note('b', { startTicks: '480000', lengthTicks: '960000' }), // overlaps a
      note('c', { startTicks: '960000', lengthTicks: '240000' }), // a ends here — reuses ch
    ];
    const { notes: alloc, unassigned } = allocateZone(notes);
    const ch = (id: string) => alloc.find((n) => n.id === id)!.channel;
    expect(ch('a')).not.toBe(ch('b'));
    expect(ch('c')).toBe(ch('a')); // 'a' ended → channel free
    expect(unassigned).toEqual([]);
  });

  it('per-note bend/pressure/cc flatten to the note’s member channel', () => {
    const clip = checkModel({
      clipId: 'cl',
      notes: [
        note('solo', {
          channel: undefined,
          expression: expr,
          startTicks: '1000',
          lengthTicks: '960000',
        }),
      ],
      events: [],
    });
    const flat = flattenToMidi1(clip);
    expect(flat.losses).toEqual([]);
    const ch = flat.notes[0].channel!;
    const bend = flat.events.filter((e) => e.kind === 'pitchBend');
    const press = flat.events.filter((e) => e.kind === 'polyPressure');
    const cc74 = flat.events.filter((e) => e.kind === 'cc');
    expect(bend).toHaveLength(2);
    expect(bend[0].channel).toBe(ch);
    expect(bend[0].startTicks).toBe('1000'); // note start + offset
    expect(bend[1].startTicks).toBe('481000');
    expect(bend[1].value).toBe(4096);
    expect(press[0].notePitch).toBe(60);
    expect(press[0].value).toBe(90);
    expect(cc74[0].controller).toBe(74);
    expect(cc74[0].startTicks).toBe('481000');
  });

  it('expression survives serialize→edit→export round trip', () => {
    // build → "serialize" (structured clone is the model boundary) →
    // edit → flatten: expression rides the note the whole way.
    const orig: MidiNote = note('rt', { expression: expr, startTicks: '0' });
    const roundTripped = JSON.parse(JSON.stringify(orig)) as MidiNote;
    let notes = insertNote([], roundTripped);
    notes = moveNote(notes, 'rt', '5000');
    const flat = flattenToMidi1({ clipId: 'c', notes, events: [] });
    const bend = flat.events.find((e) => e.kind === 'pitchBend');
    expect(bend).toBeDefined();
    expect(bend!.startTicks).toBe('5000'); // moved with the note
    expect(flat.losses).toEqual([]);
  });

  it('channel exhaustion produces explicit MpeLoss entries (T71)', () => {
    // 16 overlapping notes > 15 member channels
    const notes = Array.from({ length: 16 }, (_, i) =>
      note(`n${i}`, { startTicks: '0', lengthTicks: '960000', pitch: 40 + i }),
    );
    const flat = flattenToMidi1({ clipId: 'c', notes, events: [] });
    expect(flat.losses).toHaveLength(1);
    expect(flat.losses[0].code).toBe('CHANNEL_EXHAUSTED');
    expect(flat.losses[0].detail).toContain('expression is lost');
    expect(flat.notes).toHaveLength(15); // loser is dropped, not faked
  });

  it('MIDI 2.0 is a declared model only — never transport', () => {
    expect(MIDI2_TRANSPORT).toBe(false);
    const d = midi2Descriptor(
      note('m2', { expression: expr }),
    );
    expect(d.perNoteBend).toHaveLength(2);
    expect(d.perNoteControllers![0].index).toBe(74);
    // zone model stays the standard lower zone: master 0, members 1..15
    expect(STANDARD_ZONE.masterChannel).toBe(0);
    expect(STANDARD_ZONE.memberLow).toBe(1);
    expect(STANDARD_ZONE.memberHigh).toBe(15);
  });
});
