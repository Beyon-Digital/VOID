// Singleton wiring between the Tauri runtime and the void-studio store.
// Created lazily so headless/browser previews never touch the IPC layer
// until the first real interaction.

import { VoidClient } from 'void-client';
import { bindStudioClient, studioStore } from 'void-studio';

let client: VoidClient | null = null;
let startPromise: Promise<void> | null = null;

export function getClient(): VoidClient {
  if (!client) {
    client = VoidClient.tauri();
    bindStudioClient(studioStore, client);
  }
  return client;
}

/**
 * Start event subscriptions + a first engine_status probe. Safe to call
 * repeatedly; failures (e.g. plain-browser preview with no Tauri runtime)
 * resolve to `false` rather than throwing so the UI can render honestly.
 */
export async function ensureClientStarted(): Promise<boolean> {
  const c = getClient();
  startPromise ??= c.start().then(() => undefined);
  try {
    await startPromise;
    const status = await c.engineStatus();
    studioStore.getState().actions.setEngine({
      attached: status.attached,
      workerId: status.worker_id,
      epoch: status.engine_epoch,
      state: status.state,
    });
    return true;
  } catch {
    return false;
  }
}
