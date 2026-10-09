// Bounded asset streaming cache (W17/TIME-06; T68).
//
// The WebView never loads PCM: T68's "assets larger than RAM under a
// bounded cache" is an engine property. What lives here is the
// honest client half — a bounded tile-cache view model that mirrors
// the engine's streaming state for the UI (which tile windows are
// resident, eviction order, hit/miss counters for the perf readout).
// Real bytes flow engine-side only; a tile entry carries metadata,
// never audio data.

import { parseI64 } from 'void-client';
import type { Region } from '../takes/takes';
import { regionsOverlap } from '../takes/takes';

export interface StreamTile {
  assetId: string;
  region: Region;
  /** Monotonic fetch counter for LRU (store-assigned). */
  lastUsed: number;
  state: 'resident' | 'loading' | 'evicted';
}

export interface StreamCache {
  assetId: string;
  /** Max resident tiles — the bound that makes 'larger than RAM' safe. */
  capacity: number;
  tiles: StreamTile[];
  hits: number;
  misses: number;
  evictions: number;
}

export function makeStreamCache(assetId: string, capacity: number): StreamCache {
  return { assetId, capacity, tiles: [], hits: 0, misses: 0, evictions: 0 };
}

/**
 * Request the window [cursor, cursor+window) for an asset: returns
 * the resident tile covering it on hit, else registers a 'loading'
 * tile (the engine fetch is driven elsewhere) and evicts the oldest
 * resident tile past capacity. Pure — returns a NEW cache object.
 */
export function requestWindow(
  cache: StreamCache,
  window: Region,
  tick: number,
): { cache: StreamCache; resident: boolean; evictedTile?: StreamTile } {
  const covering = cache.tiles.find(
    (t) =>
      t.state !== 'evicted' &&
      regionsOverlap(t.region, window) &&
      parseI64(t.region.startTicks) <= parseI64(window.startTicks) &&
      parseI64(t.region.startTicks) + parseI64(t.region.lengthTicks) >=
        parseI64(window.startTicks) + parseI64(window.lengthTicks),
  );
  if (covering) {
    const tiles = cache.tiles.map((t) =>
      t === covering ? { ...t, lastUsed: tick, state: 'resident' as const } : t,
    );
    return {
      cache: { ...cache, tiles, hits: cache.hits + 1 },
      resident: true,
    };
  }
  const resident = cache.tiles.filter((t) => t.state === 'resident');
  let evicted: StreamTile | undefined;
  let tiles = cache.tiles;
  let evictions = cache.evictions;
  if (resident.length >= cache.capacity) {
    const oldest = resident.reduce((a, b) => (a.lastUsed <= b.lastUsed ? a : b));
    evicted = oldest;
    evictions += 1;
    tiles = tiles.map((t) =>
      t === oldest ? { ...t, state: 'evicted' as const } : t,
    );
  }
  const tile: StreamTile = {
    assetId: cache.assetId,
    region: window,
    lastUsed: tick,
    state: 'loading',
  };
  return {
    cache: {
      ...cache,
      tiles: [...tiles.filter((t) => t.state !== 'evicted'), tile],
      misses: cache.misses + 1,
      evictions,
    },
    resident: false,
    ...(evicted ? { evictedTile: evicted } : {}),
  };
}

/** Mark a loading tile resident (engine fetch completed). */
export function tileResident(cache: StreamCache, region: Region, tick: number): StreamCache {
  return {
    ...cache,
    tiles: cache.tiles.map((t) =>
      t.assetId === cache.assetId &&
      t.region.startTicks === region.startTicks &&
      t.region.lengthTicks === region.lengthTicks &&
      t.state === 'loading'
        ? { ...t, state: 'resident', lastUsed: tick }
        : t,
    ),
  };
}

/** Tile windows for a viewport: fixed-size windows over the asset —
 * the renderer asks for exactly these, keeping the resident set
 * bounded regardless of asset length (T68). */
export function tileWindows(
  assetId: string,
  totalTicks: string,
  tileTicks: string,
): Region[] {
  const total = parseI64(totalTicks);
  const size = parseI64(tileTicks);
  const out: Region[] = [];
  for (let s = 0n; s < total; s += size) {
    const len = s + size <= total ? size : total - s;
    out.push({ startTicks: s.toString(), lengthTicks: len.toString() });
  }
  return out;
}
