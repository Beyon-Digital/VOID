// TAKE_LIST (protocol minor 1) → take-store projection.
//
// The engine emits one row per take; folders are derived view state —
// rows sharing a folderId become one TakeFolder ordered by laneIndex.
// Malformed rows drop silently (never a fabricated take).

import type { ReadItem } from 'void-client';
import type { TakeFolder, TakeKind, TakeRecord } from './types';

/** Parse one TAKE_LIST summary into a TakeRecord; null when malformed. */
export function parseTakeRecord(item: ReadItem): TakeRecord | null {
  try {
    const v = JSON.parse(item.summary_json) as Record<string, unknown>;
    const takeId = typeof v.takeId === 'string' ? v.takeId : '';
    const folderId = typeof v.folderId === 'string' ? v.folderId : '';
    const trackId = typeof v.trackId === 'string' ? v.trackId : '';
    if (!takeId || !folderId || !trackId) return null;
    const kind: TakeKind = v.kind === 'MIDI' ? 'MIDI' : 'AUDIO';
    return {
      takeId,
      folderId,
      trackId,
      kind,
      laneIndex: typeof v.laneIndex === 'number' ? v.laneIndex : 0,
      assetId: typeof v.assetId === 'string' ? v.assetId : undefined,
      sourceHash: typeof v.sourceHash === 'string' ? v.sourceHash : undefined,
      regionStartTicks:
        typeof v.regionStartTicks === 'string' ? v.regionStartTicks : '0',
      regionLengthTicks:
        typeof v.regionLengthTicks === 'string' ? v.regionLengthTicks : '0',
      offsetTicks: typeof v.offsetTicks === 'string' ? v.offsetTicks : '0',
      complete: v.complete !== false,
      recordedAt: typeof v.recordedAt === 'string' ? v.recordedAt : undefined,
    };
  } catch {
    return null;
  }
}

/** Group TAKE_LIST rows into folders, takes ordered by laneIndex. */
export function foldersFromTakeRows(items: ReadItem[]): TakeFolder[] {
  const byFolder = new Map<string, TakeRecord[]>();
  for (const it of items) {
    const rec = parseTakeRecord(it);
    if (!rec) continue;
    const list = byFolder.get(rec.folderId) ?? [];
    list.push(rec);
    byFolder.set(rec.folderId, list);
  }
  return [...byFolder.entries()].map(([folderId, takes]) => ({
    folderId,
    trackId: takes[0].trackId,
    takes: takes.sort((a, b) => a.laneIndex - b.laneIndex),
  }));
}
