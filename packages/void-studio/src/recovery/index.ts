// Recovery view-state (UIP5/S19/S21).
//
// The studio store keeps only the LATEST telemetry frame per channel —
// a SAVE_FAILED event overwrites the durable one that came before it.
// Recovery screens need both: the failure detail AND the last verified
// checkpoint. This store records them separately, plus the engine-lost
// note and the executable path a restart was last attempted with.
//
// Nothing here fabricates state: entries appear only when the wire
// delivered them. `bindRecovery` is the app-side counterpart of
// `bindStudioClient` for this store.

import { createStore, type StoreApi } from 'zustand/vanilla';
import type { SaveResultEvent, UnlistenFn, VoidClient } from 'void-client';

export interface EngineLostNote {
  /** Worker id reported by the engine-lost event, when present. */
  workerId?: string;
  /** Wall-clock ms when the UI observed the loss. */
  atMs: number;
}

export interface RecoveryState {
  /** Last SaveResultEvent with status SAVE_DURABLE — the last-good checkpoint. */
  lastDurableSave: SaveResultEvent | null;
  /** Last SaveResultEvent with status SAVE_FAILED — the failed write. */
  lastFailedSave: SaveResultEvent | null;
  /** Last engine-lost observation; cleared when the engine re-attaches. */
  lastEngineLost: EngineLostNote | null;
  /** Executable the engine was last spawned with (in-app knowledge only). */
  lastSpawnExecutable: string | null;
}

export interface RecoveryActions {
  noteSaveResult(ev: SaveResultEvent): void;
  noteEngineLost(workerId?: string): void;
  /** Record that the engine re-attached — clears the lost note. */
  noteEngineAttached(): void;
  noteSpawnExecutable(path: string): void;
  reset(): void;
}

export type RecoveryStore = StoreApi<RecoveryState & { actions: RecoveryActions }>;

const initialState = (): RecoveryState => ({
  lastDurableSave: null,
  lastFailedSave: null,
  lastEngineLost: null,
  lastSpawnExecutable: null,
});

export function createRecoveryStore(
  init: Partial<RecoveryState> = {},
): RecoveryStore {
  return createStore<RecoveryState & { actions: RecoveryActions }>((set) => ({
    ...initialState(),
    ...init,
    actions: {
      noteSaveResult(ev) {
        if (ev.status === 'SAVE_DURABLE') {
          set({ lastDurableSave: ev });
        } else if (ev.status === 'SAVE_FAILED') {
          set({ lastFailedSave: ev });
        }
      },
      noteEngineLost(workerId) {
        set({ lastEngineLost: { workerId, atMs: Date.now() } });
      },
      noteEngineAttached() {
        set({ lastEngineLost: null });
      },
      noteSpawnExecutable(path) {
        if (path.trim().length > 0) set({ lastSpawnExecutable: path });
      },
      reset() {
        set(initialState());
      },
    },
  }));
}

/**
 * Subscribe a recovery store to a client's telemetry + engine-lost
 * channels. Returns the combined unlisten.
 */
export function bindRecovery(
  store: RecoveryStore,
  client: VoidClient,
): UnlistenFn {
  const unsubs: UnlistenFn[] = [];
  unsubs.push(
    client.onTelemetry((ev) => {
      if (ev.kind === 'SaveResultEvent') {
        store.getState().actions.noteSaveResult(ev as SaveResultEvent);
      }
    }),
  );
  unsubs.push(
    client.onEngineLost((ev) => {
      store.getState().actions.noteEngineLost(
        (ev as { worker_id?: string }).worker_id,
      );
    }),
  );
  return () => unsubs.forEach((u) => u());
}
