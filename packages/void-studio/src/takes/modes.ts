// Recording modes + step input + note repeat (W17/REC-03; T66).
//
// Wire reality: protocol major.1 has NO arm/record ops at all (NEEDS
// §7) — the take lifecycle is engine-internal. What the studio owns is
// the *semantics layer*: which mode is armed, what a take placement
// means for existing material, and how step/repeat input materializes
// as ordinary note ops under one transaction.
//
// Source retention rule (REC-03): 'replace' removes overlapping CLIPS
// from the arrangement; it never deletes take folders or their source
// assets — a replaced recording remains in its folder and re-appears
// in the comp editor.

import { i64str, parseI64 } from 'void-client';
import type { PersistentOp } from 'void-client';
import type { ClipView } from '../timeline/geometry';
import { regionsOverlap, type Region } from './takes';

export type RecordingMode = 'overdub' | 'replace' | 'loop' | 'step';

/** What a new recording pass does to material already on the track
 * inside `punch`. Pure decision — the caller turns `removeClipIds`
 * into RemoveClipOps inside the recording transaction. */
export interface PlacementPlan {
  mode: RecordingMode;
  /** Clips the new pass replaces inside the punch region. */
  removeClipIds: string[];
  /** True when the pass lands as a new take lane (loop/overdub). */
  createsLane: boolean;
}

export function planPlacement(
  mode: RecordingMode,
  punch: Region,
  existing: ClipView[],
): PlacementPlan {
  const hit = existing.filter((c) =>
    regionsOverlap({ startTicks: c.startTicks, lengthTicks: c.lengthTicks }, punch),
  );
  switch (mode) {
    case 'replace':
      // Replace = remove intersecting clips in the punch region; take
      // folders/sources are untouched.
      return { mode, removeClipIds: hit.map((c) => c.clipId), createsLane: false };
    case 'overdub':
    case 'loop':
      // Overdub and loop both keep existing material and add a lane;
      // loop's per-pass lane semantics live in loopTakes().
      return { mode, removeClipIds: [], createsLane: true };
    case 'step':
      // Step input edits notes inside an existing clip — no clip ops.
      return { mode, removeClipIds: [], createsLane: false };
  }
}

/** One step-entry note: explicit position, length, pitch, velocity. */
export interface StepNote {
  pitch: number;
  velocity: number;
  startTicks: string;
  lengthTicks: string;
}

/**
 * Step input (REC-03): a batch of InsertNoteOps into one clip under
 * one transaction — the keyboard/numeric entry path, same undo unit
 * as any other edit gesture.
 */
export function stepInputOps(
  clipId: string,
  notes: StepNote[],
  mint: () => string,
): { transactionId: string; noteIds: string[]; ops: PersistentOp[] } {
  const noteIds = notes.map(() => mint());
  return {
    transactionId: mint(),
    noteIds,
    ops: notes.map((n, i) => ({
      InsertNoteOp: {
        clip_id: clipId,
        note_id: noteIds[i],
        pitch: Math.min(127, Math.max(0, Math.round(n.pitch))),
        velocity: Math.min(127, Math.max(1, Math.round(n.velocity))),
        start_ticks: i64str(n.startTicks),
        length_ticks: i64str(n.lengthTicks),
      },
    })),
  };
}

/**
 * Note-repeat spec (REC-03 "configurable note-repeat clock"): a held
 * pad re-fires `note` on every `gridTicks` boundary for `count` hits.
 * Pure render — the timing source is the musical grid in ticks, never
 * wall-clock setInterval.
 */
export function repeatNotes(args: {
  note: StepNote;
  gridTicks: string;
  count: number;
}): StepNote[] {
  const grid = parseI64(args.gridTicks);
  if (grid <= 0n) throw new Error('note-repeat grid must be > 0 ticks');
  const count = Math.max(0, Math.round(args.count));
  const start = parseI64(args.note.startTicks);
  return Array.from({ length: count }, (_, i) => ({
    ...args.note,
    startTicks: i64str(start + BigInt(i) * grid),
    lengthTicks:
      parseI64(args.note.lengthTicks) > grid
        ? i64str(grid)
        : args.note.lengthTicks,
  }));
}
