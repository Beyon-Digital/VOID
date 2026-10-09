// Show-control types (W27: T95/T96 model side).
//
// This module is view-state + intent descriptions only: a cue's
// actions are *descriptions* of what the coordinator should do when
// the cue fires — this package never actuates hardware, MIDI, OSC or
// DMX. Safety semantics the spec demands are modelled here:
//   - pairing is a one-step-at-a-time state machine;
//   - PANIC is a manual, highest-priority, idempotent release of every
//     output state, and it does not consult sync health at all;
//   - queued/generated actions cannot actuate anything that isn't
//     armed — the armed gate is the safety contract (W27 done-when:
//     "Do not permit generated instructions to directly actuate stage
//     hardware").

/** Output families a cue action may describe. */
export type OutputKind = 'dmx' | 'osc' | 'mmc' | 'audio' | 'internal';

/** A target an armed pair may drive — descriptor, not a handle. */
export interface OutputTarget {
  targetId: string;
  kind: OutputKind;
  /** Human label for show lists. */
  label: string;
  /** Free-form addressing (dmx channel range, OSC address, MMC device). */
  address: string;
}

/** One action inside a cue — an intent, applied by the coordinator. */
export interface CueAction {
  kind: 'set' | 'fade' | 'go' | 'transport';
  targetId?: string;
  /** For set/fade: the intended value (0..1 normalized). */
  value?: number;
  /** For fade: duration ms. */
  fadeMs?: number;
  /** For go: cue id to jump to. For transport: the transport verb. */
  arg?: string;
}

/** A cue: waits, auto-follow, rehearse marker, actions. */
export interface Cue {
  cueId: string;
  name: string;
  /** Wait BEFORE this cue fires once selected (ms). */
  preWaitMs: number;
  /** Wait AFTER firing before auto-follow may advance (ms). */
  postWaitMs: number;
  /** Advance to the next cue automatically after postWait. */
  autoFollow: boolean;
  /** Rehearse marker — rehearsal jumps may land here, show runs skip. */
  rehearse: boolean;
  actions: CueAction[];
}

/** Pairing/arming states — transitions are strictly one step. */
export type PairingState = 'unpaired' | 'paired' | 'armed';

/** A paired output the show may drive (armed pairs only). */
export interface OutputPair {
  pairId: string;
  target: OutputTarget;
  state: PairingState;
  /** Declared permission scope — e.g. 'show:dmx', 'show:osc'.
   *  Permission scopes are enforced, not advisory (T95). */
  scope: string;
}

/** Live cue-runner phase (view state). */
export type RunnerPhase =
  | 'idle'
  /** Inside the armed cue's preWait window. */
  | 'preWait'
  /** Cue fired; inside postWait. */
  | 'postWait'
  /** Auto-follow armed and waiting to take the next cue. */
  | 'following';

/** What the runner produced when a cue fired — an auditable record,
 *  not a claim that hardware moved. */
export interface FireRecord {
  cueId: string;
  atMs: number;
  /** Action intents handed to the coordinator for armed pairs. */
  intents: CueAction[];
  /** Action intents dropped because their target was not armed —
   *  the generated-instruction gate (visible, not silent). */
  droppedUnarmed: CueAction[];
}

/** A pending queued action (from AI proposals, macros, timed lists).
 *  Queued items never bypass the armed gate or PANIC. */
export interface QueuedAction {
  queueId: string;
  origin: 'ai' | 'macro' | 'timed' | 'manual';
  action: CueAction;
}

/** Ops the store emits for the coordinator (single-transaction). */
export type ShowOp =
  | { op: 'cue-add'; cue: Cue }
  | { op: 'cue-update'; cue: Cue }
  | { op: 'cue-remove'; cueId: string }
  | { op: 'cue-reorder'; cueIds: string[] }
  | { op: 'pair'; pair: OutputPair }
  | { op: 'arm'; pairId: string }
  | { op: 'disarm'; pairId: string }
  | { op: 'unpair'; pairId: string }
  | { op: 'revoke'; pairId: string }
  | { op: 'panic' }
  | { op: 'queue-flush'; dropped: string[] };

/** A PANIC release record — proof everything let go. */
export interface PanicRecord {
  atMs: number;
  /** Pairs that were armed → forced to 'closed' release intents. */
  releasedPairs: string[];
  /** Queued actions dropped by the panic. */
  droppedQueued: string[];
  /** Runner was reset (true even if it was idle — PANIC is absolute). */
  runnerReset: boolean;
  /** Blackout intents emitted (one per output kind in use). */
  blackoutIntents: OutputKind[];
  /** Monotonic panic count — idempotence is observable. */
  panicCount: number;
}

/** Sync liveness for display ONLY — PANIC must not read this. */
export type SyncLiveness = 'alive' | 'dead' | 'unknown';
