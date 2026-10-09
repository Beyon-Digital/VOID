// S09 Visuals — screen-scoped stores + hooks.
//
// The visuals/visual-gen/av-panel factories already live in void-studio
// (view-state only); this screen owns the singletons and the output-intent
// store (pure view state — the operator's arm/blackout/display picks,
// pending the visual command channel).

import * as React from 'react';
import { createStore, type StoreApi } from 'zustand/vanilla';
import {
  createAvExportPanelStore,
  createVisGenStore,
  createVisualsStore,
  loadViewPage,
  makeViewKey,
  studioStore,
  useStore,
  useStudio,
  type AvExportPanelStore,
  type VisGenStore,
  type VisualsStore,
} from 'void-studio';
import { getClient } from '../../../client';
import {
  INITIAL_OUTPUT_INTENT,
  type VisualOutputIntent,
} from './model';

export const visualsStore: VisualsStore = createVisualsStore();
export const visGenStore: VisGenStore = createVisGenStore();
export const avPanelStore: AvExportPanelStore = createAvExportPanelStore();

export function useVisuals<T>(selector: (s: ReturnType<VisualsStore['getState']>) => T): T {
  return useStore(visualsStore, selector);
}
export function useVisGen<T>(selector: (s: ReturnType<VisGenStore['getState']>) => T): T {
  return useStore(visGenStore, selector);
}
export function useAvPanel<T>(selector: (s: ReturnType<AvExportPanelStore['getState']>) => T): T {
  return useStore(avPanelStore, selector);
}

// ---------------------------------------------------------------------------
// Output intent — the operator's own state machine state (view state only).
// `armed`/`blackout`/`selectedTarget` become wire ops once the visual
// command channel is bridged; every transition still surfaces its seam note.
// ---------------------------------------------------------------------------

export interface OutputIntentActions {
  selectTarget(target: VisualOutputIntent['selectedTarget']): void;
  /** Arm the output; `note` is the honest result of the route attempt. */
  setArmed(armed: boolean, note?: string | null): void;
  setBlackout(on: boolean): void;
  setNote(note: string | null): void;
  reset(): void;
}

export type OutputIntentStore = StoreApi<VisualOutputIntent & { actions: OutputIntentActions }>;

export function createOutputIntentStore(): OutputIntentStore {
  return createStore<VisualOutputIntent & { actions: OutputIntentActions }>()((set) => ({
    ...INITIAL_OUTPUT_INTENT,
    actions: {
      // Changing the route target always drops the arm — re-arming is an
      // explicit operator action (UI-T23), never implied by selection.
      selectTarget: (selectedTarget) => set({ selectedTarget, armed: false }),
      setArmed: (armed, note = null) => set({ armed, ...(note !== undefined ? { note } : {}) }),
      setBlackout: (blackout) => set({ blackout }),
      setNote: (note) => set({ note }),
      reset: () => set({ ...INITIAL_OUTPUT_INTENT }),
    },
  }));
}

export const outputIntentStore = createOutputIntentStore();

export function useOutputIntent<T>(
  selector: (s: VisualOutputIntent & { actions: OutputIntentActions }) => T,
): T {
  return useStore(outputIntentStore, selector);
}

// ---------------------------------------------------------------------------
// ASSET_LIST read-view feed — same loadViewPage path as every other surface.
// ---------------------------------------------------------------------------

export function useAssetListLoaded() {
  const projectId = useStudio((s) => s.projectId);
  const revision = useStudio((s) => s.revision);
  const attached = useStudio((s) => s.engine.attached);
  const entry = useStudio((s) => s.views[makeViewKey('ASSET_LIST')]);

  React.useEffect(() => {
    if (!projectId || !attached) return;
    void loadViewPage(studioStore, getClient(), 'ASSET_LIST').catch(() => undefined);
  }, [projectId, attached, revision]);

  return { items: entry?.items ?? [], attached, loaded: !!entry };
}
