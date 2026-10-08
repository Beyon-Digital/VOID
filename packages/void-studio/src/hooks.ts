// React bindings: a default app-level store plus hooks. Tests use
// createStudioStore() directly — no React needed.

import { useStore } from 'zustand';
import { createStudioStore, StudioStore, StudioViewState, StudioActions } from './store';

/** The default workspace store for the app shell. */
export const studioStore: StudioStore = createStudioStore();

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
