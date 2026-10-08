// Track freeze / unfreeze and long-tail bounce (W17/ENG-05; T68).
//
// Freeze is a proposal-level spec on the studio side: the bounce is
// rendered by the engine lane (an async job — same NEEDS family as
// W12 AI jobs: a render request + completion event). What we own is
// the honest client surface: freeze spec (tail policy, source
// preservation), the frozen-track view flags, and the ops emitted on
// the engine's completion receipt (remove source clips -> insert the
// bounce clip) under ONE transaction so unfreeze = one undo.

import { parseI64 } from 'void-client';
import type { PersistentOp } from 'void-client';
import type { ClipView } from '../timeline/geometry';
import { isValidRegion } from '../takes/takes';

/** Tail policy for bounce rendering (ENG-05 "bounce with long
 * tails"): the render extends past the last clip end by the tail
 * length so reverb/delay rings out. */
export type TailPolicy =
  | { kind: 'clip-edge' } // cut exactly at last clip end
  | { kind: 'fixed'; ticks: string } // fixed tail after last clip
  | { kind: 'silence'; thresholdDb: number; maxTicks: string }; // until signal < threshold (engine decides)

export interface FreezeSpec {
  freezeId: string;
  trackId: string;
  tail: TailPolicy;
  /** When true the source clips stay in the arrangement under a
   * hidden flag instead of being removed (cheap unfreeze). When
   * false they are removed in the same transaction (still undoable). */
  preserveSources: boolean;
}

/** The view-state row a frozen track carries. */
export interface FrozenTrackView {
  trackId: string;
  freezeId: string;
  /** Removed source clip ids (engine receipt told us which). */
  removedClipIds: string[];
  /** The inserted bounce clip id. */
  bounceClipId: string;
  /** Transaction that applied the freeze — unfreeze is
   * `client.undo(transactionId)` when sources were removed. */
  transactionId: string;
}

export function lastClipEnd(clips: ClipView[]): string {
  let end = 0n;
  for (const c of clips) {
    const e = parseI64(c.startTicks) + parseI64(c.lengthTicks);
    if (e > end) end = e;
  }
  return end.toString();
}

/** Render region for a freeze request: track start..last clip end +
 * tail (when fixed; 'silence' and 'clip-edge' let the engine bound
 * the render and report the true region). */
export function freezeRenderRegion(
  clips: ClipView[],
  tail: TailPolicy,
): { startTicks: string; lengthTicks: string } | null {
  if (clips.length === 0) return null;
  let start = parseI64(clips[0]!.startTicks);
  for (const c of clips) if (parseI64(c.startTicks) < start) start = parseI64(c.startTicks);
  const end = parseI64(lastClipEnd(clips));
  const renderEnd =
    tail.kind === 'fixed' ? end + parseI64(tail.ticks) : end;
  if (renderEnd <= start) return null;
  const region = { startTicks: start.toString(), lengthTicks: (renderEnd - start).toString() };
  return isValidRegion(region) ? region : null;
}

/**
 * Ops applied once the engine reports the bounce asset: remove the
 * source clips and insert the bounce clip. ONE transaction. Emitted
 * only after the render job's success event — the WebView never
 * speculates a clip in.
 */
export function freezeApplyOps(
  spec: FreezeSpec,
  sourceClips: ClipView[],
  bounceAssetId: string,
  bounceRegion: { startTicks: string; lengthTicks: string },
  mint: () => string,
): { transactionId: string; ops: PersistentOp[]; bounceClipId: string; removedClipIds: string[] } {
  const transactionId = mint();
  const ops: PersistentOp[] = [];
  const removedClipIds: string[] = [];
  if (!spec.preserveSources) {
    for (const c of sourceClips) {
      ops.push({ RemoveClipOp: { clip_id: c.clipId } });
      removedClipIds.push(c.clipId);
    }
  }
  const bounceClipId = mint();
  ops.push({
    InsertAudioClipOp: {
      clip_id: bounceClipId,
      track_id: spec.trackId,
      asset_id: bounceAssetId,
      start_ticks: bounceRegion.startTicks,
      length_ticks: bounceRegion.lengthTicks,
      offset_ticks: '0',
    },
  });
  return { transactionId, ops, bounceClipId, removedClipIds };
}
