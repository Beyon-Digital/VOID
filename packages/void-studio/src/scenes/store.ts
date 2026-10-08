// Scene feature store (W17/PAT-02..05): the launch grid, launch
// phases and the capture record — separate zustand store, all
// transient view state.

import { createStore, type StoreApi } from 'zustand/vanilla';
import type { LaunchState, SceneGrid, SceneSlot } from './scenes';
import { launchScene, makeLaunchState, settleLaunch, stopAll, slotKey } from './scenes';
import type { CaptureRecord } from './variation';
import type { LaunchQuantize, Scene } from './scenes';
import { makeGrid, setSlot } from './scenes';
import type { MeterEvent } from '../arrangement/tempo';

export interface SceneState {
  grid: SceneGrid;
  launch: LaunchState;
  capture: CaptureRecord | null;
  capturing: boolean;
  /** Default quantize for the grid (UI control). */
  defaultQuantize: LaunchQuantize;
  meters: MeterEvent[];
}

export interface SceneActions {
  setGrid(grid: SceneGrid): void;
  setSlot(slot: SceneSlot): void;
  launch(sceneId: string, nowTicks: string, q?: LaunchQuantize): string;
  settle(atTicks: string): void;
  stopAll(nowTicks: string, q?: LaunchQuantize): string;
  setDefaultQuantize(q: LaunchQuantize): void;
  setMeters(meters: MeterEvent[]): void;
  setCapture(rec: CaptureRecord | null): void;
  setCapturing(v: boolean): void;
}

export type SceneStore = StoreApi<SceneState & { actions: SceneActions }>;

export function createSceneStore(init?: {
  scenes?: Scene[];
  trackIds?: string[];
}): SceneStore {
  return createStore<SceneState & { actions: SceneActions }>((set, get) => ({
    grid: makeGrid(init?.scenes ?? [], init?.trackIds ?? []),
    launch: makeLaunchState(),
    capture: null,
    capturing: false,
    defaultQuantize: 'bar',
    meters: [{ atTicks: '0', numerator: 4, denominator: 4 }],
    actions: {
      setGrid: (grid) => set({ grid }),
      setSlot: (slot) => set((s) => ({ grid: setSlot(s.grid, slot) })),
      launch: (sceneId, nowTicks, q) => {
        const s = get();
        const { state, atTicks } = launchScene(
          s.grid,
          s.launch,
          sceneId,
          nowTicks,
          q ?? s.defaultQuantize,
          s.meters,
        );
        set({ launch: state });
        return atTicks;
      },
      settle: (atTicks) => set((s) => ({ launch: settleLaunch(s.launch, atTicks) })),
      stopAll: (nowTicks, q) => {
        const s = get();
        const { state, atTicks } = stopAll(
          s.launch,
          nowTicks,
          q ?? s.defaultQuantize,
          s.meters,
        );
        set({ launch: state });
        return atTicks;
      },
      setDefaultQuantize: (q) => set({ defaultQuantize: q }),
      setMeters: (meters) => set({ meters }),
      setCapture: (rec) => set({ capture: rec }),
      setCapturing: (v) => set({ capturing: v }),
    },
  }));
}

export function slotPhase(store: SceneStore, slotId: string): string {
  return store.getState().launch.slots[slotId]?.phase ?? 'stopped';
}
