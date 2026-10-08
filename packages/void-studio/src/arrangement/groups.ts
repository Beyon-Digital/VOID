// Track stacks/edit-groups and protected edits (W17/ARR-06, ARR-07).
//
// - Group: a named set of tracks. A lane edit (move/trim/remove/split
//   or gain/pan/mute/solo) applied to a member fans the same op shape
//   out to every member in ONE transaction (ARR-06 "everything as
//   one"). Membership is view-state (no wire group op — NEEDS).
// - Protected edits (ARR-07): a per-entity lock flag enforced HERE in
//   the studio layer — refusing to emit edit ops for protected clips,
//   tracks or time ranges. The engine never sees an op it could not
//   verify anyway; protection is a UX safety rail, not a security
//   boundary (documented).

import type { PersistentOp } from 'void-client';
import type { ClipView } from '../timeline/geometry';
import { regionsOverlap, type Region } from '../takes/takes';
import type { EditGroup } from './types';

export type { EditGroup };

export class ProtectedError extends Error {}

/** The op kinds that mutate arrangement content (checked against
 * protection). Transport/tempo edits are unaffected. */
const EDIT_OP_KEYS = new Set([
  'InsertAudioClipOp', 'InsertMidiClipOp', 'RemoveClipOp', 'MoveClipOp',
  'TrimClipOp', 'SplitClipOp', 'InsertNoteOp', 'RemoveNoteOp', 'SetNoteOp',
  'AddTrackOp', 'RemoveTrackOp',
]);

function opKey(op: PersistentOp): string {
  return Object.keys(op)[0]!;
}

/** The clip id an op touches (when it names one). */
function opClipId(op: PersistentOp): string | null {
  const k = opKey(op);
  const v = (op as Record<string, { clip_id?: string }>)[k];
  return v?.clip_id ?? null;
}

function opTrackId(op: PersistentOp): string | null {
  const k = opKey(op);
  const v = (op as Record<string, { track_id?: string }>)[k];
  return v?.track_id ?? null;
}

/** Guard: throw ProtectedError if `op` would mutate a protected
 * clip/track. Call before sending. `isProtectedClip`/`isProtectedTrack`
 * are lookup predicates on the protected set (store-owned). */
export function assertUnprotected(
  op: PersistentOp,
  opts: {
    protectedClipIds: ReadonlySet<string>;
    protectedTrackIds: ReadonlySet<string>;
    protectedRegions?: { trackId: string; region: Region }[];
    clipAt?: (clipId: string) => ClipView | undefined;
  },
): void {
  const key = opKey(op);
  if (!EDIT_OP_KEYS.has(key)) return;
  const clipId = opClipId(op);
  if (clipId !== null && opts.protectedClipIds.has(clipId)) {
    throw new ProtectedError(`clip ${clipId} is protected`);
  }
  const trackId = opTrackId(op);
  if (trackId !== null && opts.protectedTrackIds.has(trackId)) {
    throw new ProtectedError(`track ${trackId} is protected`);
  }
  // Protected time ranges on a track (ARR-07 "pin a range").
  if (opts.protectedRegions && opts.clipAt) {
    const movedStart =
      'MoveClipOp' in op ? op.MoveClipOp.start_ticks
      : 'TrimClipOp' in op ? op.TrimClipOp.start_ticks
      : undefined;
    const target = clipId !== null ? opts.clipAt(clipId) : undefined;
    const tid = trackId ?? target?.trackId ?? null;
    if (tid && target) {
      const region =
        movedStart !== undefined
          ? { startTicks: movedStart, lengthTicks: target.lengthTicks }
          : { startTicks: target.startTicks, lengthTicks: target.lengthTicks };
      const hit = opts.protectedRegions.some(
        (pr) => pr.trackId === tid && regionsOverlap(pr.region, region),
      );
      if (hit) throw new ProtectedError(`range on track ${tid} is protected`);
    }
    // Inserts landing inside a protected range.
    if ('InsertAudioClipOp' in op || 'InsertMidiClipOp' in op) {
      const body = 'InsertAudioClipOp' in op ? op.InsertAudioClipOp : op.InsertMidiClipOp;
      const hit = opts.protectedRegions.some(
        (pr) =>
          pr.trackId === body.track_id &&
          regionsOverlap(pr.region, {
            startTicks: body.start_ticks,
            lengthTicks: body.length_ticks,
          }),
      );
      if (hit) throw new ProtectedError(`range on track ${body.track_id} is protected`);
    }
  }
}

/**
 * Fan a single-clip edit op out across an edit group: every member
 * clip of each member track receives the same op shape with its own
 * clip_id. `memberClips` resolves trackId -> clips to expand on.
 * Returns null when the op does not fan out cleanly (e.g. a track
 * op), so the caller can decide.
 */
export function groupEditOps(
  group: EditGroup,
  originClipId: string,
  originTrackId: string,
  op: PersistentOp,
  memberClips: (trackId: string) => ClipView[],
): PersistentOp[] | null {
  const key = opKey(op);
  const rewrite = (clipId: string, trackId: string): PersistentOp | null => {
    switch (key) {
      case 'MoveClipOp': {
        const o = (op as { MoveClipOp: { start_ticks: string } }).MoveClipOp;
        return { MoveClipOp: { clip_id: clipId, track_id: trackId, start_ticks: o.start_ticks } };
      }
      case 'TrimClipOp': {
        const o = (op as { TrimClipOp: { start_ticks: string; length_ticks: string; offset_ticks?: string } }).TrimClipOp;
        return {
          TrimClipOp: {
            clip_id: clipId,
            start_ticks: o.start_ticks,
            length_ticks: o.length_ticks,
            ...(o.offset_ticks !== undefined ? { offset_ticks: o.offset_ticks } : {}),
          },
        };
      }
      case 'RemoveClipOp':
        return { RemoveClipOp: { clip_id: clipId } };
      default:
        return null;
    }
  };
  const out: PersistentOp[] = [];
  for (const tid of group.trackIds) {
    for (const c of memberClips(tid)) {
      if (tid === originTrackId && c.clipId === originClipId) continue;
      const r = rewrite(c.clipId, tid);
      if (r === null) return null;
      out.push(r);
    }
  }
  const head = rewrite(originClipId, originTrackId);
  if (head === null) return null;
  return [head, ...out];
}

/** Tracks hidden from the arrangement (ARR-03): pure view-state —
 * the store's hidden set; renderers skip them, ops never check it. */
export function visibleTracks(
  trackIds: string[],
  hiddenTrackIds: ReadonlySet<string>,
): string[] {
  return trackIds.filter((t) => !hiddenTrackIds.has(t));
}
