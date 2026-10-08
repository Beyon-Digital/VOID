// Track & project alternatives (W17/DOC-02).
//
// An alternative is a saved arrangement spec — the set of clips for a
// track (or tracks, for project alternatives). Applying an
// alternative is remove-everything + re-insert in ONE transaction,
// so undo restores the previous arrangement as a unit. Assets are
// referenced by id — never copied. Save/load of alternative specs is
// a project-document concern: on the wire there is no alternative
// op (NEEDS rev2), so the store keeps them as view-state until the
// document lane lands persistence.

import { i64str, parseI64 } from 'void-client';
import type { PersistentOp } from 'void-client';
import type { ClipView } from '../timeline/geometry';
import type {
  AlternativeClipSpec,
  ProjectAlternative,
  TrackAlternative,
} from './types';

export type { AlternativeClipSpec, ProjectAlternative, TrackAlternative };

export class AlternativeError extends Error {}

/** Snapshot the current clips of a track into an alternative spec. */
export function captureTrackAlternative(
  alternativeId: string,
  trackId: string,
  name: string,
  clips: ClipView[],
): TrackAlternative {
  return {
    alternativeId,
    trackId,
    name,
    clips: clips.map(clipToSpec),
  };
}

function clipToSpec(c: ClipView): AlternativeClipSpec {
  return {
    kind: c.kind === 'MIDI' ? 'MIDI' : 'AUDIO',
    ...(c.assetId !== undefined ? { assetId: c.assetId } : {}),
    startTicks: c.startTicks,
    lengthTicks: c.lengthTicks,
    offsetTicks: c.offsetTicks,
    ...(c.name !== undefined ? { name: c.name } : {}),
  };
}

export interface AlternativeApplyPlan {
  transactionId: string;
  ops: PersistentOp[];
  insertedClipIds: string[];
  removedClipIds: string[];
}

function insertOpForSpec(
  spec: AlternativeClipSpec,
  trackId: string,
  clipId: string,
): PersistentOp {
  if (spec.kind === 'AUDIO') {
    if (!spec.assetId) {
      throw new AlternativeError('audio spec lacks asset_id');
    }
    return {
      InsertAudioClipOp: {
        clip_id: clipId,
        track_id: trackId,
        asset_id: spec.assetId,
        start_ticks: spec.startTicks,
        length_ticks: spec.lengthTicks,
        offset_ticks: spec.offsetTicks,
      },
    };
  }
  return {
    InsertMidiClipOp: {
      clip_id: clipId,
      track_id: trackId,
      start_ticks: spec.startTicks,
      length_ticks: spec.lengthTicks,
    },
  };
}

/** Check an alternative spec for sanity (non-negative offsets,
 * positive lengths, audio carries an asset). */
export function checkAlternative(spec: TrackAlternative): string[] {
  const errors: string[] = [];
  for (const c of spec.clips) {
    if (parseI64(c.lengthTicks) <= 0n) errors.push('non-positive clip length');
    if (parseI64(c.startTicks) < 0n) errors.push('clip before zero');
    if (parseI64(c.offsetTicks) < 0n) errors.push('negative asset offset');
    if (c.kind === 'AUDIO' && !c.assetId) errors.push('audio clip lacks asset');
  }
  return errors;
}

/**
 * Apply a track alternative: remove every clip currently on the
 * track, insert the alternative's clips. ONE transaction; UndoOp
 * restores the previous arrangement whole.
 */
export function applyTrackAlternativeOps(
  alt: TrackAlternative,
  currentClips: ClipView[],
  mint: () => string,
): AlternativeApplyPlan {
  const errors = checkAlternative(alt);
  if (errors.length > 0) {
    throw new AlternativeError(`invalid alternative ${alt.alternativeId}: ${errors.join('; ')}`);
  }
  const transactionId = mint();
  const ops: PersistentOp[] = [];
  const insertedClipIds: string[] = [];
  for (const c of currentClips.filter((c) => c.trackId === alt.trackId)) {
    ops.push({ RemoveClipOp: { clip_id: c.clipId } });
  }
  for (const spec of alt.clips) {
    const clipId = mint();
    insertedClipIds.push(clipId);
    ops.push(insertOpForSpec(spec, alt.trackId, clipId));
  }
  return {
    transactionId,
    ops,
    insertedClipIds,
    removedClipIds: currentClips.filter((c) => c.trackId === alt.trackId).map((c) => c.clipId),
  };
}

/**
 * Apply a project alternative: one transaction covering every track
 * listed in the spec (tracks absent from the spec are untouched).
 */
export function applyProjectAlternativeOps(
  alt: ProjectAlternative,
  currentClips: ClipView[],
  mint: () => string,
): AlternativeApplyPlan {
  const transactionId = mint();
  const ops: PersistentOp[] = [];
  const insertedClipIds: string[] = [];
  const removedClipIds: string[] = [];
  for (const [trackId, specs] of Object.entries(alt.tracks)) {
    for (const c of currentClips.filter((c) => c.trackId === trackId)) {
      ops.push({ RemoveClipOp: { clip_id: c.clipId } });
      removedClipIds.push(c.clipId);
    }
    for (const spec of specs) {
      const clipId = mint();
      insertedClipIds.push(clipId);
      ops.push(insertOpForSpec(spec, trackId, clipId));
    }
  }
  return { transactionId, ops, insertedClipIds, removedClipIds };
}

/** Rename is view-state only. */
export function renameAlternative(a: TrackAlternative, name: string): TrackAlternative {
  return { ...a, name };
}
