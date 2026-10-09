// Sync view-state types (W27: T94 display/intent side).
//
// Mirrors crates/void-sync for the UI: this store holds what the
// coordinator reports (master claims, arbitration events, drift
// readings) and produces intent requests — it never runs the arbiter
// itself and never claims a sync state the coordinator didn't say.
// String-int64 DTO convention: tick positions/pulse counts that could
// exceed f53 safety cross the boundary as strings.

/** Sync source kinds — mirrors void-sync `SyncSource`. */
export type SyncSourceKind =
  | 'internal'
  | 'midiClockIn'
  | 'mtcIn'
  | 'linkPeer'
  | 'oscExternal';

export interface SyncSourceDescriptor {
  /** Stable id — matches the coordinator's source id string. */
  sourceId: string;
  kind: SyncSourceKind;
  label: string;
  /** Endpoint/prefix/session detail for display. */
  detail?: string;
  /** Declared rate for clock sources (ppm-relevant): pulses/sec. */
  declaredRateHz?: number;
}

/** A drift reading as reported by the coordinator. */
export interface DriftReading {
  sourceId: string;
  /** String-int64 DTOs. */
  expected: string;
  received: string;
  errorPpm: number;
  windowSeconds: number;
  withinTolerance: boolean;
  jumpDetected: boolean;
  atMs: number;
}

/** Health buckets for display — mirrors void-sync `SyncHealth`. */
export type SyncHealthView =
  | { kind: 'locked' }
  | { kind: 'drifting'; errorPpm: number }
  | { kind: 'resync-required' }
  | { kind: 'lost'; sinceMs: number }
  | { kind: 'no-master' };

/** A live master conflict the UI must surface (not auto-resolve). */
export interface MasterConflictView {
  heldSourceId: string;
  attemptedSourceId: string;
  atMs: number;
}

/** Arbiter event for the visible log — mirrors `ArbiterEvent`. */
export interface ArbiterEventView {
  kind:
    | 'master-claimed'
    | 'conflict-rejected'
    | 'switch-scheduled'
    | 'master-switched'
    | 'master-released';
  fromSourceId?: string;
  toSourceId?: string;
  sourceId?: string;
  atMs: number;
}

/** Intents the store emits toward the coordinator — the arbiter
 *  itself lives in void-sync; this is the request surface. */
export type SyncOp =
  | { op: 'sync-request-claim'; sourceId: string }
  | { op: 'sync-request-release'; sourceId: string }
  | {
      op: 'sync-request-resolve';
      heldSourceId: string;
      attemptedSourceId: string;
      resolution: 'keep-held' | 'switch-at-next-stop' | 'switch-immediate';
    }
  | { op: 'sync-transport-stopped' };
