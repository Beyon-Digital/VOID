// Pattern-editor view store (PAT-01, T59).
//
// The pattern being edited lives here as UI-side AUTHORING data — the
// same ownership class as a gesture capture (transient input state).
// It is not the engine document: the pattern only becomes musical data
// when materialized through the shared commit path. Rendering preview
// lists is a pure derivation done on demand (renderPattern) — the store
// never caches a second copy of "the notes".

import { createStore, StoreApi } from 'zustand/vanilla';
import {
  setRowLength,
  setStep,
  toggleStep,
  type PatternStep,
  type StepPattern,
} from './stepPattern';
import { builtinLoops, type LoopTemplate } from './loops';
import type { DrumPad } from './pads';
import { defaultPadBank } from './pads';

export interface PatternEditorState {
  /** Pattern under edit, or null (empty state is honest). */
  pattern: StepPattern | null;
  /** Currently focused cell for keyboard editing. */
  focus: { rowId: string; stepIndex: number } | null;
  /** Loop-browser query text. */
  query: string;
  /** Selected template id (loop browser). */
  templateId: string | null;
  /** Pad bank for the drum surface. */
  pads: DrumPad[];
  /** Slice map currently bound to the surface (id only — spec lives
   *  with the caller; kept as an id ref so the view stays thin). */
  sliceMapId: string | null;
}

export interface PatternEditorActions {
  loadPattern(p: StepPattern): void;
  newPattern(patternId: string, name: string, stepTicks: string, seed: string): void;
  applyTemplate(t: LoopTemplate): void;
  setFocus(rowId: string | null, stepIndex: number): void;
  moveFocus(dRow: number, dStep: number): void;
  toggleFocusedStep(): void;
  patchFocusedStep(patch: Partial<PatternStep>): void;
  setFocusedRowLength(steps: number): void;
  setQuery(q: string): void;
  selectTemplate(id: string | null): void;
  setPads(pads: DrumPad[]): void;
  setSliceMapId(id: string | null): void;
  reset(): void;
}

export type PatternEditorStore = StoreApi<
  PatternEditorState & { actions: PatternEditorActions }
>;

function initial(): PatternEditorState {
  return {
    pattern: null,
    focus: null,
    query: '',
    templateId: null,
    pads: defaultPadBank(),
    sliceMapId: null,
  };
}

export function createPatternEditorStore(
  init?: Partial<PatternEditorState>,
): PatternEditorStore {
  const base = { ...initial(), ...init };
  return createStore<PatternEditorState & { actions: PatternEditorActions }>()(
    (set, get) => ({
      ...base,
      actions: {
        loadPattern(p) {
          set({ pattern: p, focus: null });
        },
        newPattern(patternId, name, stepTicks, seed) {
          set({
            pattern: { patternId, name, stepTicks, seed, rows: [] },
            focus: null,
          });
        },
        applyTemplate(t) {
          // Deep-clone so editing a loaded template never mutates the
          // shared builtin list (browser templates stay pristine).
          set({
            pattern: JSON.parse(JSON.stringify(t.pattern)) as StepPattern,
            templateId: t.id,
            focus: null,
          });
        },
        setFocus(rowId, stepIndex) {
          set({ focus: rowId ? { rowId, stepIndex } : null });
        },
        moveFocus(dRow, dStep) {
          const { pattern, focus } = get();
          if (!pattern || pattern.rows.length === 0) return;
          const rowIdx = focus
            ? pattern.rows.findIndex((r) => r.rowId === focus.rowId)
            : -1;
          const nextRowIdx =
            rowIdx < 0
              ? 0
              : Math.min(pattern.rows.length - 1, Math.max(0, rowIdx + dRow));
          const row = pattern.rows[nextRowIdx];
          const stepIdx = focus
            ? Math.min(row.steps.length - 1, Math.max(0, focus.stepIndex + dStep))
            : 0;
          set({ focus: { rowId: row.rowId, stepIndex: Math.max(0, stepIdx) } });
        },
        toggleFocusedStep() {
          const { pattern, focus } = get();
          if (!pattern || !focus) return;
          set({ pattern: toggleStep(pattern, focus.rowId, focus.stepIndex) });
        },
        patchFocusedStep(patch) {
          const { pattern, focus } = get();
          if (!pattern || !focus) return;
          set({
            pattern: setStep(pattern, focus.rowId, focus.stepIndex, patch),
          });
        },
        setFocusedRowLength(steps) {
          const { pattern, focus } = get();
          if (!pattern || !focus) return;
          set({ pattern: setRowLength(pattern, focus.rowId, steps) });
        },
        setQuery(q) {
          set({ query: q });
        },
        selectTemplate(id) {
          set({ templateId: id });
        },
        setPads(pads) {
          set({ pads });
        },
        setSliceMapId(id) {
          set({ sliceMapId: id });
        },
        reset() {
          set(() => ({ ...initial() }));
        },
      },
    }),
  );
}

/** Loop templates for the browser (stable singleton — never mutated
 *  by applyTemplate, which clones). */
export const BUILTIN_LOOPS: LoopTemplate[] = builtinLoops();
