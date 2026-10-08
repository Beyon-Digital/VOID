// Take-lane model (W17/REC-04; T66) — pure functions.
//
// A take folder is the ordered stack of takes over one region. Loop
// recording produces one take per pass; a failed/interrupted pass is
// still a take (`complete:false`, the recovered-chunk marker) and
// NEVER overwrites an earlier pass's source asset — "a failed take
// never overwrites an earlier recorded file" (REC-04 acceptance).

import { parseI64 } from 'void-client';
import type { TakeFolder, TakeRecord } from './types';

export interface Region {
  startTicks: string;
  lengthTicks: string;
}

export function regionEnd(r: Region): bigint {
  return parseI64(r.startTicks) + parseI64(r.lengthTicks);
}

/** Half-open [start, start+length) overlap test (exact bigint math). */
export function regionsOverlap(a: Region, b: Region): boolean {
  const aS = parseI64(a.startTicks);
  const aE = aS + parseI64(a.lengthTicks);
  const bS = parseI64(b.startTicks);
  const bE = bS + parseI64(b.lengthTicks);
  return aS < bE && bS < aE;
}

export function regionContainsPoint(r: Region, ticks: string): boolean {
  const t = parseI64(ticks);
  const s = parseI64(r.startTicks);
  return t >= s && t < s + parseI64(r.lengthTicks);
}

/** Strict validity: positive length, parseable i64 fields. */
export function isValidRegion(r: Region): boolean {
  try {
    parseI64(r.startTicks);
    return parseI64(r.lengthTicks) > 0n;
  } catch {
    return false;
  }
}

export function takeRegion(t: TakeRecord): Region {
  return { startTicks: t.regionStartTicks, lengthTicks: t.regionLengthTicks };
}

/** Validate one take record; throws on malformed identity/geometry. */
export function checkTake(t: TakeRecord): void {
  if (!t.takeId || !t.folderId || !t.trackId) {
    throw new Error('take needs takeId/folderId/trackId');
  }
  if (t.kind === 'AUDIO' && !t.assetId) {
    throw new Error(`audio take ${t.takeId} has no asset_id`);
  }
  if (!isValidRegion(takeRegion(t))) {
    throw new Error(`take ${t.takeId} has a non-positive region`);
  }
  parseI64(t.offsetTicks);
}

/** Create a folder; takes sorted by laneIndex. Duplicate take ids and
 * duplicated lane indexes are rejected. */
export function makeFolder(
  folderId: string,
  trackId: string,
  takes: TakeRecord[] = [],
): TakeFolder {
  const ids = new Set<string>();
  const lanes = new Set<number>();
  for (const t of takes) {
    checkTake(t);
    if (t.folderId !== folderId) {
      throw new Error(`take ${t.takeId} belongs to ${t.folderId}, not ${folderId}`);
    }
    if (t.trackId !== trackId) {
      throw new Error(`take ${t.takeId} is on ${t.trackId}, not ${trackId}`);
    }
    if (ids.has(t.takeId)) throw new Error(`duplicate take id ${t.takeId}`);
    if (lanes.has(t.laneIndex)) throw new Error(`duplicate lane ${t.laneIndex}`);
    ids.add(t.takeId);
    lanes.add(t.laneIndex);
  }
  return {
    folderId,
    trackId,
    takes: [...takes].sort((a, b) => a.laneIndex - b.laneIndex),
  };
}

/** Insert a take (new lane = max lane + 1 unless given). Returns a NEW
 * folder — the input is never mutated. */
export function addTake(
  folder: TakeFolder,
  take: Omit<TakeRecord, 'folderId' | 'trackId' | 'laneIndex'> & { laneIndex?: number },
): TakeFolder {
  const laneIndex =
    take.laneIndex ?? folder.takes.reduce((m, t) => Math.max(m, t.laneIndex + 1), 0);
  const rec: TakeRecord = {
    ...take,
    folderId: folder.folderId,
    trackId: folder.trackId,
    laneIndex,
  };
  return makeFolder(folder.folderId, folder.trackId, [...folder.takes, rec]);
}

/** Takes under a timeline position, in lane order. */
export function takesAt(folder: TakeFolder, ticks: string): TakeRecord[] {
  return folder.takes.filter((t) => regionContainsPoint(takeRegion(t), ticks));
}

/** The topmost take under a position (latest lane wins). */
export function takeAt(folder: TakeFolder, ticks: string): TakeRecord | null {
  const hits = takesAt(folder, ticks);
  return hits.length ? hits[hits.length - 1] : null;
}

/**
 * Loop-record pass → take records (REC-04 loop takes). Each pass is a
 * descriptor of one recording cycle over `region`; `complete:false`
 * marks an interrupted pass (recovered chunks stay — they are a take,
 * not deleted media).
 */
export function loopTakes(args: {
  folderId: string;
  trackId: string;
  region: Region;
  passes: Array<{
    takeId: string;
    assetId?: string;
    sourceHash?: string;
    offsetTicks?: string;
    complete?: boolean;
    recordedAt?: string;
  }>;
  kind?: 'AUDIO' | 'MIDI';
}): TakeFolder {
  const kind = args.kind ?? 'AUDIO';
  const takes: TakeRecord[] = args.passes.map((p, i) => {
    const rec: TakeRecord = {
      takeId: p.takeId,
      folderId: args.folderId,
      trackId: args.trackId,
      kind,
      laneIndex: i,
      assetId: p.assetId,
      sourceHash: p.sourceHash,
      regionStartTicks: args.region.startTicks,
      regionLengthTicks: args.region.lengthTicks,
      offsetTicks: p.offsetTicks ?? '0',
      complete: p.complete ?? true,
      recordedAt: p.recordedAt,
    };
    checkTake(rec);
    return rec;
  });
  return makeFolder(args.folderId, args.trackId, takes);
}
