// S04 app-level singletons — feature stores live per feature (the studio
// store stays view-state-only). Created once per app; survives route hops.

import {
  createAutomationStore,
  createMixerViewStore,
  loadViewPage,
  studioStore,
  type MixerBinding,
} from 'void-studio';
import { getClient } from '../../../client';

export const mixView = createMixerViewStore();
export const mixAutomation = createAutomationStore();

export function refreshTracks() {
  return loadViewPage(studioStore, getClient(), 'TRACK_LIST').catch(() => undefined);
}

/** Fresh binding per gesture — one gesture = one transaction id. */
export function mixerBinding(): MixerBinding {
  return {
    client: getClient(),
    store: mixView,
    refreshTracks,
    transactionId: crypto.randomUUID(),
  };
}
