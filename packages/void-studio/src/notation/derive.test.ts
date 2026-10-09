import { describe, expect, it } from 'vitest';
import { parseI64 } from 'void-client';
import type { ClipView } from '../timeline/geometry';
import type { NoteView } from '../piano-roll/notes';
import { TICKS_PER_QUARTER } from '../viewport';
import {
  clipStartMap,
  exportMusicXml,
  layoutScore,
  measureTicks,
  meterLabel,
  notesToAbsolute,
  quantizeGridTicks,
  quantizeTicks,
  spellPitch,
  staffStepIndex,
  type ScoreNote,
} from './derive';

const TPQ = TICKS_PER_QUARTER;
const BAR = 4n * TPQ;

const clip = (clipId: string, startTicks: bigint): ClipView =>
  ({
    clipId,
    trackId: 'trk1',
    kind: 'midi',
    name: clipId,
    color: '#000',
    startTicks: startTicks.toString(10),
    lengthTicks: BAR.toString(10),
    offsetTicks: '0',
  }) as ClipView;

const note = (
  noteId: string,
  clipId: string,
  pitch: number,
  startTicks: bigint,
  lengthTicks: bigint,
  velocity = 100,
): NoteView => ({
  noteId,
  clipId,
  pitch,
  velocity,
  startTicks: startTicks.toString(10),
  lengthTicks: lengthTicks.toString(10),
});

const METER_44 = { beatsPerBar: 4, beatUnit: 4 } as const;

describe('notesToAbsolute', () => {
  it('carries timeline-absolute onsets through verbatim', () => {
    const clips = clipStartMap([clip('c1', BAR), clip('c2', 2n * BAR)]);
    const { notes, dropped } = notesToAbsolute(
      [note('n1', 'c1', 60, 10n, 20n), note('n2', 'c2', 62, 30n, 40n)],
      clips,
    );
    expect(dropped).toBe(0);
    expect(notes.map((n) => n.absStartTicks)).toEqual([10n, 30n]);
  });

  it('drops notes whose clip is not part of the source set', () => {
    const clips = clipStartMap([clip('c1', 0n)]);
    const { notes, dropped } = notesToAbsolute(
      [note('n1', 'c1', 60, 0n, 10n), note('ghost', 'other', 60, 0n, 10n)],
      clips,
    );
    expect(notes).toHaveLength(1);
    expect(dropped).toBe(1);
  });
});

describe('meter + display quantize', () => {
  it('computes measure ticks for 4/4, 3/4, and 6/8', () => {
    expect(measureTicks({ beatsPerBar: 4, beatUnit: 4 })).toBe(4n * TPQ);
    expect(measureTicks({ beatsPerBar: 3, beatUnit: 4 })).toBe(3n * TPQ);
    expect(measureTicks({ beatsPerBar: 6, beatUnit: 8 })).toBe(3n * TPQ);
    expect(measureTicks({ beatsPerBar: 12, beatUnit: 8 })).toBe(6n * TPQ);
  });

  it('meterLabel renders the fraction', () => {
    expect(meterLabel({ beatsPerBar: 6, beatUnit: 8 })).toBe('6/8');
  });

  it('quantizeTicks rounds to the nearest cell, ties away from zero', () => {
    const grid = quantizeGridTicks('1/16')!; // TPQ/4
    expect(quantizeTicks(grid + 1n, grid)).toBe(grid);
    expect(quantizeTicks(grid / 2n, grid)).toBe(grid); // tie -> up
    expect(quantizeTicks(grid / 2n - 1n, grid)).toBe(0n);
    expect(quantizeTicks(12345n, null)).toBe(12345n);
  });
});

describe('layoutScore', () => {
  const src: ScoreNote[] = [
    {
      noteId: 'a', clipId: 'c1', pitch: 60, velocity: 90,
      absStartTicks: 0n, durationTicks: TPQ,
    },
    {
      noteId: 'b', clipId: 'c1', pitch: 64, velocity: 90,
      absStartTicks: TPQ, durationTicks: BAR, // crosses barline -> tied
    },
    {
      noteId: 'c', clipId: 'c1', pitch: 62, velocity: 90,
      absStartTicks: TPQ, durationTicks: TPQ, // D4 = a second below E4
    },
  ];

  it('splits a note crossing a barline into tied segments', () => {
    const { measures, measureLenTicks } = layoutScore(src, {
      meter: METER_44, quantize: 'off',
    });
    expect(measureLenTicks).toBe(BAR);
    // 'b' starts at beat 2 of bar 1 and lasts a whole bar: bars 1-2.
    expect(measures).toHaveLength(2);
    const m1 = measures[0];
    const bSeg = m1.segments.find((s) => s.noteId === 'b')!;
    expect(bSeg.segOffsetTicks).toBe(TPQ);
    expect(bSeg.segDurationTicks).toBe(BAR - TPQ);
    expect(bSeg.tieStart).toBe(true);
    const bCont = measures[1].segments.find((s) => s.noteId === 'b')!;
    expect(bCont.tieStop).toBe(true);
    expect(bCont.tieStart).toBe(false);
    expect(bCont.segDurationTicks).toBe(TPQ);
  });

  it('marks second-interval collisions inside a chord', () => {
    const { measures } = layoutScore(src, { meter: METER_44, quantize: 'off' });
    const atOnset = measures[0].segments.filter(
      (s) => s.segOffsetTicks === TPQ,
    );
    const b = atOnset.find((s) => s.noteId === 'b')!; // E4 (step 0)
    const c = atOnset.find((s) => s.noteId === 'c')!; // D4 (step -1)
    expect(b.xShift).toBe(0);
    expect(c.xShift).toBe(1);
  });

  it('UI-T34: display quantization never rewrites performance ticks', () => {
    const swung = [
      {
        noteId: 's1', clipId: 'c1', pitch: 60, velocity: 80,
        absStartTicks: TPQ + 12345n, durationTicks: 234567n,
      },
    ];
    const off = layoutScore(swung, { meter: METER_44, quantize: 'off' });
    const q16 = layoutScore(swung, { meter: METER_44, quantize: '1/16' });
    const rawOff = off.measures[0].segments[0];
    const rawQ = q16.measures[0].segments[0];
    // Raw performance ticks are identical under either display grid.
    expect(rawQ.rawStartTicks).toBe(TPQ + 12345n);
    expect(rawQ.rawDurationTicks).toBe(234567n);
    expect(rawOff.rawStartTicks).toBe(rawQ.rawStartTicks);
    // Display offsets do move: onset snapped to the 16th grid.
    expect(rawOff.segOffsetTicks).toBe(TPQ + 12345n);
    expect(rawQ.segOffsetTicks).toBe(
      quantizeTicks(TPQ + 12345n, quantizeGridTicks('1/16')),
    );
  });

  it('positions notes outside a clip window are clamped defensively', () => {
    const { measures } = layoutScore(
      [
        {
          noteId: 'neg', clipId: 'c', pitch: 60, velocity: 1,
          absStartTicks: -5n, durationTicks: 10n,
        },
      ],
      { meter: METER_44, quantize: 'off' },
    );
    expect(measures.every((m) => m.empty)).toBe(true);
  });
});

describe('spellPitch + staffStepIndex', () => {
  it('spells MIDI 61 as C#4 sharps / Db4 flats', () => {
    expect(spellPitch(61, 'sharps')).toEqual({ step: 'c', alter: 1, octave: 4 });
    expect(spellPitch(61, 'flats')).toEqual({ step: 'd', alter: -1, octave: 4 });
  });

  it('maps E4 to staff step 0 (treble bottom line) and G5 above the staff', () => {
    expect(staffStepIndex(spellPitch(64))).toBe(0); // E4 bottom line
    expect(staffStepIndex(spellPitch(67))).toBe(2); // G4 second line
    expect(staffStepIndex(spellPitch(77))).toBe(8); // F5 top line
    expect(staffStepIndex(spellPitch(60))).toBe(-2); // C4 ledger below
  });
});

describe('exportMusicXml', () => {
  const partNotes: ScoreNote[] = [
    {
      noteId: 'n-1', clipId: 'c1', pitch: 60, velocity: 96,
      absStartTicks: 0n, durationTicks: BAR, // whole note
    },
    {
      noteId: 'n-2', clipId: 'c1', pitch: 64, velocity: 90,
      absStartTicks: BAR, durationTicks: BAR + TPQ, // tied across barline
    },
    {
      noteId: 'n-3', clipId: 'c1', pitch: 62, velocity: 88,
      absStartTicks: BAR, durationTicks: TPQ, // overlap, not a chord (different dur handled via voice)
    },
  ];

  it('emits score-partwise with real durations, ids, ties and voices', () => {
    const xml = exportMusicXml({
      name: 'Lead & Keys',
      meter: METER_44,
      notes: partNotes,
    });
    expect(xml).toContain('<score-partwise version="4.0">');
    expect(xml).toContain('<divisions>960000</divisions>');
    expect(xml).toContain(`<part-name>Lead &amp; Keys</part-name>`);
    // Real durations: whole-note duration == 3840000 ticks.
    expect(xml).toContain('<duration>3840000</duration>');
    // Engine note ids travel as <note id="...">.
    expect(xml).toContain('<note id="n-1"');
    expect(xml).toContain('<note id="n-2"');
    // Tie across bar 2 -> 3.
    expect(xml).toContain('<tie type="start"/>');
    expect(xml).toContain('<tie type="stop"/>');
    expect(xml).toContain('<tied type="start"/>');
    // Same onset but different durations -> separate voices, no chord.
    expect(xml).toContain('<backup>');
    expect(xml).toContain('<voice>2</voice>');
    expect(xml).not.toContain('<chord/>');
    // Velocity rides along in the note attribute.
    expect(xml).toContain('velocity="96"');
    // Measures count = 3 (bar 1, 2, 3).
    expect(xml.match(/<measure number=/g)).toHaveLength(3);
  });

  it('emits measure rests for empty interior measures', () => {
    const xml = exportMusicXml({
      name: 'S',
      meter: METER_44,
      notes: [
        {
          noteId: 'n', clipId: 'c', pitch: 60, velocity: 80,
          absStartTicks: 0n, durationTicks: TPQ,
        },
        {
          noteId: 'n2', clipId: 'c', pitch: 60, velocity: 80,
          absStartTicks: 2n * BAR, durationTicks: TPQ,
        },
      ],
    });
    expect(xml).toContain('<rest measure="yes"/>');
  });

  it('tags chord mates with <chord/> at a shared onset', () => {
    const xml = exportMusicXml({
      name: 'Ch',
      meter: METER_44,
      notes: [
        { noteId: 'p', clipId: 'c', pitch: 60, velocity: 80, absStartTicks: 0n, durationTicks: TPQ },
        { noteId: 'q', clipId: 'c', pitch: 64, velocity: 80, absStartTicks: 0n, durationTicks: TPQ },
      ],
    });
    expect(xml).toContain('<chord/>');
  });

  it('spells flats in the export when requested', () => {
    const xml = exportMusicXml({
      name: 'F',
      meter: METER_44,
      spelling: 'flats',
      notes: [
        { noteId: 'f', clipId: 'c', pitch: 61, velocity: 80, absStartTicks: 0n, durationTicks: TPQ },
      ],
    });
    expect(xml).toContain('<step>D</step><alter>-1</alter>');
  });
});

describe('i64 wire shape', () => {
  it('round-trips tick strings through parseI64', () => {
    expect(parseI64('960000')).toBe(TPQ);
  });
});
