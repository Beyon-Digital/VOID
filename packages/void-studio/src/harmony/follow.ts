// Harmonic follow (TIME-05, T59): an explicit transform that snaps
// SELECTED notes to the chord region governing each note's onset.
//
// Explicit apply is the contract: plan → review → send SetNoteOps under
// one transactionId → undo through the normal UndoOp. Notes outside
// every region — and notes not selected — are never in the plan, so a
// region override cannot touch unrelated accompaniment or original
// notes (TIME-05 acceptance). Pitches beyond `limitSemitones` from the
// nearest chord/scale tone are left alone rather than force-fitted.

import type { CommandReceipt, PersistentOp, VoidClient } from 'void-client';
import { sendWithStaleRetry, describeReceiptError } from '../workspaces/editSession';
import { governingSet } from './constraint';
import { snapPitchToSet } from './constraint';
import type { ChordTrack } from './track';
import type { ConstraintMode } from './constraint';
import { clampGesturePitch } from '../gestures/types';

export interface FollowTarget {
  noteId: string;
  clipId: string;
  pitch: number;
  startTicks: string;
}

export interface FollowChange {
  noteId: string;
  clipId: string;
  fromPitch: number;
  toPitch: number;
}

export interface FollowPlan {
  transactionId: string;
  /** Notes that would change (for preview/diff UI). */
  changes: FollowChange[];
  /** Selected notes skipped — outside all regions or beyond limit. */
  skipped: { noteId: string; reason: 'no-region' | 'beyond-limit' | 'in-tune' }[];
  ops: PersistentOp[];
}

/**
 * Plan the follow transform for the selected notes. `mode` picks chord
 * tones vs implied scale; `limitSemitones` bounds the snap. Pure.
 */
export function planHarmonicFollow(
  notes: FollowTarget[],
  track: ChordTrack,
  opts: { mode?: Exclude<ConstraintMode, 'off'>; limitSemitones?: number } = {},
  mint?: () => string,
): FollowPlan {
  const mode = opts.mode ?? 'chord';
  const limit = opts.limitSemitones ?? 6;
  const spec = { mode, track };
  const changes: FollowChange[] = [];
  const skipped: FollowPlan['skipped'] = [];
  const ops: PersistentOp[] = [];
  for (const n of notes) {
    const set = governingSet(n.startTicks, spec);
    if (!set) {
      skipped.push({ noteId: n.noteId, reason: 'no-region' });
      continue;
    }
    const to = snapPitchToSet(n.pitch, set, limit);
    if (to === n.pitch) {
      skipped.push({ noteId: n.noteId, reason: 'in-tune' });
      continue;
    }
    // beyond-limit check: snapPitchToSet returns input when nothing in
    // range, so `to === n.pitch` already covered; a snap that moved means
    // it was within limit by construction.
    changes.push({
      noteId: n.noteId,
      clipId: n.clipId,
      fromPitch: n.pitch,
      toPitch: clampGesturePitch(to),
    });
    ops.push({
      SetNoteOp: {
        clip_id: n.clipId,
        note_id: n.noteId,
        pitch: clampGesturePitch(to),
      },
    });
  }
  return {
    transactionId: mint ? mint() : '',
    changes,
    skipped,
    ops,
  };
}

/**
 * Apply the plan: each SetNoteOp under the plan's one transaction id —
 * one undoable gesture through the standard path (T59).
 */
export async function applyHarmonicFollow(
  client: VoidClient,
  plan: FollowPlan,
  refresh?: () => Promise<unknown> | unknown,
): Promise<{ receipts: CommandReceipt[]; errorText: string }> {
  const receipts: CommandReceipt[] = [];
  for (const op of plan.ops) {
    const out = await sendWithStaleRetry(client, op, {
      transactionId: plan.transactionId,
      refresh,
    });
    receipts.push(out.receipt);
    const errorText = describeReceiptError(out.receipt);
    if (errorText) return { receipts, errorText };
  }
  return { receipts, errorText: '' };
}

/** Undo the applied transform via the normal path (UndoOp). */
export function undoHarmonicFollow(
  client: VoidClient,
  plan: FollowPlan,
): Promise<CommandReceipt> {
  return client.undo(plan.transactionId);
}
