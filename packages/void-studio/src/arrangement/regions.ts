// Region loops, aliases and folder tracks (W17/ARR-03; T67).
//
// - Loop/repeat: the wire has no repeat-count field on clip ops
//   (NEEDS rev2), so "loop" materializes as real duplicate clips in
//   one transaction — inspectable, undoable, no hidden repeat state.
// - Alias: a shared-source clip with a propagate flag. Editing the
//   source fans the same delta to every alias member in one
//   transaction; copying without the flag is an independent clip.
// - Folder tracks: pure grouping view state (no wire membership).

import { i64str, parseI64 } from 'void-client';
import type { PersistentOp } from 'void-client';
import type { ClipView } from '../timeline/geometry';

export interface LoopPlan {
  transactionId: string;
  ops: PersistentOp[];
  /** clip ids inserted (order = repeat order). */
  insertedClipIds: string[];
}

/**
 * Materialize a region loop: `repeats` extra copies placed
 * back-to-back after the clip. One transaction; each repeat is a
 * real clip referencing the same asset (AUDIO) — undo removes them
 * all together.
 */
export function loopClipOps(
  clip: ClipView,
  repeats: number,
  mint: () => string,
): LoopPlan {
  const n = Math.max(0, Math.round(repeats));
  const len = parseI64(clip.lengthTicks);
  const start = parseI64(clip.startTicks);
  const ops: PersistentOp[] = [];
  const insertedClipIds: string[] = [];
  for (let i = 1; i <= n; i++) {
    const clipId = mint();
    insertedClipIds.push(clipId);
    const at = i64str(start + BigInt(i) * len);
    if (clip.kind === 'AUDIO') {
      if (!clip.assetId) {
        throw new Error(`audio clip ${clip.clipId} has no asset_id`);
      }
      ops.push({
        InsertAudioClipOp: {
          clip_id: clipId,
          track_id: clip.trackId,
          asset_id: clip.assetId,
          start_ticks: at,
          length_ticks: clip.lengthTicks,
          offset_ticks: clip.offsetTicks,
        },
      });
    } else {
      ops.push({
        InsertMidiClipOp: {
          clip_id: clipId,
          track_id: clip.trackId,
          start_ticks: at,
          length_ticks: clip.lengthTicks,
        },
      });
    }
  }
  return { transactionId: mint(), ops, insertedClipIds };
}

/** An alias group: clips sharing one source asset, optionally
 * propagating edits (ARR-03 "linked instances"). */
export interface AliasGroup {
  groupId: string;
  memberClipIds: string[];
  /** When true, a trim/move on any member is applied to all. */
  propagate: boolean;
}

/**
 * Fan an edit out to every member of an alias group. `edit` is the
 * op built for the edited clip; the same clip_id gets swapped per
 * member. Only Move/Trim translate safely (they carry clip-relative
 * fields); anything else returns null and the caller must refuse.
 */
export function aliasPropagateOps(
  group: AliasGroup,
  edit: PersistentOp,
): PersistentOp[] | null {
  if (!group.propagate) return null;
  const members = group.memberClipIds;
  if ('MoveClipOp' in edit) {
    const { track_id, start_ticks } = edit.MoveClipOp;
    return members.map((clip_id) => ({
      MoveClipOp: { clip_id, track_id, start_ticks },
    }));
  }
  if ('TrimClipOp' in edit) {
    const { start_ticks, length_ticks, offset_ticks } = edit.TrimClipOp;
    return members.map((clip_id) => ({
      TrimClipOp: { clip_id, start_ticks, length_ticks, offset_ticks },
    }));
  }
  return null;
}

/** Folder tracks — grouping view state with explicit membership. */
export interface FolderView {
  folderId: string;
  name: string;
  memberTrackIds: string[];
  collapsed: boolean;
  kind: 'folder' | 'stack';
}

/** Ordering for the timeline: folder members render contiguously
 * after the folder header; non-member tracks keep their own order. */
export function orderTracksWithFolders(
  trackIds: string[],
  folders: FolderView[],
): Array<{ trackId: string } | { folderId: string }> {
  const memberOf = new Map<string, FolderView>();
  for (const f of folders) for (const t of f.memberTrackIds) memberOf.set(t, f);
  const seenFolders = new Set<string>();
  const out: Array<{ trackId: string } | { folderId: string }> = [];
  for (const t of trackIds) {
    const f = memberOf.get(t);
    if (!f) {
      out.push({ trackId: t });
      continue;
    }
    if (!seenFolders.has(f.folderId)) {
      seenFolders.add(f.folderId);
      out.push({ folderId: f.folderId });
      if (!f.collapsed) for (const m of f.memberTrackIds) out.push({ trackId: m });
    }
    // members are emitted under their folder header only
  }
  return out;
}
