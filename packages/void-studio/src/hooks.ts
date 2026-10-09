// React bindings: a default app-level store plus hooks. Tests use
// createStudioStore() directly — no React needed.

import { useStore } from 'zustand';
import { createStudioStore, StudioStore, StudioViewState, StudioActions } from './store';
import {
  createTakeStudioStore,
  TakeStudioActions,
  TakeStudioState,
  TakeStudioStore,
} from './takes/store';

/** The default workspace store for the app shell. */
export const studioStore: StudioStore = createStudioStore();

/** Shared take/comp view store — record lanes and the comp screen read
 * the same folders, so a take the TAKE_LIST view reports in one
 * workspace is comp-able in the other. */
export const takeStudioStore: TakeStudioStore = createTakeStudioStore();

export function useTakes<T>(
  selector: (s: TakeStudioState & { actions: TakeStudioActions }) => T,
): T {
  return useStore(takeStudioStore, selector);
}

export function useStudio<T>(selector: (s: StudioViewState & { actions: StudioActions }) => T): T {
  return useStore(studioStore, selector);
}

export function useStudioActions(): StudioActions {
  return useStore(studioStore, (s) => s.actions);
}

export function useStudioStore<T>(
  selector: (s: StudioViewState & { actions: StudioActions }) => T,
): T {
  return useStore(studioStore, selector);
}
