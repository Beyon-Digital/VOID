// Instrument presets + rack view state (W10, T40).
//
// Presets are descriptor-shaped param maps — they carry *declared
// capability defaults*, not engine state. The rack holds only which
// instance lives in which slot; live param values are read-back data
// (PLUGIN_LIST view), never a second copy of engine truth.
//
// License note (T40): bundled presets ship with the app and their rights
// are recorded here as `license` text — kept separate from any library
// licence the project assets carry.

import { createStore, StoreApi } from 'zustand/vanilla';
import type { InstrumentDescriptor, ParamDescriptor } from './descriptors';
import { BUILTIN_INSTRUMENTS, descriptorByUid, paramOf, clampParam } from './descriptors';

export interface InstrumentPreset {
  id: string;
  name: string;
  /** descriptor.pluginUid this preset applies to. */
  instrumentUid: string;
  category: string;
  /** param_id -> value; ids are validated against the descriptor on apply. */
  params: Record<string, number>;
  /** Rights statement for bundled content — recorded, not implied. */
  license: string;
}

/** Bundled presets cleared to ship (original/cleared — W10 requirement). */
export const BUILTIN_PRESETS: readonly InstrumentPreset[] = [
  {
    id: 'preset.four_osc.init',
    name: 'FourOsc Init',
    instrumentUid: 'void.builtin.four_osc',
    category: 'init',
    params: {},
    license: 'VOID factory content — original work, cleared for distribution',
  },
  {
    id: 'preset.four_osc.warm_pad',
    name: 'Warm Pad',
    instrumentUid: 'void.builtin.four_osc',
    category: 'pads',
    params: {
      osc1_wave: 0,
      osc1_level: 0.7,
      osc2_wave: 0,
      osc2_tune: 6,
      osc2_level: 0.7,
      osc3_wave: 1,
      osc3_tune: -7,
      osc3_level: 0.5,
      filter_cutoff: 2400,
      filter_resonance: 0.15,
      env_attack: 420,
      env_decay: 800,
      env_sustain: 0.75,
      env_release: 1200,
      master_level: 0.75,
    },
    license: 'VOID factory content — original work, cleared for distribution',
  },
  {
    id: 'preset.four_osc.bright_lead',
    name: 'Bright Lead',
    instrumentUid: 'void.builtin.four_osc',
    category: 'leads',
    params: {
      osc1_wave: 2,
      osc1_level: 0.85,
      osc2_wave: 2,
      osc2_tune: 1200,
      osc2_level: 0.4,
      filter_cutoff: 9000,
      filter_resonance: 0.25,
      env_attack: 5,
      env_decay: 300,
      env_sustain: 0.65,
      env_release: 220,
      master_level: 0.8,
    },
    license: 'VOID factory content — original work, cleared for distribution',
  },
  {
    id: 'preset.sampler.init',
    name: 'Sampler Init',
    instrumentUid: 'void.builtin.sampler',
    category: 'init',
    params: {},
    license: 'VOID factory content — original work, cleared for distribution',
  },
];

/** Presets visible for an instrument, optionally filtered. */
export function presetsFor(
  instrumentUid: string,
  presets: readonly InstrumentPreset[] = BUILTIN_PRESETS,
): InstrumentPreset[] {
  return presets.filter((p) => p.instrumentUid === instrumentUid);
}

/**
 * Resolve a preset into concrete param ops input: every param NOT listed
 * falls back to the descriptor default — a preset is always complete
 * state, not a diff over whatever is currently loaded (T40: reopening a
 * preset must reproduce the same sound). Unknown param ids throw — a
 * preset that targets a non-existent param is malformed content, not a
 * silent no-op.
 */
export function resolvePreset(
  descriptor: InstrumentDescriptor,
  preset: InstrumentPreset,
): { param: ParamDescriptor; value: number }[] {
  if (preset.instrumentUid !== descriptor.pluginUid) {
    throw new Error(
      `preset ${preset.id} targets ${preset.instrumentUid}, not ${descriptor.pluginUid}`,
    );
  }
  for (const id of Object.keys(preset.params)) {
    if (!paramOf(descriptor, id)) {
      throw new Error(`preset ${preset.id} sets unknown param ${id}`);
    }
  }
  return descriptor.params.map((p) => ({
    param: p,
    value: clampParam(p, preset.params[p.id] ?? p.defaultValue),
  }));
}

// ---------------------------------------------------------------------------
// Rack view state
// ---------------------------------------------------------------------------

/** One slot in a track's instrument rack — identity + declared state. */
export interface InstrumentSlot {
  slotIndex: number;
  /** plugin_instance_id from the InsertPluginOp. */
  instanceId: string;
  pluginUid: string;
  /** Values the UI last SENT (or read back) — display only. */
  lastKnown: Record<string, number>;
}

export interface InstrumentBrowseState {
  /** Track the rack belongs to; '' until a track is chosen. */
  trackId: string;
  slots: InstrumentSlot[];
  /** Selected browse target (instrument uid to insert). */
  browseUid: string | null;
  /** Selected preset id for preview before load. */
  previewPresetId: string | null;
  filterText: string;
  category: string | null;
}

export interface InstrumentBrowseActions {
  setTrack(trackId: string): void;
  setBrowseUid(uid: string | null): void;
  setPreviewPreset(id: string | null): void;
  setFilter(text: string): void;
  setCategory(cat: string | null): void;
  upsertSlot(slot: InstrumentSlot): void;
  removeSlot(instanceId: string): void;
  noteParamSent(instanceId: string, paramId: string, value: number): void;
  reset(): void;
}

export type InstrumentBrowseStore = StoreApi<
  InstrumentBrowseState & { actions: InstrumentBrowseActions }
>;

export function initialInstrumentBrowse(): InstrumentBrowseState {
  return {
    trackId: '',
    slots: [],
    browseUid: null,
    previewPresetId: null,
    filterText: '',
    category: null,
  };
}

export function createInstrumentBrowseStore(
  init?: Partial<InstrumentBrowseState>,
): InstrumentBrowseStore {
  return createStore<
    InstrumentBrowseState & { actions: InstrumentBrowseActions }
  >()((set, get) => ({
    ...initialInstrumentBrowse(),
    ...init,
    actions: {
      setTrack: (trackId) =>
        set((s) => (s.trackId === trackId ? s : { trackId, slots: [] })),
      setBrowseUid: (browseUid) => set({ browseUid, previewPresetId: null }),
      setPreviewPreset: (previewPresetId) => set({ previewPresetId }),
      setFilter: (filterText) => set({ filterText }),
      setCategory: (category) => set({ category }),
      upsertSlot: (slot) =>
        set((s) => ({
          slots: s.slots.some((x) => x.instanceId === slot.instanceId)
            ? s.slots.map((x) => (x.instanceId === slot.instanceId ? slot : x))
            : [...s.slots, slot].sort((a, b) => a.slotIndex - b.slotIndex),
        })),
      removeSlot: (instanceId) =>
        set((s) => ({
          slots: s.slots.filter((x) => x.instanceId !== instanceId),
        })),
      noteParamSent: (instanceId, paramId, value) =>
        set((s) => ({
          slots: s.slots.map((x) =>
            x.instanceId === instanceId
              ? { ...x, lastKnown: { ...x.lastKnown, [paramId]: value } }
              : x,
          ),
        })),
      reset: () => set(initialInstrumentBrowse()),
    },
  }));
}

/** Filtered preset list for the browse panel. */
export function browseResults(state: InstrumentBrowseState): {
  instruments: InstrumentDescriptor[];
  presets: InstrumentPreset[];
} {
  const q = state.filterText.trim().toLowerCase();
  const instruments = BUILTIN_INSTRUMENTS.filter(
    (d) =>
      (!q || d.name.toLowerCase().includes(q) || d.pluginUid.includes(q)) &&
      (state.browseUid === null || d.pluginUid === state.browseUid || !q),
  );
  const presets = BUILTIN_PRESETS.filter(
    (p) =>
      (!state.category || p.category === state.category) &&
      (!q || p.name.toLowerCase().includes(q)) &&
      (!state.browseUid || p.instrumentUid === state.browseUid),
  );
  return { instruments, presets };
}

/** Descriptor lookup helper for the rack. */
export function slotDescriptor(
  slot: InstrumentSlot,
): InstrumentDescriptor | undefined {
  return descriptorByUid(slot.pluginUid);
}
