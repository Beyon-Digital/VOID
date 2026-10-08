// Clip op payload builders — exact PersistentOp shapes for the wire.
//
// These map editor gestures onto the protocol schema (void_control.fbs):
//   move    -> MoveClipOp   { clip_id, track_id, start_ticks }
//   resize  -> TrimClipOp   { clip_id, start_ticks, length_ticks, offset_ticks }
//   split   -> SplitClipOp  { clip_id, at_ticks, new_clip_id }
//   delete  -> RemoveClipOp { clip_id }
//   insert  -> InsertMidiClipOp / InsertAudioClipOp (duplicate = insert
//             with a fresh clip_id — there is no DuplicateClipOp variant)
// Every int64 field is a decimal string (i64str-validated upstream).

import type { PersistentOp } from 'void-client';
import { i64str, parseI64 } from 'void-client';
import type { ClipView } from './geometry';

/** Assert a tick range is well-formed before it leaves JS. */
export function assertValidRange(
  startTicks: string,
  lengthTicks: string,
): void {
  const start = parseI64(startTicks);
  const len = parseI64(lengthTicks);
  if (len <= 0n) {
    throw new Error(`clip length must be > 0 (start ${start}, len ${len})`);
  }
}

export function moveClipOp(
  clipId: string,
  trackId: string,
  startTicks: string,
): PersistentOp {
  i64str(startTicks); // validation only — clips may sit at negative pickup ticks
  return {
    MoveClipOp: {
      clip_id: clipId,
      track_id: trackId,
      start_ticks: i64str(startTicks),
    },
  };
}

export function trimClipOp(
  clipId: string,
  startTicks: string,
  lengthTicks: string,
  offsetTicks = '0',
): PersistentOp {
  assertValidRange(startTicks, lengthTicks);
  return {
    TrimClipOp: {
      clip_id: clipId,
      start_ticks: i64str(startTicks),
      length_ticks: i64str(lengthTicks),
      offset_ticks: i64str(offsetTicks),
    },
  };
}

export function splitClipOp(
  clipId: string,
  atTicks: string,
  newClipId: string,
): PersistentOp {
  return {
    SplitClipOp: {
      clip_id: clipId,
      at_ticks: i64str(atTicks),
      new_clip_id: newClipId,
    },
  };
}

export function removeClipOp(clipId: string): PersistentOp {
  return { RemoveClipOp: { clip_id: clipId } };
}

/**
 * Duplicate = insert a new clip covering the same range (the schema has
 * no dedicated duplicate op). Audio clips need their asset_id; MIDI
 * clips just need the range.
 */
export function duplicateClipOp(clip: ClipView, newClipId: string): PersistentOp {
  assertValidRange(clip.startTicks, clip.lengthTicks);
  if (clip.kind === 'AUDIO') {
    if (!clip.assetId) {
      throw new Error(
        `cannot duplicate audio clip ${clip.clipId}: asset_id missing from read view`,
      );
    }
    return {
      InsertAudioClipOp: {
        clip_id: newClipId,
        track_id: clip.trackId,
        asset_id: clip.assetId,
        start_ticks: clip.startTicks,
        length_ticks: clip.lengthTicks,
        offset_ticks: clip.offsetTicks,
      },
    };
  }
  return {
    InsertMidiClipOp: {
      clip_id: newClipId,
      track_id: clip.trackId,
      start_ticks: clip.startTicks,
      length_ticks: clip.lengthTicks,
    },
  };
}
