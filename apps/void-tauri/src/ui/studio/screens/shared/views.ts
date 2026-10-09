// Paged view loading for UIP5 screens — pulls every page of a read view
// (bounded by readViewPages' own maxPages) and mirrors each page into the
// studio store cache so panels share one view of the data.

import { getClient } from '../../../client';
import { loadViewPage, makeViewKey, studioStore } from 'void-studio';
import type { ReadItem, ViewKindName } from 'void-client';

export interface LoadedView {
  items: ReadItem[];
  /** Revisions reported by the pages (last page wins). */
  revision?: string;
}

/**
 * Fetch all pages of a view into the store cache; returns collected items.
 * Throws the underlying read error — callers surface it verbatim.
 */
export async function loadAllPages(
  view: ViewKindName,
  opts: { trackId?: string; startTicks?: string; endTicks?: string; limit?: number } = {},
): Promise<LoadedView> {
  const client = getClient();
  const items: ReadItem[] = [];
  let cursor: string | undefined;
  let revision: string | undefined;
  // readViewPages bounds the loop; each page also merges into the store.
  for (;;) {
    const page = await loadViewPage(studioStore, client, view, { ...opts, cursor });
    items.push(...page.items);
    revision = page.revision;
    if (page.done || !page.next_cursor) break;
    cursor = page.next_cursor;
  }
  return { items, revision };
}

/** Items already in the store cache for a view (synchronous read). */
export function cachedViewItems(
  view: ViewKindName,
  opts: { trackId?: string; startTicks?: string; endTicks?: string } = {},
): ReadItem[] {
  const key = makeViewKey(view, opts.trackId, opts.startTicks, opts.endTicks);
  return studioStore.getState().views[key]?.items ?? [];
}

/** TRACK_LIST row (engine-defined summary shape). */
export interface TrackRow {
  trackId: string;
  kind?: string;
  name?: string;
  index?: number;
}

export function parseTrackRow(item: ReadItem): TrackRow | null {
  try {
    const v = JSON.parse(item.summary_json) as Record<string, unknown>;
    if (typeof v.trackId !== 'string' || v.trackId.length === 0) return null;
    return {
      trackId: v.trackId,
      kind: typeof v.kind === 'string' ? v.kind : undefined,
      name: typeof v.name === 'string' ? v.name : undefined,
      index: typeof v.index === 'number' ? v.index : undefined,
    };
  } catch {
    return null;
  }
}
