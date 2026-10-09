// Spatial view-state store (W26: T92/T93 display side).
//
// Holds the declared output paths and their verbatim gate verdicts,
// plus the monitoring-config draft. The gate (void-spatial) decides;
// this store displays — it can surface "unavailable with reason" but
// can never manufacture "available" (the only available state
// requires a validator id supplied by the coordinator).

import { createStore, StoreApi } from 'zustand/vanilla';
import {
  validateMonitoringDraft,
  type GateVerdictView,
  type MonitoringDraft,
  type SpatialOutputView,
} from './types';

export interface SpatialState {
  outputs: Record<string, SpatialOutputView>;
  /** Currently committed monitoring config, if any. */
  monitoring: MonitoringDraft | null;
  /** Draft being edited + its local structural errors. */
  draft: MonitoringDraft | null;
  draftErrors: string[];
}

export interface SpatialActions {
  registerOutput(output: Omit<SpatialOutputView, 'verdict' | 'verdictAtMs'>): void;
  removeOutput(outputId: string): void;
  /** Coordinator-supplied verdict — stored verbatim, never altered. */
  reportVerdict(outputId: string, verdict: GateVerdictView, atMs: number): void;
  /** Read the live verdict — the UI's only availability signal. */
  verdictFor(outputId: string): GateVerdictView | null;
  /** True only when a verdict is available AND came with a validator
   *  record — a convenience for badges; unavailable stays typed. */
  isAvailable(outputId: string): boolean;

  setMonitoring(config: MonitoringDraft): string[];
  editDraft(draft: MonitoringDraft | null): void;
  updateDraft(patch: Partial<MonitoringDraft>): void;
}

export type SpatialStore = StoreApi<SpatialState & SpatialActions>;

export function createSpatialStore(): SpatialStore {
  return createStore<SpatialState & SpatialActions>((set, get) => ({
    outputs: {},
    monitoring: null,
    draft: null,
    draftErrors: [],

    registerOutput(output) {
      set((s) => ({
        outputs: {
          ...s.outputs,
          [output.outputId]: { ...output, verdict: null, verdictAtMs: null },
        },
      }));
    },
    removeOutput(outputId) {
      set((s) => {
        const outputs = { ...s.outputs };
        delete outputs[outputId];
        return { outputs };
      });
    },
    reportVerdict(outputId, verdict, atMs) {
      set((s) => {
        const o = s.outputs[outputId];
        if (!o) return s;
        return {
          outputs: { ...s.outputs, [outputId]: { ...o, verdict, verdictAtMs: atMs } },
        };
      });
    },
    verdictFor(outputId) {
      return get().outputs[outputId]?.verdict ?? null;
    },
    isAvailable(outputId) {
      const v = get().outputs[outputId]?.verdict;
      return v?.kind === 'available';
    },

    setMonitoring(config) {
      const errors = validateMonitoringDraft(config);
      if (errors.length === 0) set({ monitoring: config });
      return errors;
    },
    editDraft(draft) {
      set({
        draft,
        draftErrors: draft ? validateMonitoringDraft(draft) : [],
      });
    },
    updateDraft(patch) {
      const s = get();
      if (!s.draft) return;
      const next = { ...s.draft, ...patch };
      set({ draft: next, draftErrors: validateMonitoringDraft(next) });
    },
  }));
}
