// Arrangement feature store (W17; DOC-02, ARR-03/05/06/07, TIME-04).
//
// Separate zustand store — the studioStore view-state gate
// (assertViewStateOnly) rejects feature keys there, so
// alternatives/sections/markers/fades/folders/groups/protection/
// freeze/streaming live here. All values are transient view state:
// no PCM, no song clone, no undo stack.

import { createStore, type StoreApi } from 'zustand/vanilla';
import type {
  Marker,
  ProjectAlternative,
  Section,
  TrackAlternative,
  TrackFolder,
} from './types';
import type { ClipFades } from './fades';
import type { EditGroup } from './groups';
import type { FrozenTrackView, FreezeSpec } from './freeze';
import type { StreamCache } from './stream';
import type { Region } from '../takes/takes';

export interface ArrangementState {
  /** Saved track alternatives keyed by alternativeId (DOC-02). */
  trackAlternatives: Record<string, TrackAlternative>;
  /** Saved project alternatives keyed by alternativeId. */
  projectAlternatives: Record<string, ProjectAlternative>;
  /** Global sections keyed by sectionId (TIME-04). */
  sections: Record<string, Section>;
  /** Position markers keyed by markerId. */
  markers: Record<string, Marker>;
  /** Folder/stack grouping keyed by folderId (ARR-03/06). */
  folders: Record<string, TrackFolder>;
  /** Edit groups keyed by groupId (ARR-06). */
  groups: Record<string, EditGroup>;
  /** Fade specs keyed by clipId (ARR-05, view-state per NEEDS). */
  fades: Record<string, ClipFades>;
  /** Protected clip ids (ARR-07 client-side enforcement). */
  protectedClipIds: Record<string, true>;
  protectedTrackIds: Record<string, true>;
  protectedRegions: { trackId: string; region: Region }[];
  /** Hidden tracks (ARR-03 view filter). */
  hiddenTrackIds: Record<string, true>;
  /** Freeze specs + frozen track views (ENG-05; T68). */
  freezeSpecs: Record<string, FreezeSpec>;
  frozenTracks: Record<string, FrozenTrackView>;
  /** Streaming caches keyed by assetId (TIME-06; T68). */
  streams: Record<string, StreamCache>;
  /** Last applied alternative transaction (for undo affordance). */
  lastApply: { alternativeId: string; transactionId: string } | null;
}

export type ArrangementStore = StoreApi<ArrangementState>;

export function createArrangementStore(): ArrangementStore {
  return createStore<ArrangementState>(() => ({
    trackAlternatives: {},
    projectAlternatives: {},
    sections: {},
    markers: {},
    folders: {},
    groups: {},
    fades: {},
    protectedClipIds: {},
    protectedTrackIds: {},
    protectedRegions: [],
    hiddenTrackIds: {},
    freezeSpecs: {},
    frozenTracks: {},
    streams: {},
    lastApply: null,
  }));
}
