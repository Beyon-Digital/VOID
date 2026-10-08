// Transient marker model (W19 / EDIT-04, EDIT-05; T72/T74 model half).
//
// Detection itself is an engine/analysis product — onsets arrive as a
// bounded summary list (like LoudnessTile; never PCM in the WebView).
// This module owns the *marker model*: insertion order, edit ops that
// keep markers correct across clip split/move/trim, and removal so
// false detections are hand-fixable (EDIT-05 "false triggers can be
// removed").
//
// Coordinates: markers are CLIP-LOCAL — `ticks` counts from the clip's
// own start, so a MoveClipOp carries its markers without touching the
// bank (they survive by construction). SplitClipOp semantics (verified
// in Ops.cpp): the original clip keeps [start, at) and the new clip gets
// [at, end) — `markersOnSplit` partitions the bank accordingly and
// re-zeros the right half's coordinates.

import { i64str, parseI64 } from 'void-client';

/** One transient anchor on a clip, in clip-local ticks. */
export interface TransientMarker {
  /** Stable marker id (caller-minted). */
  id: string;
  /** Position inside the clip, 0 <= ticks < clip.lengthTicks. */
  ticks: string;
  /** Analysis strength 0..1 when the marker came from detection. */
  strength?: number;
  /** True for user-placed markers (detection vs manual origin). */
  manual?: boolean;
}

/** clipId -> sorted marker list. */
export type MarkerBank = Record<string, TransientMarker[]>;

/** One detected onset from the analysis product. */
export interface OnsetPoint {
  ticks: string;
  strength?: number;
}

function assertInside(ticks: bigint, clipLen: bigint, what: string): void {
  if (ticks < 0n || ticks >= clipLen) {
    throw new Error(`${what} out of clip bounds: ${ticks} not in [0, ${clipLen})`);
  }
}

function sortMarkers(list: TransientMarker[]): TransientMarker[] {
  return [...list].sort((a, b) => {
    const d = parseI64(a.ticks) - parseI64(b.ticks);
    return d < 0n ? -1 : d > 0n ? 1 : a.id < b.id ? -1 : 1;
  });
}

/** Detection → markers. Onsets outside the clip or duplicate-anchored
 * (same ticks as an existing/earlier onset) are skipped. */
export function markersFromOnsets(
  clipLengthTicks: string,
  onsets: OnsetPoint[],
  mint: () => string,
): TransientMarker[] {
  const len = parseI64(clipLengthTicks);
  const seen = new Set<string>();
  const out: TransientMarker[] = [];
  for (const o of onsets) {
    const t = parseI64(o.ticks);
    if (t < 0n || t >= len || seen.has(o.ticks)) continue;
    seen.add(o.ticks);
    out.push({ id: mint(), ticks: i64str(t), strength: o.strength });
  }
  return sortMarkers(out);
}

/** Insert a marker at clip-local `ticks` (snap/refine is a caller
 * concern — this validates bounds and ordering). */
export function insertMarker(
  bank: MarkerBank,
  clipId: string,
  clipLengthTicks: string,
  marker: TransientMarker,
): MarkerBank {
  const len = parseI64(clipLengthTicks);
  const t = parseI64(marker.ticks);
  assertInside(t, len, 'marker');
  const list = bank[clipId] ?? [];
  if (list.some((m) => m.id === marker.id)) {
    throw new Error(`duplicate marker id ${marker.id}`);
  }
  if (list.some((m) => m.ticks === marker.ticks)) {
    throw new Error(`marker already exists at ${marker.ticks} on clip ${clipId}`);
  }
  return { ...bank, [clipId]: sortMarkers([...list, marker]) };
}

/** Delete a marker by id. */
export function deleteMarker(
  bank: MarkerBank,
  clipId: string,
  markerId: string,
): MarkerBank {
  const list = bank[clipId] ?? [];
  const next = list.filter((m) => m.id !== markerId);
  if (next.length === list.length) {
    throw new Error(`marker ${markerId} not on clip ${clipId}`);
  }
  return { ...bank, [clipId]: next };
}

/** Shift markers whose position lies inside [regionStart, regionEnd)
 * (clip-local) by delta. Any marker pushed out of bounds rejects the
 * whole shift — partial shifts would silently drop anchors. */
export function shiftTransientMarkers(
  bank: MarkerBank,
  clipId: string,
  clipLengthTicks: string,
  region: { startTicks: string; lengthTicks: string },
  deltaTicks: string,
): MarkerBank {
  const len = parseI64(clipLengthTicks);
  const delta = parseI64(deltaTicks);
  const rS = parseI64(region.startTicks);
  const rE = rS + parseI64(region.lengthTicks);
  const list = bank[clipId] ?? [];
  const moved: TransientMarker[] = [];
  for (const m of list) {
    const t = parseI64(m.ticks);
    if (t >= rS && t < rE) {
      const nt = t + delta;
      assertInside(nt, len, `shifted marker ${m.id}`);
      moved.push({ ...m, ticks: i64str(nt) });
    } else {
      moved.push(m);
    }
  }
  return { ...bank, [clipId]: sortMarkers(moved) };
}

/**
 * SplitClipOp mapping: markers at ticks < atTicks stay on the original
 * clip; markers >= atTicks move to the new clip with their position
 * re-zeroed (atTicks becomes the new clip's 0). Returns {left, right}
 * marker lists — callers update the bank under the new clip id.
 */
export function markersOnSplit(
  markers: TransientMarker[],
  atTicks: string,
): { left: TransientMarker[]; right: TransientMarker[] } {
  const at = parseI64(atTicks);
  if (at <= 0n) throw new Error('split point must be inside the clip');
  const left: TransientMarker[] = [];
  const right: TransientMarker[] = [];
  for (const m of markers) {
    const t = parseI64(m.ticks);
    if (t < at) {
      left.push(m);
    } else {
      right.push({ ...m, ticks: i64str(t - at) });
    }
  }
  return { left: sortMarkers(left), right: sortMarkers(right) };
}

/**
 * Apply a split to the bank: the original clip id keeps `left`, the new
 * clip id (from the SplitClipOp receipt) gets `right`.
 */
export function bankOnSplit(
  bank: MarkerBank,
  clipId: string,
  newClipId: string,
  atTicks: string,
): MarkerBank {
  const { left, right } = markersOnSplit(bank[clipId] ?? [], atTicks);
  return { ...bank, [clipId]: left, [newClipId]: right };
}

/**
 * Markers are clip-local, so a MoveClipOp (timeline position change)
 * leaves the bank untouched — this is the documented invariant, asserted
 * in tests rather than assumed.
 */
export function bankOnMove(bank: MarkerBank): MarkerBank {
  return bank;
}

/**
 * TrimClipOp mapping for `newStartTicks`/`newLengthTicks`-style trims
 * expressed in clip-local coordinates: markers outside [keepStart,
 * keepStart+keepLen) are dropped; kept markers re-zero to keepStart.
 */
export function bankOnTrim(
  bank: MarkerBank,
  clipId: string,
  keepStartTicks: string,
  keepLengthTicks: string,
): MarkerBank {
  const ks = parseI64(keepStartTicks);
  const kl = parseI64(keepLengthTicks);
  const list = bank[clipId] ?? [];
  const kept = list
    .filter((m) => {
      const t = parseI64(m.ticks);
      return t >= ks && t < ks + kl;
    })
    .map((m) => ({ ...m, ticks: i64str(parseI64(m.ticks) - ks) }));
  return { ...bank, [clipId]: kept };
}

/** Drop a clip's markers (clip removed). */
export function bankOnRemove(bank: MarkerBank, clipId: string): MarkerBank {
  const next = { ...bank };
  delete next[clipId];
  return next;
}

/** Nearest marker to a clip-local position — snap target for editing. */
export function nearestMarker(
  markers: TransientMarker[],
  ticks: string,
): TransientMarker | undefined {
  const t = parseI64(ticks);
  let best: TransientMarker | undefined;
  let bestD: bigint | undefined;
  for (const m of markers) {
    const d = parseI64(m.ticks) - t;
    const ad = d < 0n ? -d : d;
    if (bestD === undefined || ad < bestD) {
      bestD = ad;
      best = m;
    }
  }
  return best;
}
