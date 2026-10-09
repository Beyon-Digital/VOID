// Shared recovery view-state for the UIP5 screens (S19 engine-stopped,
// S21 save-failed). Bound lazily beside the studio store — it subscribes
// to the same client so a durable checkpoint recorded before a failed
// save is never lost from view.

import { getClient } from '../../../client';
import { useStore } from 'void-studio';
import {
  createRecoveryStore,
  bindRecovery,
  type RecoveryStore,
  type RecoveryState,
  type RecoveryActions,
} from 'void-studio/src/recovery';

export const recoveryStore: RecoveryStore = createRecoveryStore();

let bound = false;

/** Idempotent: subscribes the shared recovery store to the client once. */
export function ensureRecoveryBound(): void {
  if (bound) return;
  bound = true;
  bindRecovery(recoveryStore, getClient());
}

/** React hook: subscribe to the shared recovery store. */
export function useRecovery<T>(
  selector: (s: RecoveryState & { actions: RecoveryActions }) => T,
): T {
  return useStore(recoveryStore, selector);
}
