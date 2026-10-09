// Clip fades, crossfades and strip-silence (W17/ARR-05; T66/T67).
//
// The wire has no per-clip fade fields (NEEDS rev2) — fades are a
// ClipFadeMap of view-state specs keyed by clipId, produced here and
// consumed by the engine lane when the field lands. Strip-silence is
// honest: detection input is a bounded loudness-tile summary (an
// engine/job product, NOT PCM in the WebView); this module turns it
// into a keep-region plan materialized as ordinary clip ops.

import { i64str, parseI64 } from 'void-client';
import type { PersistentOp } from 'void-client';
import type { ClipView } from '../timeline/geometry';
import type { FadeShape, FadeSpec } from '../takes/types';
import { regionsOverlap, type Region } from '../takes/takes';

export type { FadeShape, FadeSpec };

/** Fades attached to one clip (in/out). */
export interface ClipFades {
  fadeIn?: FadeSpec;
  fadeOut?: FadeSpec;
}

/** View-state map: clipId -> fades. */
export type FadeMap = Record<string, ClipFades>;

/** Set/bound a fade: a fade can never exceed the clip's length. */
export function setFade(
  map: FadeMap,
  clip: ClipView,
  edge: 'in' | 'out',
  spec: FadeSpec | undefined,
): FadeMap {
  if (spec !== undefined) {
    const len = parseI64(spec.lengthTicks);
    const clipLen = parseI64(clip.lengthTicks);
    if (len <= 0n || len > clipLen) {
      throw new Error(`fade length ${len} out of range for clip ${clip.clipId}`);
    }
  }
  const cur = map[clip.clipId] ?? {};
  const next = { ...cur, [edge === 'in' ? 'fadeIn' : 'fadeOut']: spec };
  return { ...map, [clip.clipId]: next };
}

/**
 * Crossfade pair for a seam where `left` ends inside `right`'s start
 * (overlapping audio clips). Returns {out on left, in on right};
 * length is bounded by the overlap and by each clip's length.
 */
export function crossfadeForSeam(
  left: ClipView,
  right: ClipView,
  opts: { shape?: FadeShape; lengthTicks?: string } = {},
): { leftFade: FadeSpec | null; rightFade: FadeSpec | null } {
  const lEnd = parseI64(left.startTicks) + parseI64(left.lengthTicks);
  const rStart = parseI64(right.startTicks);
  const overlap = lEnd - rStart;
  if (overlap <= 0n) return { leftFade: null, rightFade: null };
  let len = opts.lengthTicks !== undefined ? parseI64(opts.lengthTicks) : overlap;
  if (len > overlap) len = overlap;
  const lLen = parseI64(left.lengthTicks);
  const rLen = parseI64(right.lengthTicks);
  if (len > lLen) len = lLen;
  if (len > rLen) len = rLen;
  const spec: FadeSpec = {
    shape: opts.shape ?? 'equal-power',
    lengthTicks: i64str(len),
  };
  return { leftFade: spec, rightFade: spec };
}

// ---------------------------------------------------------------------------
// Strip silence (ARR-05): threshold analysis over a loudness summary.
// ---------------------------------------------------------------------------

/** One loudness tile: dB peak over a span (analysis product — the
 * engine/job emits these; the WebView never holds PCM). */
export interface LoudnessTile {
  startTicks: string;
  lengthTicks: string;
  /** Peak dBFS in the tile (<= 0 for signal, -Infinity ≈ silence). */
  peakDb: number;
}

export interface StripOptions {
  /** Tiles at or below this are silence. */
  thresholdDb: number;
  /** Silence shorter than this does not split. */
  minSilenceTicks: string;
  /** Pad kept regions on each side (never crosses neighbours). */
  padTicks?: string;
}

/**
 * Detect silence runs: consecutive tiles under threshold with total
 * span >= minSilenceTicks. Returns the SILENT regions.
 */
export function detectSilence(
  tiles: LoudnessTile[],
  opts: StripOptions,
): Region[] {
  const minSil = parseI64(opts.minSilenceTicks);
  const out: Region[] = [];
  let runStart: bigint | null = null;
  let runLen = 0n;
  const flush = () => {
    if (runStart !== null && runLen >= minSil) {
      out.push({ startTicks: i64str(runStart), lengthTicks: i64str(runLen) });
    }
    runStart = null;
    runLen = 0n;
  };
  for (const t of tiles) {
    const s = parseI64(t.startTicks);
    const len = parseI64(t.lengthTicks);
    if (t.peakDb <= opts.thresholdDb) {
      if (runStart === null) runStart = s;
      runLen += len;
    } else {
      flush();
    }
  }
  flush();
  return out;
}

/**
 * The complement: regions worth keeping inside the clip span, padded
 * by padTicks (clamped at region edges, never overlapping).
 */
export function keptRegions(
  clip: { startTicks: string; lengthTicks: string },
  tiles: LoudnessTile[],
  opts: StripOptions,
): Region[] {
  const clipStart = parseI64(clip.startTicks);
  const clipEnd = clipStart + parseI64(clip.lengthTicks);
  const pad = parseI64(opts.padTicks ?? '0');
  const silent = detectSilence(tiles, opts);
  const kept: Region[] = [];
  let cursor = clipStart;
  for (const s of silent) {
    const sS = parseI64(s.startTicks);
    const sE = sS + parseI64(s.lengthTicks);
    const kEnd = sS + pad < sE ? sS + pad : sE; // keep pad inside the gap
    if (kEnd > cursor) {
      kept.push({ startTicks: i64str(cursor), lengthTicks: i64str(kEnd - cursor) });
    }
    cursor = sE - pad > sS ? sE - pad : sS;
  }
  if (cursor < clipEnd) {
    kept.push({ startTicks: i64str(cursor), lengthTicks: i64str(clipEnd - cursor) });
  }
  return kept.filter((r) => parseI64(r.lengthTicks) > 0n);
}

export interface StripPlan {
  transactionId: string;
  ops: PersistentOp[];
  kept: Region[];
  removedClipIds: string[];
  insertedClipIds: string[];
}

/**
 * Strip silence → clip ops: remove the source clip, insert one clip
 * per kept region with correct asset offset. ONE transaction. The
 * source asset is untouched (ARR-05 nondestructive).
 */
export function stripSilenceOps(
  clip: ClipView,
  tiles: LoudnessTile[],
  opts: StripOptions,
  mint: () => string,
): StripPlan {
  if (clip.kind !== 'AUDIO' || !clip.assetId) {
    throw new Error('strip silence applies to audio clips with an asset');
  }
  const kept = keptRegions(clip, tiles, opts);
  const ops: PersistentOp[] = [{ RemoveClipOp: { clip_id: clip.clipId } }];
  const insertedClipIds: string[] = [];
  const clipStart = parseI64(clip.startTicks);
  const offset = parseI64(clip.offsetTicks);
  for (const r of kept) {
    const clipId = mint();
    insertedClipIds.push(clipId);
    ops.push({
      InsertAudioClipOp: {
        clip_id: clipId,
        track_id: clip.trackId,
        asset_id: clip.assetId,
        start_ticks: r.startTicks,
        length_ticks: r.lengthTicks,
        offset_ticks: i64str(offset + parseI64(r.startTicks) - clipStart),
      },
    });
  }
  return {
    transactionId: mint(),
    ops,
    kept,
    removedClipIds: [clip.clipId],
    insertedClipIds,
  };
}

/** Convenience: does this clip have any overlap with a region? */
export function clipOverlaps(clip: ClipView, r: Region): boolean {
  return regionsOverlap(
    { startTicks: clip.startTicks, lengthTicks: clip.lengthTicks },
    r,
  );
}
