// Chord-track region model (TIME-05, T59).
//
// A chord track is an ORDERED set of regions — each carrying a chord
// symbol and a voicing policy. It is UI-side harmonic metadata the same
// way snap settings are: it constrains NEW input and drives the
// explicit harmonic-follow transform; it never silently rewrites notes.
// (TIME-05 acceptance: a region override changes only what the user
// applies it to — original notes are untouched until follow is applied.)
//
// Serialization is provided so the app-state/checkpoint lane can persist
// the track later — the wire protocol major.1 has no chord ops, which is
// documented in docs/gestures/ rather than hacked around.

import { parseI64 } from 'void-client';
import { parseChordSymbol, formatChordSymbol, type ChordSymbol } from './chords';

export type VoicingPolicy = 'close' | 'open' | 'drop2' | 'spread';

export interface ChordRegion {
  regionId: string;
  startTicks: string;
  lengthTicks: string;
  chord: ChordSymbol;
  voicing: VoicingPolicy;
}

export interface ChordTrack {
  /** Sorted, non-overlapping regions (invariant maintained here). */
  regions: ChordRegion[];
}

export function emptyChordTrack(): ChordTrack {
  return { regions: [] };
}

function end(r: { startTicks: string; lengthTicks: string }): bigint {
  return parseI64(r.startTicks) + parseI64(r.lengthTicks);
}

/** The region covering `ticks`, or null. */
export function chordAt(track: ChordTrack, ticks: string): ChordRegion | null {
  const t = parseI64(ticks);
  for (const r of track.regions) {
    if (t >= parseI64(r.startTicks) && t < end(r)) return r;
  }
  return null;
}

function sortRegions(regions: ChordRegion[]): ChordRegion[] {
  return [...regions].sort((a, b) => {
    const d = parseI64(a.startTicks) - parseI64(b.startTicks);
    return d < 0n ? -1 : d > 0n ? 1 : 0;
  });
}

/**
 * Insert/replace a region. Overlap resolution is deterministic — the
 * NEW region wins its whole span; existing regions are trimmed or split
 * around it, and fully covered ones are removed. Same-id regions are
 * replaced (move/resize edits ride this path).
 */
export function upsertRegion(track: ChordTrack, region: ChordRegion): ChordTrack {
  const ns = parseI64(region.startTicks);
  const ne = end(region);
  if (ne <= ns) return track; // reject empty/negative spans
  const out: ChordRegion[] = [];
  for (const r of track.regions) {
    if (r.regionId === region.regionId) continue;
    const rs = parseI64(r.startTicks);
    const re = end(r);
    if (re <= ns || rs >= ne) {
      out.push(r); // disjoint — untouched (TIME-05: no unrelated changes)
      continue;
    }
    if (rs < ns) {
      out.push({
        ...r,
        lengthTicks: (ns - rs).toString(10),
        regionId: r.regionId, // left fragment keeps identity
      });
    }
    if (re > ne) {
      out.push({
        ...r,
        // Right fragment: new id on split (new region = new identity),
        regionId: rs < ns ? `${r.regionId}#r` : r.regionId,
        startTicks: ne.toString(10),
        lengthTicks: (re - ne).toString(10),
      });
    }
    // fully covered → dropped
  }
  out.push({ ...region });
  return { regions: sortRegions(out) };
}

export function removeRegion(track: ChordTrack, regionId: string): ChordTrack {
  return { regions: track.regions.filter((r) => r.regionId !== regionId) };
}

// -- serialization (for later app-state persistence) ---------------------------

interface RegionJson {
  regionId: string;
  startTicks: string;
  lengthTicks: string;
  chord: string;
  voicing: VoicingPolicy;
}

export function serializeChordTrack(track: ChordTrack): string {
  const regions: RegionJson[] = track.regions.map((r) => ({
    regionId: r.regionId,
    startTicks: r.startTicks,
    lengthTicks: r.lengthTicks,
    chord: formatChordSymbol(r.chord),
    voicing: r.voicing,
  }));
  return JSON.stringify({ version: 1, regions });
}

/** Parse a serialized track — invalid rows are skipped, never guessed. */
export function parseChordTrack(json: string): ChordTrack {
  let raw: unknown;
  try {
    raw = JSON.parse(json);
  } catch {
    return emptyChordTrack();
  }
  const rows = (raw as { regions?: unknown })?.regions;
  if (!Array.isArray(rows)) return emptyChordTrack();
  let track = emptyChordTrack();
  for (const row of rows) {
    const r = row as Partial<RegionJson>;
    const chord = typeof r.chord === 'string' ? parseChordSymbol(r.chord) : null;
    if (
      !chord ||
      typeof r.regionId !== 'string' ||
      typeof r.startTicks !== 'string' ||
      typeof r.lengthTicks !== 'string'
    )
      continue;
    const voicing: VoicingPolicy =
      r.voicing === 'open' || r.voicing === 'drop2' || r.voicing === 'spread'
        ? r.voicing
        : 'close';
    track = upsertRegion(track, {
      regionId: r.regionId,
      startTicks: r.startTicks,
      lengthTicks: r.lengthTicks,
      chord,
      voicing,
    });
  }
  return track;
}
