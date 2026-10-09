// S24 score screen — model tests (UI-T34 evidence at app level).

import { describe, expect, it } from 'vitest';
import type { ReadItem } from 'void-client';
import type { ClipView, NoteView } from 'void-studio';
import {
  deriveLayout,
  deriveScoreNotes,
  exportFileName,
  isMidiClip,
  ledgerSteps,
  midiForStaffStep,
  noteHeadY,
  offsetX,
  partsFromTrackItems,
  passageInfo,
  CLEF_W,
  MEASURE_WIDTH,
  STAFF_HEIGHT,
} from './model';

const item = (objectId: string, summary: Record<string, unknown>): ReadItem =>
  ({ object_id: objectId, summary_json: JSON.stringify(summary) }) as ReadItem;

const clip = (clipId: string, kind: string, start: string): ClipView =>
  ({
    clipId,
    trackId: 't1',
    kind,
    name: clipId,
    color: '#000',
    startTicks: start,
    lengthTicks: '3840000',
    offsetTicks: '0',
  }) as ClipView;

const note = (id: string, clipId: string, pitch: number, start: string, len: string): NoteView => ({
  noteId: id,
  clipId,
  pitch,
  velocity: 90,
  startTicks: start,
  lengthTicks: len,
});

describe('partsFromTrackItems', () => {
  it('projects TRACK_LIST items to part rows; tolerates bad JSON', () => {
    const parts = partsFromTrackItems([
      item('o1', { track_id: 't1', name: 'Keys', kind: 'midi', muted: false }),
      { object_id: 'o2', summary_json: 'not-json' } as ReadItem,
    ]);
    expect(parts[0]).toEqual({ trackId: 't1', name: 'Keys', kind: 'midi', muted: false });
    expect(parts[1].trackId).toBe('o2');
  });
});

describe('derive pipeline (UI-T34)', () => {
  const clips = [clip('c1', 'midi', '0'), clip('a1', 'audio', '3840000')];
  const notes = [
    note('n1', 'c1', 60, '960000', '960000'),
    note('n2', 'c1', 64, '960000', '960000'), // chord mate
    note('n3', 'c1', 67, '2012345', '480000'), // swung onset
  ];

  it('filters non-MIDI clips via isMidiClip', () => {
    expect(clips.filter(isMidiClip).map((c) => c.clipId)).toEqual(['c1']);
  });

  it('keeps engine note ids as element ids (bidirectional roll selection)', () => {
    const { notes: sn } = deriveScoreNotes(notes, clips);
    const layout = deriveLayout(sn, { beatsPerBar: 4, beatUnit: 4 }, '1/16', 'sharps');
    const ids = layout.measures.flatMap((m) => m.segments.map((s) => s.noteId));
    expect(ids).toContain('n1');
    expect(ids).toContain('n3');
  });

  it('display quantize moves drawn offsets but never raw ticks', () => {
    const { notes: sn } = deriveScoreNotes(notes, clips);
    const off = deriveLayout(sn, { beatsPerBar: 4, beatUnit: 4 }, 'off', 'sharps');
    const q = deriveLayout(sn, { beatsPerBar: 4, beatUnit: 4 }, '1/8', 'sharps');
    const segOff = off.measures[0].segments.find((s) => s.noteId === 'n3')!;
    const segQ = q.measures[0].segments.find((s) => s.noteId === 'n3')!;
    expect(segOff.segOffsetTicks).toBe(2012345n);
    expect(segQ.segOffsetTicks).toBe(1920000n); // snapped to the 1/8 grid
    expect(segQ.rawStartTicks).toBe(2012345n); // performance untouched
  });
});

describe('staff geometry', () => {
  it('maps staff steps to y (bottom line = staff height)', () => {
    expect(noteHeadY(0)).toBe(STAFF_HEIGHT);
    expect(noteHeadY(8)).toBe(0); // top line
  });

  it('requests ledger lines only beyond the staff', () => {
    expect(ledgerSteps(0)).toEqual([]);
    expect(ledgerSteps(8)).toEqual([]);
    expect(ledgerSteps(9)).toEqual([]); // space above — no ledger
    expect(ledgerSteps(10)).toEqual([10]); // A5 ledger
    expect(ledgerSteps(-1)).toEqual([]); // D4 space — no ledger
    expect(ledgerSteps(-2)).toEqual([-2]); // C4 ledger
    expect(ledgerSteps(-4)).toEqual([-2, -4]); // A3
  });

  it('maps staff steps back to diatonic MIDI pitches', () => {
    expect(midiForStaffStep(0)).toBe(64); // E4
    expect(midiForStaffStep(-2)).toBe(60); // C4
    expect(midiForStaffStep(8)).toBe(77); // F5
    expect(midiForStaffStep(4)).toBe(71); // B4
  });

  it('offsets notes inside the measure after the clef pad', () => {
    expect(offsetX(0n, 3840000n)).toBe(CLEF_W);
    expect(offsetX(1920000n, 3840000n)).toBeCloseTo(
      CLEF_W + (MEASURE_WIDTH - CLEF_W - 8) / 2,
    );
  });
});

describe('passageInfo', () => {
  const { notes: sn } = deriveScoreNotes(
    [note('a', 'c1', 60, '0', '960000'), note('b', 'c1', 64, '3840000', '960000')],
    [clip('c1', 'midi', '0')],
  );
  const layout = deriveLayout(sn, { beatsPerBar: 4, beatUnit: 4 }, '1/16', 'sharps');

  it('reports bar range + count for the selection', () => {
    expect(passageInfo(['a', 'b'], layout)).toEqual({ bars: '1–2', count: 2 });
    expect(passageInfo(['b'], layout)).toEqual({ bars: '2', count: 1 });
    expect(passageInfo([], layout)).toEqual({ bars: null, count: 0 });
    expect(passageInfo(['missing'], layout)).toEqual({ bars: null, count: 0 });
  });
});

describe('exportFileName', () => {
  it('slugifies the part name', () => {
    expect(exportFileName('Lead & Keys')).toBe('lead-keys.musicxml');
    expect(exportFileName('')).toBe('part.musicxml');
  });
});
