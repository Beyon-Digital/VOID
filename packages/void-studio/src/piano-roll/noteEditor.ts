// NoteEditor — the piano roll's command lane.
//
// Reads: NOTE_RANGE pages scoped by track + tick window, filtered to the
// selected clip. Writes: InsertNoteOp / SetNoteOp / RemoveNoteOp — the
// schema's only note ops. A "move" is SetNoteOp with the changed fields
// only; -1/omitted fields mean "unchanged" per the FlatBuffers table.
// Velocity edits are SetNoteOp{velocity}. One gesture = one transaction.

import type { PersistentOp, ReadItem, VoidClient } from 'void-client';
import { i64str, parseI64 } from 'void-client';
import {
  sendWithStaleRetry,
  describeReceiptError,
  type EditOutcome,
} from '../workspaces/editSession';
import type { StudioStore } from '../store';
import { makeViewKey } from '../store';
import {
  clampPitch,
  clampVelocity,
  parseNoteItem,
  MIN_NOTE_LENGTH,
  type NoteView,
} from './notes';
import type { NoteEditIntent } from './keyboard';

export interface NoteEditResult extends EditOutcome {
  errorText: string;
}

async function issue(
  client: VoidClient,
  op: PersistentOp,
  transactionId: string,
  refresh?: () => Promise<unknown> | unknown,
): Promise<NoteEditResult> {
  const out = await sendWithStaleRetry(client, op, {
    transactionId,
    refresh,
  });
  return { ...out, errorText: describeReceiptError(out.receipt) };
}

export function insertNoteOp(
  clipId: string,
  noteId: string,
  pitch: number,
  velocity: number,
  startTicks: string,
  lengthTicks: string,
): PersistentOp {
  if (parseI64(lengthTicks) < MIN_NOTE_LENGTH) {
    throw new Error(`note length must be >= ${MIN_NOTE_LENGTH} ticks`);
  }
  return {
    InsertNoteOp: {
      clip_id: clipId,
      note_id: noteId,
      pitch: clampPitch(pitch),
      velocity: clampVelocity(velocity),
      start_ticks: i64str(startTicks),
      length_ticks: i64str(lengthTicks),
    },
  };
}

/** SetNoteOp fields are optional; absent = unchanged on the engine. */
export function setNoteOp(
  clipId: string,
  noteId: string,
  fields: {
    pitch?: number;
    velocity?: number;
    startTicks?: string;
    lengthTicks?: string;
  },
): PersistentOp {
  const op: {
    clip_id: string;
    note_id: string;
    pitch?: number;
    velocity?: number;
    start_ticks?: string;
    length_ticks?: string;
  } = { clip_id: clipId, note_id: noteId };
  if (fields.pitch !== undefined) op.pitch = clampPitch(fields.pitch);
  if (fields.velocity !== undefined) op.velocity = clampVelocity(fields.velocity);
  if (fields.startTicks !== undefined) op.start_ticks = i64str(fields.startTicks);
  if (fields.lengthTicks !== undefined) {
    if (parseI64(fields.lengthTicks) < MIN_NOTE_LENGTH) {
      throw new Error(`note length must be >= ${MIN_NOTE_LENGTH} ticks`);
    }
    op.length_ticks = i64str(fields.lengthTicks);
  }
  return { SetNoteOp: op };
}

export function removeNoteOp(clipId: string, noteId: string): PersistentOp {
  return { RemoveNoteOp: { clip_id: clipId, note_id: noteId } };
}

/** Map a keyboard/pointer edit intent onto note ops for a selection. */
export function intentToNoteOps(
  clipId: string,
  notes: NoteView[],
  intent: NoteEditIntent,
): PersistentOp[] {
  switch (intent.type) {
    case 'noop':
      return [];
    case 'delete':
      return notes.map((n) => removeNoteOp(clipId, n.noteId));
    case 'move': {
      const d = parseI64(intent.dTicks);
      return notes.map((n) =>
        setNoteOp(clipId, n.noteId, {
          startTicks: (parseI64(n.startTicks) + d).toString(10),
          pitch: clampPitch(n.pitch + intent.dPitch),
        }),
      );
    }
    case 'resize-end': {
      const d = parseI64(intent.dTicks);
      return notes.map((n) => {
        const len = parseI64(n.lengthTicks) + d;
        return setNoteOp(clipId, n.noteId, {
          lengthTicks: (len < MIN_NOTE_LENGTH ? MIN_NOTE_LENGTH : len).toString(10),
        });
      });
    }
    case 'velocity':
      return notes.map((n) =>
        setNoteOp(clipId, n.noteId, {
          velocity: clampVelocity(n.velocity + intent.dVelocity),
        }),
      );
  }
}

export class NoteEditor {
  constructor(
    private readonly client: VoidClient,
    private readonly newId: () => string,
  ) {}

  /**
   * Page NOTE_RANGE for the clip's track/window and keep only notes
   * belonging to `clipId` — the engine returns the range, the roll shows
   * one clip's slice of it.
   */
  async loadClipNotes(
    store: StudioStore,
    clip: { clipId: string; trackId: string; startTicks: string; lengthTicks: string },
    opts: { maxPages?: number } = {},
  ): Promise<{ notes: NoteView[]; dropped: number }> {
    const clipEnd = (parseI64(clip.startTicks) + parseI64(clip.lengthTicks)).toString(10);
    const notes: NoteView[] = [];
    let dropped = 0;
    for await (const page of this.client.readViewPages({
      view: 'NOTE_RANGE',
      track_id: clip.trackId,
      start_ticks: clip.startTicks,
      end_ticks: clipEnd,
      maxPages: opts.maxPages,
    })) {
      store
        .getState()
        .actions.mergeReadPage(
          makeViewKey('NOTE_RANGE', clip.trackId, clip.startTicks, clipEnd),
          page,
        );
      for (const item of page.items as ReadItem[]) {
        const n = parseNoteItem(item);
        if (n && n.clipId === clip.clipId) notes.push(n);
        else if (!n) dropped++;
      }
      if (page.error && page.error !== 'NONE') break;
      if (page.done) break;
    }
    return { notes, dropped };
  }

  async insertNote(
    clipId: string,
    pitch: number,
    velocity: number,
    startTicks: string,
    lengthTicks: string,
    transactionId: string,
    refresh?: () => Promise<unknown> | unknown,
  ): Promise<NoteEditResult> {
    const op = insertNoteOp(
      clipId,
      this.newId(),
      pitch,
      velocity,
      startTicks,
      lengthTicks,
    );
    return issue(this.client, op, transactionId, refresh);
  }

  async applyIntent(
    clipId: string,
    notes: NoteView[],
    intent: NoteEditIntent,
    transactionId: string,
    refresh?: () => Promise<unknown> | unknown,
  ): Promise<NoteEditResult[]> {
    const ops = intentToNoteOps(clipId, notes, intent);
    const results: NoteEditResult[] = [];
    for (const op of ops) {
      results.push(await issue(this.client, op, transactionId, refresh));
      // Stop at the first failure — subsequent ops in the same gesture
      // would hide the error the user needs to see.
      if (results[results.length - 1].errorText) break;
    }
    return results;
  }

  async setVelocity(
    clipId: string,
    noteId: string,
    velocity: number,
    transactionId: string,
    refresh?: () => Promise<unknown> | unknown,
  ): Promise<NoteEditResult> {
    return issue(
      this.client,
      setNoteOp(clipId, noteId, { velocity }),
      transactionId,
      refresh,
    );
  }
}
