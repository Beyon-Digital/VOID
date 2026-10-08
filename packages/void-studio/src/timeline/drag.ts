// Clip drag state machine — pointer gesture -> preview -> committed ops.
//
// A drag is ONE transaction: begin captures the clip's origin and the
// gesture's transaction_id; update produces an optimistic preview (view
// state only); commit maps the final preview to exactly one PersistentOp
// (move/trim/split) or an insert for duplicate. Reverted previews come
// from the caller discarding the preview on a failed receipt.

import type { PersistentOp } from 'void-client';
import { i64str, parseI64 } from 'void-client';
import type { SnapSettings } from '../workspaces/editorStore';
import type { DragMode } from '../workspaces/editorStore';
import { snapTicks } from './snap';
import type { ClipView } from './geometry';
import {
  assertValidRange,
  duplicateClipOp,
  moveClipOp,
  splitClipOp,
  trimClipOp,
} from './ops';

export interface ClipDrag {
  mode: DragMode;
  clipId: string;
  trackId: string;
  transactionId: string;
  /** Origin geometry, captured at pointerdown. */
  originStartTicks: string;
  originLengthTicks: string;
  originOffsetTicks: string;
  /** Tick position where the pointer grabbed the clip (anchor). */
  grabTicks: string;
  /** Asset/kind data needed for duplicate. */
  assetId?: string;
  kind?: string;
  name?: string;
}

export function beginClipDrag(
  clip: ClipView,
  mode: DragMode,
  grabTicks: string,
  transactionId: string,
  targetTrackId?: string,
): ClipDrag {
  assertValidRange(clip.startTicks, clip.lengthTicks);
  return {
    mode,
    clipId: clip.clipId,
    trackId: targetTrackId ?? clip.trackId,
    transactionId,
    originStartTicks: clip.startTicks,
    originLengthTicks: clip.lengthTicks,
    originOffsetTicks: clip.offsetTicks,
    grabTicks: i64str(grabTicks),
    assetId: clip.assetId,
    kind: clip.kind,
    name: clip.name,
  };
}

export interface DragPreviewState {
  startTicks: string;
  lengthTicks: string;
  offsetTicks: string;
  /** For 'split': the resolved split position. */
  atTicks?: string;
  trackId: string;
}

const MIN_CLIP_LENGTH = 1n; // ticks; anything smaller is malformed

/**
 * Produce the optimistic geometry for a pointer at `pointerTicks`.
 * Snapping applies to the leading edge (move) / the moved edge (trims) /
 * the split cursor.
 */
export function previewClipDrag(
  d: ClipDrag,
  pointerTicks: string,
  snap: SnapSettings,
  targetTrackId?: string,
): DragPreviewState {
  const pointer = parseI64(pointerTicks);
  const grab = parseI64(d.grabTicks);
  const oStart = parseI64(d.originStartTicks);
  const oLen = parseI64(d.originLengthTicks);
  const oOff = parseI64(d.originOffsetTicks);
  const trackId = targetTrackId ?? d.trackId;

  switch (d.mode) {
    case 'move':
    case 'duplicate': {
      const delta = pointer - grab;
      const start = snapTicks((oStart + delta).toString(10), snap);
      return {
        startTicks: start,
        lengthTicks: d.originLengthTicks,
        offsetTicks: d.originOffsetTicks,
        trackId,
      };
    }
    case 'trim-start': {
      const start = snapTicks(pointer.toString(10), snap);
      const delta = parseI64(start) - oStart;
      // Clamp: new length stays >= MIN; start may not pass the end.
      const newLen = oLen - delta > MIN_CLIP_LENGTH ? oLen - delta : MIN_CLIP_LENGTH;
      const appliedDelta = oLen - newLen;
      return {
        startTicks: (oStart + appliedDelta).toString(10),
        lengthTicks: newLen.toString(10),
        offsetTicks: (oOff + appliedDelta).toString(10),
        trackId,
      };
    }
    case 'trim-end': {
      const end = snapTicks(pointer.toString(10), snap);
      const newLen = parseI64(end) - oStart;
      return {
        startTicks: d.originStartTicks,
        lengthTicks: (newLen > MIN_CLIP_LENGTH ? newLen : MIN_CLIP_LENGTH).toString(10),
        offsetTicks: d.originOffsetTicks,
        trackId,
      };
    }
    case 'split': {
      const at = snapTicks(pointer.toString(10), snap);
      // Clamp the split point strictly inside the clip.
      const oEnd = oStart + oLen;
      const clamped =
        parseI64(at) <= oStart
          ? oStart + 1n
          : parseI64(at) >= oEnd
            ? oEnd - 1n
            : parseI64(at);
      return {
        startTicks: d.originStartTicks,
        lengthTicks: d.originLengthTicks,
        offsetTicks: d.originOffsetTicks,
        atTicks: clamped.toString(10),
        trackId,
      };
    }
  }
}

/**
 * Map a committed drag to its PersistentOp. `newClipId` is required for
 * 'split' (the second half) and 'duplicate' (the inserted copy).
 */
export function commitClipDrag(
  d: ClipDrag,
  preview: DragPreviewState,
  newClipId?: string,
): PersistentOp {
  switch (d.mode) {
    case 'move':
      return moveClipOp(d.clipId, preview.trackId, preview.startTicks);
    case 'trim-start':
    case 'trim-end':
      return trimClipOp(
        d.clipId,
        preview.startTicks,
        preview.lengthTicks,
        preview.offsetTicks,
      );
    case 'split': {
      if (!preview.atTicks) throw new Error('split preview missing atTicks');
      if (!newClipId) throw new Error('split requires a newClipId');
      return splitClipOp(d.clipId, preview.atTicks, newClipId);
    }
    case 'duplicate': {
      if (!newClipId) throw new Error('duplicate requires a newClipId');
      const clip: ClipView = {
        clipId: d.clipId,
        trackId: preview.trackId,
        kind: d.kind,
        name: d.name,
        assetId: d.assetId,
        startTicks: preview.startTicks,
        lengthTicks: preview.lengthTicks,
        offsetTicks: preview.offsetTicks,
      };
      return duplicateClipOp(clip, newClipId);
    }
  }
}
