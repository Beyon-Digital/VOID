// T37 — piano roll + controller lanes: note op payload shapes through
// FakeTransport, note-grid geometry/hit-testing, velocity-lane math,
// keyboard note entry, and NOTE_RANGE paging.

import { describe, expect, it } from 'vitest';
import { FakeTransport, VoidClient } from 'void-client';
import type { PersistentCommandDto } from 'void-client';
import { makeViewport } from '../viewport';
import { createStudioStore, makeViewKey } from '../store';
import {
  clampPitch,
  clampVelocity,
  hitTestNote,
  hitTestNotes,
  isBlackKey,
  noteRect,
  parseNoteItem,
  pitchName,
  pitchRow,
  type NoteView,
} from './notes';
import { noteEditIntent, pitchForKey } from './keyboard';
import {
  adjustedVelocity,
  velocityAtPy,
  velocityBarPx,
  velocityBars,
  velocityHit,
} from './velocity';
import { NoteEditor, insertNoteOp, intentToNoteOps, setNoteOp } from './noteEditor';
import type { SnapSettings } from '../workspaces/editorStore';

const SNAP: SnapSettings = {
  enabled: true,
  division: 'beat',
  beatsPerBar: 4,
  tempoMapRevision: '1',
};

const note = (over: Partial<NoteView> = {}): NoteView => ({
  noteId: 'n-1',
  clipId: 'clip-1',
  pitch: 60,
  velocity: 96,
  startTicks: '3840000',
  lengthTicks: '960000',
  ...over,
});

// -- note model / geometry ----------------------------------------------------

describe('note model + grid geometry', () => {
  const v = makeViewport('0', '15360000');
  const z = { ticksPerPixel: 24000 };
  const ROW = 14;

  it('parses the wire shape; rejects out-of-range and malformed items', () => {
    const good = parseNoteItem({
      object_id: 'note:n-9',
      summary_json: JSON.stringify({
        note_id: 'n-9',
        clip_id: 'clip-1',
        pitch: 64,
        velocity: 100,
        start_ticks: '0',
        length_ticks: '480000',
      }),
    });
    expect(good).toMatchObject({ noteId: 'n-9', clipId: 'clip-1', pitch: 64 });
    expect(
      parseNoteItem({
        object_id: 'x',
        summary_json: JSON.stringify({
          note_id: 'n',
          clip_id: 'c',
          pitch: 200,
          velocity: 100,
          start_ticks: '0',
          length_ticks: '1',
        }),
      }),
    ).toBeNull(); // pitch out of range
    expect(
      parseNoteItem({
        object_id: 'x',
        summary_json: JSON.stringify({
          note_id: 'n',
          clip_id: 'c',
          pitch: 60,
          velocity: 0,
          start_ticks: '0',
          length_ticks: '1',
        }),
      }),
    ).toBeNull(); // velocity must be >= 1
    expect(parseNoteItem({ object_id: 'x', summary_json: '{bad' })).toBeNull();
  });

  it('projects a note to its px rect (row 0 = pitch 127)', () => {
    const n = note({ pitch: 127 });
    const r = noteRect(n, v, z, ROW);
    expect(r.y).toBe(0);
    expect(r.x).toBeCloseTo(3840000 / 24000, 3);
    expect(r.h).toBe(ROW);
    expect(pitchRow(60)).toBe(127 - 60);
  });

  it('hit-tests body vs resize-end edge', () => {
    const n = note({ pitch: 60 });
    const r = noteRect(n, v, z, ROW);
    const py = r.y + 2;
    expect(hitTestNote(n, r.x + 2, py, v, z, ROW)).toBe('body');
    expect(hitTestNote(n, r.x + r.w - 1, py, v, z, ROW)).toBe('resize-end');
    expect(hitTestNote(n, r.x - 30, py, v, z, ROW)).toBe(null);
    // wrong row → no hit
    expect(hitTestNote(n, r.x + 2, py + ROW * 4, v, z, ROW)).toBe(null);
    expect(hitTestNotes([n], r.x + 2, py, v, z, ROW)?.note.noteId).toBe('n-1');
  });

  it('clamps pitch/velocity and names pitches', () => {
    expect(clampPitch(200)).toBe(127);
    expect(clampPitch(-5)).toBe(0);
    expect(clampVelocity(0)).toBe(1);
    expect(clampVelocity(300)).toBe(127);
    expect(pitchName(60)).toBe('C4');
    expect(pitchName(61)).toBe('C#4');
    expect(isBlackKey(61)).toBe(true);
    expect(isBlackKey(60)).toBe(false);
  });
});

// -- keyboard entry -----------------------------------------------------------

describe('keyboard note entry + edit intents', () => {
  it('maps the DAW typing rows to semitones above base', () => {
    expect(pitchForKey('z', 60)).toBe(60); // C at base
    expect(pitchForKey('s', 60)).toBe(61);
    expect(pitchForKey('m', 60)).toBe(71);
    expect(pitchForKey('q', 60)).toBe(72); // upper row = +12
    expect(pitchForKey('l', 60)).toBe(null); // unmapped
    expect(pitchForKey('m', 125)).toBe(127); // clamps at the top
  });

  it('arrows move by a grid step or semitone; alt resizes; +/- velocity', () => {
    expect(noteEditIntent('ArrowRight', {}, SNAP)).toEqual({
      type: 'move',
      dTicks: '960000',
      dPitch: 0,
    });
    expect(noteEditIntent('ArrowLeft', { alt: true }, SNAP)).toEqual({
      type: 'resize-end',
      dTicks: '-960000',
    });
    expect(noteEditIntent('ArrowUp', {}, SNAP)).toEqual({
      type: 'move',
      dTicks: '0',
      dPitch: 1,
    });
    expect(noteEditIntent('ArrowDown', { shift: true }, SNAP)).toEqual({
      type: 'move',
      dTicks: '0',
      dPitch: -12,
    });
    expect(noteEditIntent('=', {}, SNAP)).toEqual({ type: 'velocity', dVelocity: 8 });
    expect(noteEditIntent('Delete', {}, SNAP)).toEqual({ type: 'delete' });
    expect(noteEditIntent('F5', {}, SNAP)).toEqual({ type: 'noop' });
  });
});

// -- velocity lane --------------------------------------------------------------

describe('velocity lane', () => {
  it('bar height scales to the lane; pointer y maps back to 1..127', () => {
    expect(velocityBarPx(127, 56)).toBe(56);
    expect(velocityBarPx(1, 56)).toBe(1);
    expect(velocityAtPy(0, 56)).toBe(127);
    expect(velocityAtPy(56, 56)).toBe(1);
    expect(velocityAtPy(28, 56)).toBe(64);
    expect(adjustedVelocity(120, 8)).toBe(127);
  });

  it('locates the note whose bar spans px', () => {
    const bars = velocityBars([note()], 56);
    expect(bars[0].velocity).toBe(96);
    const hit = velocityHit([{ note: note(), x: 100, w: 40 }], 120);
    expect(hit?.noteId).toBe('n-1');
    expect(velocityHit([{ note: note(), x: 100, w: 40 }], 10)).toBeNull();
  });
});

// -- op payloads ----------------------------------------------------------------

describe('note op payload shapes', () => {
  it('InsertNoteOp carries clip_id, note_id, pitch, velocity, ticks', () => {
    expect(insertNoteOp('clip-1', 'n-1', 60, 100, '3840000', '960000')).toEqual({
      InsertNoteOp: {
        clip_id: 'clip-1',
        note_id: 'n-1',
        pitch: 60,
        velocity: 100,
        start_ticks: '3840000',
        length_ticks: '960000',
      },
    });
    expect(() => insertNoteOp('c', 'n', 60, 100, '0', '0')).toThrow(/length/);
  });

  it('SetNoteOp only ships changed fields (unset = unchanged)', () => {
    expect(setNoteOp('clip-1', 'n-1', { pitch: 72 })).toEqual({
      SetNoteOp: { clip_id: 'clip-1', note_id: 'n-1', pitch: 72 },
    });
    expect(setNoteOp('clip-1', 'n-1', { velocity: 40, startTicks: '960000' })).toEqual({
      SetNoteOp: {
        clip_id: 'clip-1',
        note_id: 'n-1',
        velocity: 40,
        start_ticks: '960000',
      },
    });
  });

  it('a move intent becomes SetNoteOp with start_ticks+pitch per note', () => {
    const ops = intentToNoteOps('clip-1', [note()], {
      type: 'move',
      dTicks: '960000',
      dPitch: 12,
    });
    expect(ops).toEqual([
      {
        SetNoteOp: {
          clip_id: 'clip-1',
          note_id: 'n-1',
          pitch: 72,
          start_ticks: '4800000',
        },
      },
    ]);
  });

  it('a delete intent becomes RemoveNoteOp; velocity intent clamps', () => {
    expect(
      intentToNoteOps('clip-1', [note()], { type: 'delete' }),
    ).toEqual([{ RemoveNoteOp: { clip_id: 'clip-1', note_id: 'n-1' } }]);
    expect(
      intentToNoteOps('clip-1', [note({ velocity: 124 })], {
        type: 'velocity',
        dVelocity: 8,
      }),
    ).toEqual([
      { SetNoteOp: { clip_id: 'clip-1', note_id: 'n-1', velocity: 127 } },
    ]);
    expect(
      intentToNoteOps('clip-1', [note({ lengthTicks: '960000' })], {
        type: 'resize-end',
        dTicks: '-9990000',
      }),
    ).toEqual([
      {
        SetNoteOp: {
          clip_id: 'clip-1',
          note_id: 'n-1',
          length_ticks: '1',
        },
      },
    ]);
  });
});

// -- editor round-trips through FakeTransport --------------------------------------

describe('NoteEditor over the wire', () => {
  it('insertNote sends exactly one send_command with the generated note_id', async () => {
    const transport = new FakeTransport();
    transport.respond('send_command', (args) => {
      const dto = args!.dto as PersistentCommandDto;
      return {
        kind: 'CommandReceipt',
        command_id: dto.command_id,
        transaction_id: dto.transaction_id,
        status: 'APPLIED',
        error: 'NONE',
        revision: '4',
      };
    });
    const client = new VoidClient({ transport, ids: () => 'fixed-id' });
    const ed = new NoteEditor(client, () => 'note-new');
    const res = await ed.insertNote('clip-1', 64, 100, '0', '960000', 'tx-1');
    expect(res.receipt.status).toBe('APPLIED');
    const dto = transport.calls[0].args!.dto as PersistentCommandDto;
    expect(dto.transaction_id).toBe('tx-1');
    expect(dto.op).toEqual({
      InsertNoteOp: {
        clip_id: 'clip-1',
        note_id: 'note-new',
        pitch: 64,
        velocity: 100,
        start_ticks: '0',
        length_ticks: '960000',
      },
    });
  });

  it('setVelocity sends SetNoteOp{velocity} only', async () => {
    const transport = new FakeTransport();
    transport.respond('send_command', (args) => ({
      kind: 'CommandReceipt',
      command_id: (args!.dto as PersistentCommandDto).command_id,
      status: 'APPLIED',
      error: 'NONE',
      revision: '5',
    }));
    const client = new VoidClient({ transport, ids: () => 'id' });
    const ed = new NoteEditor(client, () => 'x');
    await ed.setVelocity('clip-1', 'n-1', 33, 'tx-vel');
    const dto = transport.calls[0].args!.dto as PersistentCommandDto;
    expect(dto.op).toEqual({
      SetNoteOp: { clip_id: 'clip-1', note_id: 'n-1', velocity: 33 },
    });
  });

  it('loadClipNotes pages NOTE_RANGE scoped to the clip window and filters clip_id', async () => {
    const transport = new FakeTransport();
    transport.respond('read_view', (args) => {
      const dto = args!.dto as { view: string; track_id: string; start_ticks: string; end_ticks: string };
      expect(dto.view).toBe('NOTE_RANGE');
      expect(dto.track_id).toBe('trk-1');
      expect(dto.start_ticks).toBe('3840000');
      expect(dto.end_ticks).toBe('4800000');
      return {
        kind: 'ReadResponse',
        request_id: 'r',
        revision: '6',
        items: [
          {
            object_id: 'note:a',
            summary_json: JSON.stringify({
              note_id: 'a',
              clip_id: 'clip-1',
              pitch: 60,
              velocity: 90,
              start_ticks: '3840000',
              length_ticks: '240000',
            }),
          },
          {
            object_id: 'note:other',
            summary_json: JSON.stringify({
              note_id: 'other',
              clip_id: 'clip-2', // different clip — filtered out
              pitch: 60,
              velocity: 90,
              start_ticks: '3840000',
              length_ticks: '240000',
            }),
          },
          { object_id: 'junk', summary_json: '{bad' },
        ],
        next_cursor: '',
        done: true,
        error: 'NONE',
      };
    });
    const client = new VoidClient({ transport, ids: () => 'id' });
    const ed = new NoteEditor(client, () => 'x');
    const store = createStudioStore();
    const { notes, dropped } = await ed.loadClipNotes(store, {
      clipId: 'clip-1',
      trackId: 'trk-1',
      startTicks: '3840000',
      lengthTicks: '960000',
    });
    expect(notes.map((n) => n.noteId)).toEqual(['a']);
    expect(dropped).toBe(1);
    expect(
      store.getState().views[
        makeViewKey('NOTE_RANGE', 'trk-1', '3840000', '4800000')
      ].done,
    ).toBe(true);
  });
});
