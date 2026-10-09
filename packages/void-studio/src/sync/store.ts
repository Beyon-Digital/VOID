// Sync view-state store (W27: T94 display/intent side).
//
// Holds the coordinator-reported sync state: declared sources, the
// current master, pending conflicts, drift readings, and the arbiter
// log. UI intents become SyncOps — this store does not decide
// arbitration, it records what the coordinator decided and what the
// user asked for. "No competing transport master" is enforced by the
// arbiter; here a second claim is recorded as a conflict view, never
// applied locally.

import { createStore, StoreApi } from 'zustand/vanilla';
import type {
  ArbiterEventView,
  DriftReading,
  MasterConflictView,
  SyncHealthView,
  SyncOp,
  SyncSourceDescriptor,
} from './types';

export interface SyncState {
  sources: Record<string, SyncSourceDescriptor>;
  /** Current master source id as last reported, or null. */
  masterSourceId: string | null;
  /** A switch the arbiter scheduled for the next transport stop. */
  pendingSwitchTo: string | null;
  /** Open conflicts awaiting user resolution. */
  conflicts: MasterConflictView[];
  /** Latest drift reading per source. */
  drift: Record<string, DriftReading>;
  /** Visible arbitration log (bounded). */
  log: ArbiterEventView[];
  /** Last traffic timestamp per source (liveness input). */
  lastTrafficMs: Record<string, number>;
}

const MAX_LOG = 512;
/** Sources silent longer than this are 'lost' for display. */
export const SYNC_LOST_SILENCE_MS = 2000;

export interface SyncActions {
  declareSource(d: SyncSourceDescriptor): void;
  removeSource(sourceId: string): void;

  /** User requests — returned as SyncOps for the coordinator. */
  requestClaim(sourceId: string): SyncOp;
  requestRelease(sourceId: string): SyncOp;
  requestResolve(
    heldSourceId: string,
    attemptedSourceId: string,
    resolution: 'keep-held' | 'switch-at-next-stop' | 'switch-immediate',
  ): SyncOp;

  /** Coordinator reports — applied verbatim to view state. */
  reportMasterClaimed(sourceId: string, atMs: number): void;
  reportConflict(heldSourceId: string, attemptedSourceId: string, atMs: number): void;
  reportSwitchScheduled(fromSourceId: string, toSourceId: string, atMs: number): void;
  reportMasterSwitched(fromSourceId: string, toSourceId: string, atMs: number): void;
  reportMasterReleased(sourceId: string, atMs: number): void;
  reportDrift(r: DriftReading): void;
  reportTraffic(sourceId: string, atMs: number): void;
  transportStopped(atMs: number): void;

  /** Derived display health — pure view derivation. */
  health(nowMs: number): SyncHealthView;
}

export type SyncStore = StoreApi<SyncState & SyncActions>;

export function createSyncStore(): SyncStore {
  return createStore<SyncState & SyncActions>((set, get) => {
    const pushLog = (e: ArbiterEventView) =>
      set((s) => ({ log: [...s.log, e].slice(-MAX_LOG) }));

    return {
      sources: {},
      masterSourceId: null,
      pendingSwitchTo: null,
      conflicts: [],
      drift: {},
      log: [],
      lastTrafficMs: {},

      declareSource(d) {
        set((s) => ({ sources: { ...s.sources, [d.sourceId]: d } }));
      },
      removeSource(sourceId) {
        set((s) => {
          const sources = { ...s.sources };
          const drift = { ...s.drift };
          const lastTrafficMs = { ...s.lastTrafficMs };
          delete sources[sourceId];
          delete drift[sourceId];
          delete lastTrafficMs[sourceId];
          return {
            sources,
            drift,
            lastTrafficMs,
            masterSourceId: s.masterSourceId === sourceId ? null : s.masterSourceId,
          };
        });
      },

      requestClaim(sourceId) {
        return { op: 'sync-request-claim', sourceId };
      },
      requestRelease(sourceId) {
        return { op: 'sync-request-release', sourceId };
      },
      requestResolve(heldSourceId, attemptedSourceId, resolution) {
        // The user resolved a conflict — drop it from the open list
        // optimistically? No: honest view keeps it until the
        // coordinator reports the outcome (claimed/switched/rejected).
        // The op is the request, not the decision.
        return { op: 'sync-request-resolve', heldSourceId, attemptedSourceId, resolution };
      },

      reportMasterClaimed(sourceId, atMs) {
        set({ masterSourceId: sourceId });
        pushLog({ kind: 'master-claimed', sourceId, atMs });
      },
      reportConflict(heldSourceId, attemptedSourceId, atMs) {
        set((s) => ({
          conflicts: [
            ...s.conflicts.filter(
              (c) => !(c.heldSourceId === heldSourceId && c.attemptedSourceId === attemptedSourceId),
            ),
            { heldSourceId, attemptedSourceId, atMs },
          ],
        }));
        pushLog({ kind: 'conflict-rejected', fromSourceId: heldSourceId, toSourceId: attemptedSourceId, atMs });
      },
      reportSwitchScheduled(fromSourceId, toSourceId, atMs) {
        set((s) => ({
          pendingSwitchTo: toSourceId,
          conflicts: s.conflicts.filter(
            (c) => !(c.heldSourceId === fromSourceId && c.attemptedSourceId === toSourceId),
          ),
        }));
        pushLog({ kind: 'switch-scheduled', fromSourceId, toSourceId, atMs });
      },
      reportMasterSwitched(fromSourceId, toSourceId, atMs) {
        set((s) => ({
          masterSourceId: toSourceId,
          pendingSwitchTo: null,
          conflicts: s.conflicts.filter(
            (c) => !(c.heldSourceId === fromSourceId && c.attemptedSourceId === toSourceId),
          ),
        }));
        pushLog({ kind: 'master-switched', fromSourceId, toSourceId, atMs });
      },
      reportMasterReleased(sourceId, atMs) {
        set((s) => ({
          masterSourceId: s.masterSourceId === sourceId ? null : s.masterSourceId,
        }));
        pushLog({ kind: 'master-released', sourceId, atMs });
      },
      reportDrift(r) {
        set((s) => ({ drift: { ...s.drift, [r.sourceId]: r } }));
      },
      reportTraffic(sourceId, atMs) {
        set((s) => ({ lastTrafficMs: { ...s.lastTrafficMs, [sourceId]: atMs } }));
      },
      transportStopped(atMs) {
        const s = get();
        if (s.pendingSwitchTo && s.masterSourceId) {
          const to = s.pendingSwitchTo;
          set({ masterSourceId: to, pendingSwitchTo: null });
          pushLog({ kind: 'master-switched', fromSourceId: s.masterSourceId, toSourceId: to, atMs });
        }
      },

      health(nowMs) {
        const s = get();
        if (!s.masterSourceId) return { kind: 'no-master' };
        const last = s.lastTrafficMs[s.masterSourceId];
        if (last !== undefined && nowMs - last >= SYNC_LOST_SILENCE_MS) {
          return { kind: 'lost', sinceMs: nowMs - last };
        }
        const r = s.drift[s.masterSourceId];
        if (!r) return { kind: 'locked' };
        if (r.jumpDetected) return { kind: 'resync-required' };
        if (!r.withinTolerance) return { kind: 'drifting', errorPpm: r.errorPpm };
        return { kind: 'locked' };
      },
    };
  });
}
