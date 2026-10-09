// Automation feature store — view state only (W18).
//
// Same convention as arrangement/store.ts: a separate zustand store
// (studioStore's assertViewStateOnly gate rejects feature keys). Lane
// models here are *unsent intent*: parameter writes during a pass are
// real ops, but the recorded curve has no wire persistence in protocol
// major.1 (docs/mix/NEEDS.md) — it lives here as view state, visibly
// not document truth.

import { createStore, type StoreApi } from 'zustand/vanilla';
import type { AutomationLane, AutomationMode, Interpolation } from './types';
import type { WritePass } from './modes';
import type { Region } from '../takes/takes';
import { commitPass } from './modes';
import { moveRegionAutomation } from './region';

export interface AutomationViewState {
  /** Lane models keyed by lane id (unsent intent — see header). */
  lanes: Record<string, AutomationLane>;
  /** Lanes made visible per track (UI affordance only). */
  visibleByTrack: Record<string, string[]>;
  /** The lane currently selected for editing, if any. */
  selectedLaneId?: string;
  /** Global write-arm gate (MIX-05 "explicit write-arm protection"). */
  writeArmed: boolean;
  /** The open write pass, if one is recording. */
  activePass?: WritePass;
  /** Last committed mixed transaction id (undo affordance). */
  lastTransactionId?: string;
}

export interface AutomationActions {
  upsertLane(lane: AutomationLane): void;
  removeLane(laneId: string): void;
  setMode(laneId: string, mode: AutomationMode): void;
  setInterpolation(laneId: string, interp: Interpolation): void;
  setLaneVisible(trackId: string, laneId: string, visible: boolean): void;
  select(laneId?: string): void;
  setWriteArmed(armed: boolean): void;
  setActivePass(pass?: WritePass): void;
  /** Commit a closed pass into its lane and clear it. */
  commitActivePass(): void;
  /** Move-with-region applied to every lane touching a track's regions. */
  applyRegionMove(laneIds: string[], region: Region, deltaTicks: string): void;
  setLastTransaction(id?: string): void;
  reset(): void;
}

export type AutomationStore = StoreApi<AutomationViewState & { actions: AutomationActions }>;

export function createAutomationStore(): AutomationStore {
  const base: AutomationViewState = {
    lanes: {},
    visibleByTrack: {},
    writeArmed: false,
  };
  return createStore<AutomationViewState & { actions: AutomationActions }>()(
    (set, get) => ({
      ...base,
      actions: {
        upsertLane: (lane) =>
          set((s) => ({ lanes: { ...s.lanes, [lane.id]: lane } })),
        removeLane: (laneId) =>
          set((s) => {
            const lanes = { ...s.lanes };
            delete lanes[laneId];
            const visibleByTrack = Object.fromEntries(
              Object.entries(s.visibleByTrack).map(([t, ids]) => [
                t,
                ids.filter((id) => id !== laneId),
              ]),
            );
            return { lanes, visibleByTrack };
          }),
        setMode: (laneId, mode) =>
          set((s) => ({
            lanes: {
              ...s.lanes,
              [laneId]: { ...s.lanes[laneId], mode },
            },
          })),
        setInterpolation: (laneId, interp) =>
          set((s) => ({
            lanes: {
              ...s.lanes,
              [laneId]: { ...s.lanes[laneId], interpolation: interp },
            },
          })),
        setLaneVisible: (trackId, laneId, visible) =>
          set((s) => {
            const cur = s.visibleByTrack[trackId] ?? [];
            const next = visible
              ? cur.includes(laneId)
                ? cur
                : [...cur, laneId]
              : cur.filter((id) => id !== laneId);
            return { visibleByTrack: { ...s.visibleByTrack, [trackId]: next } };
          }),
        select: (laneId) => set({ selectedLaneId: laneId }),
        setWriteArmed: (armed) => set({ writeArmed: armed }),
        setActivePass: (pass) => set({ activePass: pass }),
        commitActivePass: () => {
          const { activePass, lanes } = get();
          if (!activePass) return;
          const lane = lanes[activePass.laneId];
          if (!lane) {
            set({ activePass: undefined });
            return;
          }
          set({
            lanes: { ...lanes, [lane.id]: commitPass(lane, activePass) },
            activePass: undefined,
          });
        },
        applyRegionMove: (laneIds, region, deltaTicks) =>
          set((s) => {
            const lanes = { ...s.lanes };
            for (const id of laneIds) {
              if (lanes[id]) {
                lanes[id] = moveRegionAutomation(lanes[id], region, deltaTicks);
              }
            }
            return { lanes };
          }),
        setLastTransaction: (id) => set({ lastTransactionId: id }),
        reset: () =>
          set({
            lanes: {},
            visibleByTrack: {},
            writeArmed: false,
            selectedLaneId: undefined,
            activePass: undefined,
            lastTransactionId: undefined,
          }),
      },
    }),
  );
}
