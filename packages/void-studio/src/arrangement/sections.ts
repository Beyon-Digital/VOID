// Global sections, markers and section-level edits (W17/TIME-04; T67).
//
// A section is a named [start, length) range; section move/copy is a
// global edit across every track: clips fully inside the range travel
// (move = MoveClipOp to destination; copy = duplicate insert at the
// same offset inside the destination), boundary-crossing clips are
// split at the section edge first (SplitClipOp), then the inside part
// travels. One transaction, one undo. Automation/chords have no wire
// ops — the plan records them in `carriedSpec` for the engine lane.
// Markers are view-state (NEEDS rev2); a section move returns the new
// marker positions so the store can apply them together.

import { i64str, parseI64 } from 'void-client';
import type { PersistentOp } from 'void-client';
import type { ClipView } from '../timeline/geometry';
import { regionEnd, regionsOverlap, type Region } from '../takes/takes';
import type { Marker, Section, SectionCarry } from './types';

export type { Marker, Section };

export class SectionError extends Error {}

export function isValidSection(s: Section): boolean {
  try {
    return parseI64(s.lengthTicks) > 0n && parseI64(s.startTicks) >= 0n;
  } catch {
    return false;
  }
}

/** Section ordering for the ruler: sorted by start; overlapping
 * sections are a spec violation (checkSections catches it). */
export function checkSections(sections: Section[]): string[] {
  const errors: string[] = [];
  const sorted = [...sections].sort(
    (a, b) => Number(parseI64(a.startTicks) - parseI64(b.startTicks)),
  );
  for (const s of sorted) {
    if (!isValidSection(s)) errors.push(`section ${s.sectionId} is malformed`);
  }
  for (let i = 1; i < sorted.length; i++) {
    const prev = sorted[i - 1]!;
    const cur = sorted[i]!;
    if (
      parseI64(cur.startTicks) <
      regionEnd({ startTicks: prev.startTicks, lengthTicks: prev.lengthTicks })
    ) {
      errors.push(`sections ${prev.sectionId} and ${cur.sectionId} overlap`);
    }
  }
  const ids = new Set<string>();
  for (const s of sections) {
    if (ids.has(s.sectionId)) errors.push(`duplicate section id ${s.sectionId}`);
    ids.add(s.sectionId);
  }
  return errors;
}

export function sectionRegion(s: Section): Region {
  return { startTicks: s.startTicks, lengthTicks: s.lengthTicks };
}

export interface SectionEditPlan {
  transactionId: string;
  /** Ordered: splits first, then removes, then inserts/moves. */
  ops: PersistentOp[];
  /** Marker positions after the edit: markerId -> new ticks. */
  markerMoves: { markerId: string; ticks: string }[];
  /** Clips that had to be split at the section edge. */
  splitClipIds: string[];
  /** Documented (non-wire) content carried along for the engine lane:
   * automation/chords inside the moved range. */
  carriedSpec: { automation: boolean; chords: boolean };
}

/**
 * Move a section to a new start position. Clips fully inside travel;
 * clips crossing the section boundary are split at the edge, and only
 * the inside part moves (per TIME-04 boundary semantics). Markers
 * inside the section shift by the same delta. One transaction.
 */
export function moveSectionOps(
  section: Section,
  clips: ClipView[],
  markers: Marker[],
  newStartTicks: string,
  carry: SectionCarry,
  mint: () => string,
): SectionEditPlan {
  const delta = parseI64(newStartTicks) - parseI64(section.startTicks);
  if (delta === 0n) {
    return {
      transactionId: mint(), ops: [], markerMoves: [], splitClipIds: [],
      carriedSpec: { automation: carry.automation, chords: carry.chords },
    };
  }
  const region = sectionRegion(section);
  const rStart = parseI64(region.startTicks);
  const rEnd = rStart + parseI64(region.lengthTicks);
  const transactionId = mint();
  const splitOps: PersistentOp[] = [];
  const moveOps: PersistentOp[] = [];
  const splitClipIds: string[] = [];
  for (const clip of carry.clips ? clips : []) {
    if (!regionsOverlap(
      { startTicks: clip.startTicks, lengthTicks: clip.lengthTicks }, region,
    )) continue;
    const cStart = parseI64(clip.startTicks);
    const cEnd = cStart + parseI64(clip.lengthTicks);
    const inside = cStart >= rStart && cEnd <= rEnd;
    if (inside) {
      moveOps.push({
        MoveClipOp: {
          clip_id: clip.clipId,
          track_id: clip.trackId,
          start_ticks: i64str(cStart + delta),
        },
      });
      continue;
    }
    // Boundary-crossing: split at whichever edges are crossed, then move
    // the inside piece(s). A clip may cross both edges (contains region).
    const crossesLeft = cStart < rStart && cEnd > rStart;
    const crossesRight = cStart < rEnd && cEnd > rEnd;
    const splitIds: string[] = [];
    if (crossesLeft) {
      const id = mint();
      splitIds.push(id);
      splitOps.push({
        SplitClipOp: { clip_id: clip.clipId, at_ticks: region.startTicks, new_clip_id: id },
      });
      splitClipIds.push(clip.clipId);
    }
    if (crossesRight) {
      const id = mint();
      splitIds.push(id);
      // After a left split the region-piece id is the split result;
      // otherwise the original clip holds the left part and the new id
      // holds the part after rEnd.
      const targetId = crossesLeft ? splitIds[0]! : clip.clipId;
      splitOps.push({
        SplitClipOp: { clip_id: targetId, at_ticks: i64str(rEnd), new_clip_id: id },
      });
      splitClipIds.push(targetId);
    }
    const insideId = crossesLeft
      ? splitIds[0]!
      : (crossesRight ? clip.clipId : null);
    if (insideId !== null) {
      moveOps.push({
        MoveClipOp: {
          clip_id: insideId,
          track_id: clip.trackId,
          start_ticks: i64str(
            (crossesLeft ? rStart : cStart) + delta,
          ),
        },
      });
    }
  }
  const markerMoves = carry.markers
    ? markers
        .filter((m) => {
          const t = parseI64(m.ticks);
          return t >= rStart && t < rEnd;
        })
        .map((m) => ({ markerId: m.markerId, ticks: i64str(parseI64(m.ticks) + delta) }))
    : [];
  return {
    // splits land before moves so a crossing clip's inside piece exists
    // when its MoveClipOp arrives.
    transactionId, ops: [...splitOps, ...moveOps], markerMoves, splitClipIds,
    carriedSpec: { automation: carry.automation, chords: carry.chords },
  };
}

/**
 * Copy a section to a destination: duplicates the inside clips at the
 * same offset from the destination start (absolute-time copy). One
 * transaction; originals are untouched.
 */
export function copySectionOps(
  section: Section,
  clips: ClipView[],
  newStartTicks: string,
  carry: SectionCarry,
  mint: () => string,
): SectionEditPlan {
  const region = sectionRegion(section);
  const rStart = parseI64(region.startTicks);
  const delta = parseI64(newStartTicks) - rStart;
  const transactionId = mint();
  const ops: PersistentOp[] = [];
  const splitClipIds: string[] = [];
  for (const clip of carry.clips ? clips : []) {
    if (!regionsOverlap(
      { startTicks: clip.startTicks, lengthTicks: clip.lengthTicks }, region,
    )) continue;
    // Only fully-inside clips copy; boundary-crossing clips are skipped
    // (the spec keeps copies clean — the caller pre-splits if needed).
    const cStart = parseI64(clip.startTicks);
    const cEnd = cStart + parseI64(clip.lengthTicks);
    if (!(cStart >= rStart && cEnd <= rStart + parseI64(region.lengthTicks))) {
      continue;
    }
    const clipId = mint();
    const at = i64str(cStart + delta);
    if (clip.kind === 'AUDIO') {
      if (!clip.assetId) throw new SectionError(`clip ${clip.clipId} lacks asset_id`);
      ops.push({
        InsertAudioClipOp: {
          clip_id: clipId,
          track_id: clip.trackId,
          asset_id: clip.assetId,
          start_ticks: at,
          length_ticks: clip.lengthTicks,
          offset_ticks: clip.offsetTicks,
        },
      });
    } else {
      ops.push({
        InsertMidiClipOp: {
          clip_id: clipId, track_id: clip.trackId,
          start_ticks: at, length_ticks: clip.lengthTicks,
        },
      });
    }
  }
  const markerMoves: { markerId: string; ticks: string }[] = [];
  return {
    transactionId, ops, markerMoves, splitClipIds,
    carriedSpec: { automation: carry.automation, chords: carry.chords },
  };
}

/** Marker helpers — view-state only until wire marker ops land. */
export function shiftMarkers(markers: Marker[], moves: { markerId: string; ticks: string }[]): Marker[] {
  const byId = new Map(moves.map((m) => [m.markerId, m.ticks]));
  return markers.map((m) => (byId.has(m.markerId) ? { ...m, ticks: byId.get(m.markerId)! } : m));
}

export const CARRY_EVERYTHING = { clips: true, markers: true, automation: true, chords: true } as const;
