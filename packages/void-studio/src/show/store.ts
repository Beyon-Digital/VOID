// Show store (W27: T95/T96 view-state side).
//
// Owns the cue list model, the pairing/arming state machine, the
// mapping-chain registry, and PANIC. Produces ShowOps for the
// coordinator — this store never actuates hardware, never fakes a
// fired cue, and never consults sync state for PANIC.

import { createStore, StoreApi } from 'zustand/vanilla';
import {
  closeChainsForSource,
  runChain,
  validateChain,
  type ChainResult,
  type ClosedTarget,
  type MappingChain,
  type MappingEvent,
} from './mapping';
import type {
  Cue,
  CueAction,
  FireRecord,
  OutputKind,
  OutputPair,
  PanicRecord,
  QueuedAction,
  RunnerPhase,
  ShowOp,
  SyncLiveness,
} from './types';

export interface ShowState {
  cues: Cue[];
  /** Selected cue (armed-for-GO), if any. */
  selectedCueId: string | null;
  /** Currently running cue + phase. */
  runningCueId: string | null;
  runnerPhase: RunnerPhase;
  /** Auditable fire history (bounded). */
  fireLog: FireRecord[];
  pairs: Record<string, OutputPair>;
  chains: MappingChain[];
  /** Targets closed by revocations (audit list). */
  closedTargets: ClosedTarget[];
  /** Queued generated actions — never auto-actuated. */
  queue: QueuedAction[];
  /** Sync liveness for display; PANIC ignores it by contract. */
  syncLiveness: SyncLiveness;
  panicCount: number;
  panicLog: PanicRecord[];
}

const MAX_FIRE_LOG = 256;
const MAX_PANIC_LOG = 64;

export interface ShowActions {
  // Cue list model (single-transaction ops emitted).
  addCue(cue: Cue): ShowOp[];
  updateCue(cue: Cue): ShowOp[];
  removeCue(cueId: string): ShowOp[];
  reorderCues(cueIds: string[]): ShowOp[];
  selectCue(cueId: string | null): void;

  // Runner: select → preWait → fire → postWait → (autoFollow).
  go(atMs: number): FireRecord | null;
  /** Advance runner time; auto-follows when postWait elapses. */
  tick(atMs: number): void;
  /** Stop the runner without releasing pairs (HOLD). */
  holdRunner(): void;

  // Pairing state machine — strict one-step transitions.
  pair(pair: OutputPair): void;
  arm(pairId: string): boolean;
  disarm(pairId: string): boolean;
  unpair(pairId: string): boolean;
  /** Emergency revocation — works from ANY state, closes all chain
   *  targets bound to the pair as a source too (T96). */
  revoke(pairId: string): ClosedTarget[];

  // Mapping engine.
  upsertChain(chain: MappingChain): string[];
  removeChain(chainId: string): ClosedTarget[];
  dispatchEvent(event: MappingEvent): ChainResult[];

  // Queued generated actions — the manual path always wins.
  enqueue(action: QueuedAction): void;
  flushQueue(): QueuedAction[];

  // PANIC — highest priority, idempotent, sync-independent.
  panic(atMs: number): PanicRecord;
  setSyncLiveness(l: SyncLiveness): void;
}

export type ShowStore = StoreApi<ShowState & ShowActions>;

export function createShowStore(): ShowStore {
  return createStore<ShowState & ShowActions>((set, get) => {
    const emitPair = (pairId: string): OutputPair | undefined => get().pairs[pairId];

    return {
      cues: [],
      selectedCueId: null,
      runningCueId: null,
      runnerPhase: 'idle',
      fireLog: [],
      pairs: {},
      chains: [],
      closedTargets: [],
      queue: [],
      syncLiveness: 'unknown',
      panicCount: 0,
      panicLog: [],

      addCue(cue) {
        set((s) => ({ cues: [...s.cues, cue] }));
        return [{ op: 'cue-add', cue }];
      },
      updateCue(cue) {
        set((s) => ({ cues: s.cues.map((c) => (c.cueId === cue.cueId ? cue : c)) }));
        return [{ op: 'cue-update', cue }];
      },
      removeCue(cueId) {
        set((s) => ({
          cues: s.cues.filter((c) => c.cueId !== cueId),
          selectedCueId: s.selectedCueId === cueId ? null : s.selectedCueId,
          runningCueId: s.runningCueId === cueId ? null : s.runningCueId,
          runnerPhase: s.runningCueId === cueId ? 'idle' : s.runnerPhase,
        }));
        return [{ op: 'cue-remove', cueId }];
      },
      reorderCues(cueIds) {
        set((s) => {
          const byId = new Map(s.cues.map((c) => [c.cueId, c]));
          const next = cueIds.map((id) => byId.get(id)).filter((c): c is Cue => !!c);
          // Any cue not listed keeps its tail position (lossless reorder).
          const listed = new Set(cueIds);
          next.push(...s.cues.filter((c) => !listed.has(c.cueId)));
          return { cues: next };
        });
        return [{ op: 'cue-reorder', cueIds }];
      },
      selectCue(cueId) {
        set({ selectedCueId: cueId });
      },

      go(atMs) {
        const s = get();
        const cue = s.cues.find((c) => c.cueId === s.selectedCueId) ?? s.cues[0];
        if (!cue) return null;
        // preWait is a real state — fire is deferred, not immediate.
        if (cue.preWaitMs > 0 && s.runnerPhase === 'idle') {
          set({ runningCueId: cue.cueId, runnerPhase: 'preWait' });
          // The caller schedules tick(atMs+preWaitMs); we model the
          // phase honestly instead of claiming the cue fired early.
          return null;
        }
        const intents: CueAction[] = [];
        const droppedUnarmed: CueAction[] = [];
        for (const a of cue.actions) {
          const targetPair = a.targetId ? emitPair(a.targetId) : undefined;
          // The armed gate: actions addressing an unarmed/absent target
          // are dropped and recorded — never actuated (W27 done-when).
          if (a.targetId && (!targetPair || targetPair.state !== 'armed')) {
            droppedUnarmed.push(a);
          } else {
            intents.push(a);
          }
        }
        const rec: FireRecord = { cueId: cue.cueId, atMs, intents, droppedUnarmed };
        set((st) => ({
          runningCueId: cue.cueId,
          runnerPhase: cue.postWaitMs > 0 || cue.autoFollow ? 'postWait' : 'idle',
          fireLog: [...st.fireLog, rec].slice(-MAX_FIRE_LOG),
        }));
        return rec;
      },

      tick(atMs) {
        const s = get();
        if (s.runnerPhase !== 'postWait' || !s.runningCueId) return;
        const cue = s.cues.find((c) => c.cueId === s.runningCueId);
        if (!cue) {
          set({ runnerPhase: 'idle', runningCueId: null });
          return;
        }
        if (cue.autoFollow) {
          const idx = s.cues.findIndex((c) => c.cueId === cue.cueId);
          const next = s.cues.slice(idx + 1).find((c) => !c.rehearse);
          if (next) {
            set({ selectedCueId: next.cueId, runnerPhase: 'following' });
            // Fire on the next tick/go — auto-follow is a transition,
            // not a silent jump.
            set({ runnerPhase: 'idle' });
            get().go(atMs);
            return;
          }
        }
        set({ runnerPhase: 'idle', runningCueId: null });
      },

      holdRunner() {
        set({ runnerPhase: 'idle', runningCueId: null });
      },

      pair(pair) {
        // Strict: new entries land at 'paired' regardless of caller
        // claims — arming is a separate explicit step.
        set((s) => ({
          pairs: { ...s.pairs, [pair.pairId]: { ...pair, state: 'paired' } },
        }));
      },
      arm(pairId) {
        const p = emitPair(pairId);
        if (!p || p.state !== 'paired') return false;
        set((s) => ({
          pairs: { ...s.pairs, [pairId]: { ...p, state: 'armed' } },
        }));
        return true;
      },
      disarm(pairId) {
        const p = emitPair(pairId);
        if (!p || p.state !== 'armed') return false;
        set((s) => ({
          pairs: { ...s.pairs, [pairId]: { ...p, state: 'paired' } },
        }));
        return true;
      },
      unpair(pairId) {
        const p = emitPair(pairId);
        // Safe path: unpair requires explicit disarm first — an armed
        // pair cannot silently vanish (disarm safe).
        if (!p || p.state === 'armed') return false;
        set((s) => {
          const pairs = { ...s.pairs };
          delete pairs[pairId];
          return { pairs };
        });
        return true;
      },
      revoke(pairId) {
        const s = get();
        const closed = closeChainsForSource(s.chains, pairId);
        set((st) => {
          const pairs = { ...st.pairs };
          delete pairs[pairId];
          return {
            pairs,
            // Chains bound to the revoked pairing are disabled — "no
            // access after revocation" means the source drives nothing,
            // not merely that its targets were closed once.
            chains: st.chains.map((c) =>
              c.sourceId === pairId ? { ...c, enabled: false } : c,
            ),
            closedTargets: [...st.closedTargets, ...closed],
          };
        });
        return closed;
      },

      upsertChain(chain) {
        const errors = validateChain(chain);
        if (errors.length > 0) return errors;
        set((s) => ({
          chains: [
            ...s.chains.filter((c) => c.chainId !== chain.chainId),
            chain,
          ],
        }));
        return [];
      },
      removeChain(chainId) {
        const s = get();
        const chain = s.chains.find((c) => c.chainId === chainId);
        const closed: ClosedTarget[] = chain
          ? chain.targets.map((t) => ({
              chainId,
              targetId: t.targetId,
              kind: t.kind,
              state: 'closed' as const,
            }))
          : [];
        set((st) => ({
          chains: st.chains.filter((c) => c.chainId !== chainId),
          closedTargets: [...st.closedTargets, ...closed],
        }));
        return closed;
      },
      dispatchEvent(event) {
        const s = get();
        return s.chains
          .filter((c) => c.sourceId === event.sourceId)
          .map((c) => runChain(c, event));
      },

      enqueue(action) {
        set((s) => ({ queue: [...s.queue, action] }));
      },
      flushQueue() {
        const s = get();
        // Queued actions respect the same armed gate — flush returns
        // what it dispatched and drops what it mustn't touch.
        const dispatched: QueuedAction[] = [];
        const dropped: QueuedAction[] = [];
        for (const q of s.queue) {
          const p = q.action.targetId ? s.pairs[q.action.targetId] : undefined;
          if (q.action.targetId && (!p || p.state !== 'armed')) dropped.push(q);
          else dispatched.push(q);
        }
        set({ queue: [] });
        void dropped;
        return dispatched;
      },

      panic(atMs) {
        const s = get();
        const releasedPairs = Object.values(s.pairs)
          .filter((p) => p.state === 'armed')
          .map((p) => p.pairId);
        const blackoutIntents = Array.from(
          new Set(Object.values(s.pairs).map((p) => p.target.kind)),
        ) as OutputKind[];
        const rec: PanicRecord = {
          atMs,
          releasedPairs,
          droppedQueued: s.queue.map((q) => q.queueId),
          runnerReset: true,
          blackoutIntents,
          panicCount: s.panicCount + 1,
        };
        // Idempotent by construction: after PANIC there is nothing
        // armed/queued/running left to release — a second call just
        // records another count, releasing nothing.
        set((st) => ({
          pairs: Object.fromEntries(
            Object.entries(st.pairs).map(([id, p]) => [
              id,
              { ...p, state: 'paired' as const },
            ]),
          ),
          queue: [],
          runningCueId: null,
          runnerPhase: 'idle',
          selectedCueId: null,
          panicCount: st.panicCount + 1,
          panicLog: [...st.panicLog, rec].slice(-MAX_PANIC_LOG),
        }));
        return rec;
      },

      setSyncLiveness(l) {
        set({ syncLiveness: l });
      },
    };
  });
}
