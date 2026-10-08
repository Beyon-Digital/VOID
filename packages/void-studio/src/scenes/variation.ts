// Scene capture + pattern-variation conversion (W17/PAT-04, PAT-05;
// T67).
//
// - capture-to-arrangement: a running session's played scenes are
//   recorded as a capture record (scene launch events + per-slot
//   content); `captureToArrangement` turns it into real clip ops —
//   one transaction inserting clips at the arrangement positions the
//   capture spanned. Audio-cell capture materializes only when the
//   cell carries a rendered asset id (a live audio record is an
//   engine job — NEEDS).
// - pattern→MIDI: reuses patterns/patternToInsertOps (InsertNoteOp
//   batch under one transaction) — a pattern slot variation becomes
//   a real MIDI clip on the arrangement.

import { i64str, parseI64 } from 'void-client';
import type { PersistentOp } from 'void-client';
import type { ClipView } from '../timeline/geometry';
import { patternToInsertOps, type StepPattern } from '../patterns/stepPattern';
import type { LaunchQuantize, SceneSlot } from './scenes';
import { quantizeLaunchAt } from './scenes';

/** A recorded launch event: scene fired at arrangement tick
 * (post-quantize). */
export interface CaptureEvent {
  sceneId: string;
  atTicks: string;
  slots: { trackId: string; content: SceneSlot['content'] }[];
}

/** The capture record of a session performance. */
export interface CaptureRecord {
  captureId: string;
  startedAtTicks: string;
  events: CaptureEvent[];
}

export function makeCapture(captureId: string, startedAtTicks: string): CaptureRecord {
  return { captureId, startedAtTicks, events: [] };
}

export function recordEvent(rec: CaptureRecord, e: CaptureEvent): CaptureRecord {
  return { ...rec, events: [...rec.events, e] };
}

/**
 * Turn a capture into arrangement clips. Only slots whose content
 * already references materialized media become clips:
 * - kind 'clip' → re-insert the referenced clip's spec at the launch
 *   position (callers resolve the clip view via `resolveClip`).
 * - kind 'pattern' → needs a rendered asset to place as audio; a
 *   raw pattern reference yields a MIDI clip + InsertNoteOps when the
 *   caller supplies `resolvePattern` (the honest path for PAT-05).
 * Anything else is dropped — no invented media.
 */
export function captureToArrangementOps(
  rec: CaptureRecord,
  opts: {
    resolveClip: (clipId: string) => ClipView | undefined;
    resolvePattern?: (patternId: string) => StepPattern | undefined;
    /** Clip length for pattern cells (the pattern cycle or caller's
     * scene span). Required for pattern cells. */
    patternClipLengthTicks?: string;
  },
  mint: () => string,
): {
  transactionId: string;
  ops: PersistentOp[];
  /** Cells skipped because they carried no materialized media. */
  skipped: { trackId: string; kind: string }[];
  /** clip_id → note ids inserted into it (pattern cells). */
  noteIdsByClip: Record<string, string[]>;
} {
  const transactionId = mint();
  const ops: PersistentOp[] = [];
  const skipped: { trackId: string; kind: string }[] = [];
  const noteIdsByClip: Record<string, string[]> = {};
  for (const e of rec.events) {
    for (const s of e.slots) {
      if (s.content.kind === 'clip') {
        const clip = opts.resolveClip(s.content.clipId);
        if (clip === undefined) {
          skipped.push({ trackId: s.trackId, kind: 'clip' });
          continue;
        }
        const clipId = mint();
        if (clip.kind === 'AUDIO') {
          if (!clip.assetId) {
            skipped.push({ trackId: s.trackId, kind: 'clip-no-asset' });
            continue;
          }
          ops.push({
            InsertAudioClipOp: {
              clip_id: clipId,
              track_id: s.trackId,
              asset_id: clip.assetId,
              start_ticks: e.atTicks,
              length_ticks: clip.lengthTicks,
              offset_ticks: clip.offsetTicks,
            },
          });
        } else {
          ops.push({
            InsertMidiClipOp: {
              clip_id: clipId,
              track_id: s.trackId,
              start_ticks: e.atTicks,
              length_ticks: clip.lengthTicks,
            },
          });
        }
      } else if (s.content.kind === 'pattern') {
        const pattern = opts.resolvePattern?.(s.content.patternId);
        const len = opts.patternClipLengthTicks;
        if (pattern === undefined || len === undefined) {
          skipped.push({ trackId: s.trackId, kind: 'pattern' });
          continue;
        }
        const clipId = mint();
        ops.push({
          InsertMidiClipOp: {
            clip_id: clipId,
            track_id: s.trackId,
            start_ticks: e.atTicks,
            length_ticks: len,
          },
        });
        const notes = patternToInsertOps(clipId, pattern, mint);
        ops.push(...notes.ops);
        noteIdsByClip[clipId] = notes.noteIds;
      } else {
        skipped.push({ trackId: s.trackId, kind: 'empty' });
      }
    }
  }
  return { transactionId, ops, skipped, noteIdsByClip };
}

/**
 * Pattern variation → a fresh StepPattern (PAT-05 "variations as
 * first-class objects"): duplicate rows/steps with new ids are NOT
 * needed — StepPattern is immutable data; a variation is the same
 * shape re-saved under a new patternId. `renamePattern` keeps it
 * honest (no wire pattern op exists — document lane persists).
 */
export function patternVariation(
  patternId: string,
  base: StepPattern,
  name: string,
): StepPattern {
  return { ...base, patternId, name };
}

/**
 * Convert a pattern slot into a MIDI clip at `atTicks` — one
 * transaction: InsertMidiClipOp + the pattern's InsertNoteOps.
 */
export function patternToClipOps(
  trackId: string,
  pattern: StepPattern,
  atTicks: string,
  lengthTicks: string,
  mint: () => string,
): { transactionId: string; ops: PersistentOp[]; clipId: string; noteIds: string[] } {
  const transactionId = mint();
  const clipId = mint();
  const notes = patternToInsertOps(clipId, pattern, mint);
  return {
    transactionId,
    clipId,
    noteIds: notes.noteIds,
    ops: [
      {
        InsertMidiClipOp: {
          clip_id: clipId,
          track_id: trackId,
          start_ticks: i64str(parseI64(atTicks)),
          length_ticks: i64str(parseI64(lengthTicks)),
        },
      },
      ...notes.ops,
    ],
  };
}

/** Quantize helper re-exported for callers building capture stamps. */
export function captureStamp(nowTicks: string, q: LaunchQuantize): string {
  return quantizeLaunchAt(nowTicks, q);
}
