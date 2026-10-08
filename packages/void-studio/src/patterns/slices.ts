// Quick-slice spec (SND-02, T59): sample → slice boundaries → region refs.
//
// A slice is DATA ONLY: {assetId, startTicks, lengthTicks} ranges into an
// immutable source asset. Nothing here decodes, copies, or writes audio —
// the source asset is never touched (T59: "source sample is immutable").
// Turning a slice into music = InsertAudioClipOp with the slice's offset
// and length, so the engine reads the same immutable asset at a window.

import { parseI64 } from 'void-client';
import type { InsertAudioClipOp, PersistentOp } from 'void-client';

export interface SliceSpec {
  sliceId: string;
  assetId: string;
  /** Offset into the source asset (ticks at the asset's musical grid). */
  startTicks: string;
  lengthTicks: string;
}

/** An ordered, non-overlapping slice set over one asset. */
export interface SliceMap {
  mapId: string;
  assetId: string;
  assetLengthTicks: string;
  slices: SliceSpec[];
}

export class SliceError extends Error {}

function checkPoints(assetLengthTicks: bigint, points: bigint[]): bigint[] {
  const sorted = [...new Set(points.map((p) => p.toString()))]
    .map(BigInt)
    .sort((a, b) => (a < b ? -1 : a > b ? 1 : 0));
  for (const p of sorted) {
    if (p <= 0n || p >= assetLengthTicks) {
      throw new SliceError(
        `slice boundary ${p} outside asset range (0, ${assetLengthTicks})`,
      );
    }
  }
  return sorted;
}

/** Equal-count slicing: N even regions across the asset. */
export function sliceByCount(
  assetId: string,
  assetLengthTicks: string,
  count: number,
  mint: () => string,
): SliceMap {
  const len = parseI64(assetLengthTicks);
  if (len <= 0n) throw new SliceError('asset length must be positive');
  const n = Math.floor(count);
  if (n < 1) throw new SliceError('slice count must be ≥ 1');
  const base = len / BigInt(n);
  const rem = len % BigInt(n);
  const slices: SliceSpec[] = [];
  let at = 0n;
  for (let i = 0; i < n; i++) {
    // first `rem` slices get one extra tick — covers the whole asset
    const l = base + (BigInt(i) < rem ? 1n : 0n);
    slices.push({
      sliceId: mint(),
      assetId,
      startTicks: at.toString(10),
      lengthTicks: l.toString(10),
    });
    at += l;
  }
  return { mapId: mint(), assetId, assetLengthTicks: len.toString(10), slices };
}

/** Boundary-point slicing: user-placed/transient-detected cut points. */
export function sliceByPoints(
  assetId: string,
  assetLengthTicks: string,
  points: string[],
  mint: () => string,
): SliceMap {
  const len = parseI64(assetLengthTicks);
  if (len <= 0n) throw new SliceError('asset length must be positive');
  const bounds = checkPoints(len, points.map((p) => parseI64(p)));
  const cuts = [0n, ...bounds, len];
  const slices: SliceSpec[] = [];
  for (let i = 0; i + 1 < cuts.length; i++) {
    slices.push({
      sliceId: mint(),
      assetId,
      startTicks: cuts[i].toString(10),
      lengthTicks: (cuts[i + 1] - cuts[i]).toString(10),
    });
  }
  return { mapId: mint(), assetId, assetLengthTicks: len.toString(10), slices };
}

/**
 * Slice → audio-clip op: a region reference — same immutable asset,
 * windowed by offset+length. `atTicks` is where the clip lands.
 */
export function sliceToClipOp(
  slice: SliceSpec,
  fields: { clipId: string; trackId: string; atTicks: string },
): PersistentOp {
  const op: InsertAudioClipOp = {
    clip_id: fields.clipId,
    track_id: fields.trackId,
    asset_id: slice.assetId,
    start_ticks: fields.atTicks,
    length_ticks: slice.lengthTicks,
    offset_ticks: slice.startTicks,
  };
  return { InsertAudioClipOp: op };
}

/** Every slice → one clip op in consecutive order on the timeline. */
export function sliceMapToClipOps(
  map: SliceMap,
  fields: { trackId: string; atTicks: string },
  mint: () => string,
): { transactionId: string; ops: PersistentOp[] } {
  let at = parseI64(fields.atTicks);
  const ops = map.slices.map((s) => {
    const op = sliceToClipOp(s, {
      clipId: mint(),
      trackId: fields.trackId,
      atTicks: at.toString(10),
    });
    at += parseI64(s.lengthTicks);
    return op;
  });
  return { transactionId: mint(), ops };
}
