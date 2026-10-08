// Chord-track editor view store (TIME-05, T59).
//
// The track itself is harmonic authoring metadata (see track.ts header);
// this store adds the editing surface: selection, the pending region
// draft, and constraint mode. It never touches notes — follow is a
// separate explicit transform on explicit selection.

import { createStore, StoreApi } from 'zustand/vanilla';
import type { ChordSymbol } from './chords';
import {
  emptyChordTrack,
  removeRegion,
  upsertRegion,
  type ChordRegion,
  type ChordTrack,
  type VoicingPolicy,
} from './track';
import type { ConstraintMode } from './constraint';

export interface ChordTrackState {
  track: ChordTrack;
  /** Region being edited/selected (null = none). */
  selectedRegionId: string | null;
  /** Constraint applied to NEW input (off = chromatic passthrough). */
  constraintMode: ConstraintMode;
}

export interface ChordTrackActions {
  upsert(region: ChordRegion): void;
  upsertSymbol(fields: {
    regionId: string;
    startTicks: string;
    lengthTicks: string;
    chord: ChordSymbol;
    voicing?: VoicingPolicy;
  }): void;
  remove(regionId: string): void;
  select(regionId: string | null): void;
  setConstraintMode(mode: ConstraintMode): void;
  loadTrack(track: ChordTrack): void;
  reset(): void;
}

export type ChordTrackStore = StoreApi<
  ChordTrackState & { actions: ChordTrackActions }
>;

export function createChordTrackStore(
  init?: Partial<ChordTrackState>,
): ChordTrackStore {
  const base: ChordTrackState = {
    track: emptyChordTrack(),
    selectedRegionId: null,
    constraintMode: 'off',
    ...init,
  };
  return createStore<ChordTrackState & { actions: ChordTrackActions }>()(
    (set, get) => ({
      ...base,
      actions: {
        upsert(region) {
          set({ track: upsertRegion(get().track, region) });
        },
        upsertSymbol(fields) {
          const region: ChordRegion = {
            regionId: fields.regionId,
            startTicks: fields.startTicks,
            lengthTicks: fields.lengthTicks,
            chord: fields.chord,
            voicing: fields.voicing ?? 'close',
          };
          set({ track: upsertRegion(get().track, region) });
        },
        remove(regionId) {
          const s = get();
          set({
            track: removeRegion(s.track, regionId),
            selectedRegionId:
              s.selectedRegionId === regionId ? null : s.selectedRegionId,
          });
        },
        select(regionId) {
          set({ selectedRegionId: regionId });
        },
        setConstraintMode(mode) {
          set({ constraintMode: mode });
        },
        loadTrack(track) {
          set({ track, selectedRegionId: null });
        },
        reset() {
          set({
            track: emptyChordTrack(),
            selectedRegionId: null,
            constraintMode: 'off',
          });
        },
      },
    }),
  );
}
