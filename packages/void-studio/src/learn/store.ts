// Learn registry + arming + dispatch store (MIDI-02, MIX-07, T58).
//
// State machine:
//   - Registry holds mappings + macros (survives arm/disarm).
//   - ARM is explicit and SINGLE: arm(targetId) picks the one target
//     that may move. A control event dispatches ONLY when its mapping
//     resolves to the armed target (T58: "only armed target responds").
//   - Learn capture: beginLearn(targetId) then the next distinct
//     control event binds a mapping for that control.
//   - Momentary controls (down/up) are tracked as HELD; releaseAll()
//     releases every held key (all-notes-off semantics — nothing stays
//     on after disconnect/focus-loss).
//   - panic() takes priority: it drains held controls, disarms, and any
//     pending learn capture — and a post-panic control event is ignored
//     until the user re-arms. panic is safe to call repeatedly.
//
// Dispatch produces ParamChange descriptions — the caller applies them
// to the real parameter owner. This store never sends to the engine.

import { createStore, StoreApi } from 'zustand/vanilla';
import {
  controlKey,
  sameControl,
  type ControlEvent,
  type ControlSpec,
  type Macro,
  type Mapping,
  type ParamChange,
  type TargetSpec,
} from './types';
import { checkMapping, mapMacroValue, mapValue } from './values';

export type LearnPhase = 'idle' | 'capturing';

export interface LearnState {
  mappings: Mapping[];
  macros: Macro[];
  /** The single armed target (kind+id), or null. */
  armedTarget: TargetSpec | null;
  phase: LearnPhase;
  /** Target currently awaiting a control to bind. */
  learnTarget: TargetSpec | null;
  /** Held momentary controls: controlKey → armed atMs. */
  held: Record<string, { control: ControlSpec; atMs: number }>;
  /** Monotonic panic count — lets tests assert panic priority. */
  panicCount: number;
}

export interface DispatchResult {
  /** Parameter updates to apply (possibly fan-out from a macro). */
  changes: ParamChange[];
  /** Why an event produced no changes (for UX/diagnostics). */
  ignored?: 'no-arm' | 'no-mapping' | 'target-not-armed' | 'post-panic-disarmed' | 'out-of-range';
}

export interface LearnActions {
  upsertMapping(m: Mapping): void;
  removeMapping(mappingId: string): void;
  upsertMacro(m: Macro): void;
  removeMacro(macroId: string): void;
  /** Explicitly arm one target; arming another replaces the arm. */
  arm(target: TargetSpec): void;
  disarm(): void;
  /** Start learn capture for a target — next control event binds. */
  beginLearn(target: TargetSpec): void;
  cancelLearn(): void;
  /** Route one control event. Returns changes (and why ignored). */
  dispatch(ev: ControlEvent): DispatchResult;
  /** Disconnect/focus-loss: release held, disarm, drop learn. */
  releaseAll(cause: 'disconnect' | 'focus-loss' | 'navigate'): { released: ControlSpec[] };
  /** PANIC — releaseAll with priority; also marks post-panic state. */
  panic(): { released: ControlSpec[] };
  reset(): void;
}

export type LearnStore = StoreApi<LearnState & { actions: LearnActions }>;

function targetKey(t: TargetSpec | null): string {
  return t ? `${t.kind}:${t.id}` : '';
}

export function createLearnStore(init?: Partial<LearnState>): LearnStore {
  const base: LearnState = {
    mappings: [],
    macros: [],
    armedTarget: null,
    phase: 'idle',
    learnTarget: null,
    held: {},
    panicCount: 0,
    ...init,
  };
  return createStore<LearnState & { actions: LearnActions }>()((set, get) => ({
    ...base,
    actions: {
      upsertMapping(m) {
        checkMapping(m);
        set((s) => {
          const others = s.mappings.filter(
            (x) => x.mappingId !== m.mappingId,
          );
          // A control can bind ONE target — rebind replaces (T58 spec:
          // "Every mapped input updates the same parameter").
          const rest = others.filter((x) => !sameControl(x.control, m.control));
          return { mappings: [...rest, m] };
        });
      },
      removeMapping(mappingId) {
        set((s) => ({ mappings: s.mappings.filter((x) => x.mappingId !== mappingId) }));
      },
      upsertMacro(m) {
        set((s) => ({
          macros: [...s.macros.filter((x) => x.macroId !== m.macroId), m],
        }));
      },
      removeMacro(macroId) {
        set((s) => ({ macros: s.macros.filter((x) => x.macroId !== macroId) }));
      },
      arm(target) {
        set({ armedTarget: { ...target } });
      },
      disarm() {
        set({ armedTarget: null });
      },
      beginLearn(target) {
        set({ phase: 'capturing', learnTarget: { ...target } });
      },
      cancelLearn() {
        set({ phase: 'idle', learnTarget: null });
      },
      dispatch(ev) {
        const s = get();
        const key = controlKey(ev.control);
        // Held tracking — every 'down' lands, every 'up' releases.
        if (ev.phase === 'down') {
          set({ held: { ...s.held, [key]: { control: ev.control, atMs: ev.atMs } } });
        } else if (ev.phase === 'up') {
          const held = { ...s.held };
          delete held[key];
          set({ held });
        }
        // Learn capture consumes the event as a bind — it precedes the
        // arm check because learning normally happens BEFORE arming.
        if (s.phase === 'capturing' && s.learnTarget && ev.phase === 'down') {
          const mapping: Mapping = {
            mappingId: `learn-${key}-${s.learnTarget.id}`,
            control: { ...ev.control },
            target: { ...s.learnTarget },
            curve: 'linear',
            min: 0,
            max: 1,
            invert: false,
            outOfRange: 'clamp',
          };
          get().actions.upsertMapping(mapping);
          set({ phase: 'idle', learnTarget: null });
          return { changes: [] };
        }
        // Post-panic/disarm: controls do nothing until the user
        // explicitly re-arms (panic priority + only-armed rule, T58).
        if (!s.armedTarget) {
          return { changes: [], ignored: 'no-arm' };
        }
        const mapping = s.mappings.find((m) => sameControl(m.control, ev.control));
        if (!mapping) return { changes: [], ignored: 'no-mapping' };
        if (targetKey(mapping.target) !== targetKey(s.armedTarget)) {
          return { changes: [], ignored: 'target-not-armed' };
        }
        if (ev.phase === 'up') {
          // Release events never write parameters.
          return { changes: [] };
        }
        const value = mapValue(mapping, ev.value);
        if (value === null) return { changes: [], ignored: 'out-of-range' };
        const changes: ParamChange[] = [];
        if (mapping.target.kind === 'param') {
          changes.push({
            paramId: mapping.target.id,
            value,
            mappingId: mapping.mappingId,
            atMs: ev.atMs,
          });
        } else {
          const macro = s.macros.find((x) => x.macroId === mapping.target.id);
          if (!macro) return { changes: [], ignored: 'no-mapping' };
          for (const mv of mapMacroValue(macro, value)) {
            changes.push({
              paramId: mv.paramId,
              value: mv.value,
              mappingId: mapping.mappingId,
              atMs: ev.atMs,
            });
          }
        }
        return { changes };
      },
      releaseAll() {
        const s = get();
        const released = Object.values(s.held).map((h) => ({ ...h.control }));
        set({
          held: {},
          armedTarget: null,
          phase: 'idle',
          learnTarget: null,
        });
        return { released };
      },
      panic() {
        const s = get();
        const released = Object.values(s.held).map((h) => ({ ...h.control }));
        set({
          held: {},
          armedTarget: null,
          phase: 'idle',
          learnTarget: null,
          panicCount: s.panicCount + 1,
        });
        return { released };
      },
      reset() {
        set({ ...base, held: {}, panicCount: 0 });
      },
    },
  }));
}
