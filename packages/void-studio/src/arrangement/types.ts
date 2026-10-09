// Arrangement domain types (W17; DOC-02, ARR-03/05/06/07, TIME-04).
//
// Wire reality: protocol major.1 ops cover clips/notes/tempo/loop-range/
// transport only. Everything here is either (a) a view-state model
// (folders, groups, sections, markers, protected/hidden sets, fades,
// alternatives specs) or (b) a plan that materializes as ordinary clip
// ops under one transaction. The wire gaps are itemized in
// docs/engine/NEEDS.md rev2 items 18+.

import type { Region } from '../takes/takes';

/** A named global section (TIME-04) — a labeled range on the
 * arrangement used for move/copy/delete global edits. View-state:
 * sections have no wire op, but every edit they drive is real ops. */
export interface Section {
  sectionId: string;
  name: string;
  startTicks: string;
  lengthTicks: string;
  color?: string;
}

/** A position marker (TIME-04). View-state until the wire grows
 * marker ops (NEEDS). Absolute-anchor markers pin to ticks. */
export interface Marker {
  markerId: string;
  name: string;
  ticks: string;
  kind: 'marker' | 'section-edge';
}

/** A clip inside an alternative/section snapshot — the arrangement
 * spec, not a document object. */
export interface AlternativeClipSpec {
  kind: 'AUDIO' | 'MIDI';
  assetId?: string;
  startTicks: string;
  lengthTicks: string;
  offsetTicks: string;
  name?: string;
}

/** A named track alternative (DOC-02): one saved arrangement of a
 * track's clip list. Applying it is remove+insert in one transaction
 * — immutable assets are referenced, never duplicated. */
export interface TrackAlternative {
  alternativeId: string;
  trackId: string;
  name: string;
  clips: AlternativeClipSpec[];
}

/** A named project alternative (DOC-02): saved arrangements for a
 * set of tracks. */
export interface ProjectAlternative {
  alternativeId: string;
  name: string;
  /** Per-track clip specs, keyed by trackId. */
  tracks: Record<string, AlternativeClipSpec[]>;
}

/** Folder track (ARR-03 folders / ARR-06 stacks): grouping view
 * state. Membership is explicit track-id list; ordering follows the
 * caller's track list. No wire field — NEEDS. */
export interface TrackFolder {
  folderId: string;
  name: string;
  memberTrackIds: string[];
  collapsed: boolean;
  /** 'folder' = pure grouping; 'stack' = sum/group semantics marker
   *  for downstream routing work (no routing ops on the wire yet). */
  kind: 'folder' | 'stack';
}

/** Edit group (ARR-06): an op applied to any member fans out to all
 * member clips in one transaction via groupEditOps. */
export interface EditGroup {
  groupId: string;
  name: string;
  trackIds: string[];
}

/** Which lanes a section move carries (TIME-04: clips + automation +
 * markers together). Automation has no wire ops yet, so "automation"
 * membership is recorded in the plan spec for the engine lane. */
export interface SectionCarry {
  clips: boolean;
  markers: boolean;
  /** Automation/chords travel as documented spec only (no ops). */
  automation: boolean;
  chords: boolean;
}

export const CARRY_ALL: SectionCarry = {
  clips: true,
  markers: true,
  automation: true,
  chords: true,
};

export function regionOf(startTicks: string, lengthTicks: string): Region {
  return { startTicks, lengthTicks };
}
