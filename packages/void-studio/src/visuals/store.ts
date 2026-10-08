// Visuals view-state store (W22).
//
// View state ONLY: selected layer ids, channel focus, last frame meta
// (no pixels), drop counters, transition status. The scene document
// lives in void-visual; this store just projects it for the UI.

import { createStore, StoreApi } from 'zustand/vanilla';
import type {
  VisualAnchorView,
  VisualChannel,
  VisualDropCounters,
  VisualFrameMeta,
  VisualLayerView,
  VisualOutputView,
  VisualTransitionView,
} from './model';

export interface VisualsViewState {
  /** Layer projections keyed by layer id (projection, not document). */
  layers: Record<string, VisualLayerView>;
  /** Per-channel stack order. */
  order: Record<VisualChannel, string[]>;
  anchors: Record<string, VisualAnchorView>;
  transitions: Record<VisualChannel, VisualTransitionView | null>;
  outputs: Record<VisualChannel, VisualOutputView | null>;
  /** UI selection + channel focus (pure view state). */
  focusedChannel: VisualChannel;
  selectedLayerIds: string[];
  /** Latest produced frame meta per channel (identity/sha, no pixels). */
  lastFrame: Record<VisualChannel, VisualFrameMeta | null>;
  dropCounters: VisualDropCounters;
  /** Engine-side revision observed in receipts/snapshots. */
  engineRevision: number;
}

export interface VisualsActions {
  /** Replace projections from a snapshot (checkpoint/read-view). */
  applySnapshot(s: {
    layers: VisualLayerView[];
    anchors: VisualAnchorView[];
    transitions: (VisualTransitionView | null)[];
    outputs: (VisualOutputView | null)[];
    order?: Partial<Record<VisualChannel, string[]>>;
    revision?: number;
  }): void;
  noteFrame(frame: VisualFrameMeta): void;
  noteCounters(c: Partial<VisualDropCounters>): void;
  selectLayers(ids: string[]): void;
  focusChannel(ch: VisualChannel): void;
  clear(): void;
}

export type VisualsStore = StoreApi<VisualsViewState & { actions: VisualsActions }>;

const emptyCounters: VisualDropCounters = {
  droppedClocks: 0,
  droppedFrames: 0,
  skippedRenders: 0,
  producedFrames: 0,
};

export function createVisualsStore(): VisualsStore {
  return createStore<VisualsViewState & { actions: VisualsActions }>()((set) => ({
    layers: {},
    order: { preview: [], program: [] },
    anchors: {},
    transitions: { preview: null, program: null },
    outputs: { preview: null, program: null },
    focusedChannel: 'program',
    selectedLayerIds: [],
    lastFrame: { preview: null, program: null },
    dropCounters: { ...emptyCounters },
    engineRevision: 0,
    actions: {
      applySnapshot(s) {
        const layers: Record<string, VisualLayerView> = {};
        for (const l of s.layers) layers[l.layerId] = l;
        const anchors: Record<string, VisualAnchorView> = {};
        for (const a of s.anchors) anchors[a.anchorId] = a;
        const order = { preview: [] as string[], program: [] as string[] };
        for (const l of s.layers) {
          order[l.channel][l.index] = l.layerId;
        }
        // Compact sparse index arrays.
        order.preview = order.preview.filter(Boolean);
        order.program = order.program.filter(Boolean);
        if (s.order?.preview) order.preview = s.order.preview;
        if (s.order?.program) order.program = s.order.program;
        const transitions = { preview: null, program: null } as VisualsViewState['transitions'];
        for (const t of s.transitions) {
          if (t) transitions[t.channel] = t;
        }
        const outputs = { preview: null, program: null } as VisualsViewState['outputs'];
        for (const o of s.outputs) {
          if (o) outputs[o.channel] = o;
        }
        set({
          layers,
          anchors,
          order,
          transitions,
          outputs,
          ...(s.revision !== undefined ? { engineRevision: s.revision } : {}),
        });
      },
      noteFrame(frame) {
        set((st) => ({ lastFrame: { ...st.lastFrame, [frame.channel]: frame } }));
      },
      noteCounters(c) {
        set((st) => ({ dropCounters: { ...st.dropCounters, ...c } }));
      },
      selectLayers(ids) {
        set({ selectedLayerIds: [...ids] });
      },
      focusChannel(ch) {
        set({ focusedChannel: ch });
      },
      clear() {
        set({
          layers: {},
          order: { preview: [], program: [] },
          anchors: {},
          transitions: { preview: null, program: null },
          outputs: { preview: null, program: null },
          selectedLayerIds: [],
          lastFrame: { preview: null, program: null },
          dropCounters: { ...emptyCounters },
          engineRevision: 0,
        });
      },
    },
  }));
}
