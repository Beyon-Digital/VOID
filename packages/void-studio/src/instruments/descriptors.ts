// Builtin instrument descriptors (W10, T40).
//
// The engine exposes builtin instruments through the plugin surface:
// InsertPluginOp{track_id, slot, plugin_instance_id, format, plugin_uid}
// installs one and SetPluginParamOp{plugin_instance_id, param_id, value}
// binds a control. The prompt's "InsertInstrument/SetParam" names are
// stale — these are the real op variants in protocol/void_control.fbs.
//
// A descriptor is *capability metadata*, not state: the param list is the
// engine-advertised contract the UI binds against. If the engine adds a
// param, add it here — never invent params the engine does not expose
// (T40: only exposed controls may be adjustable).

import type { PersistentOp } from 'void-client';

/** Format string the engine reports for a builtin instrument. */
export const BUILTIN_PLUGIN_FORMAT = 'VOID-BUILTIN';

/** One engine-exposed parameter. */
export interface ParamDescriptor {
  /** Engine param_id as sent in SetPluginParamOp. */
  id: string;
  /** Display label. */
  label: string;
  min: number;
  max: number;
  defaultValue: number;
  /** Optional unit suffix for display ("dB", "ms", "ct"). */
  unit?: string;
  /**
   * Optional quantization step (0/undefined = continuous). UI controls
   * snap to this; engine still receives a plain finite float.
   */
  step?: number;
}

/** A builtin instrument the engine can instantiate. */
export interface InstrumentDescriptor {
  /** plugin_uid sent in InsertPluginOp. */
  pluginUid: string;
  name: string;
  params: ParamDescriptor[];
}

/**
 * FourOsc — starter poly-synth. Four oscillators with level/tune/wave,
 * shared filter and amp envelope. Param ids mirror the engine's
 * advertised ids; ranges are the engine's clamp bounds.
 */
export const FOUR_OSC: InstrumentDescriptor = {
  pluginUid: 'void.builtin.four_osc',
  name: 'FourOsc',
  params: [
    { id: 'osc1_wave', label: 'Osc 1 Wave', min: 0, max: 3, defaultValue: 0, step: 1 },
    { id: 'osc1_tune', label: 'Osc 1 Tune', min: -1200, max: 1200, defaultValue: 0, unit: 'ct' },
    { id: 'osc1_level', label: 'Osc 1 Level', min: 0, max: 1, defaultValue: 0.8 },
    { id: 'osc2_wave', label: 'Osc 2 Wave', min: 0, max: 3, defaultValue: 0, step: 1 },
    { id: 'osc2_tune', label: 'Osc 2 Tune', min: -1200, max: 1200, defaultValue: 0, unit: 'ct' },
    { id: 'osc2_level', label: 'Osc 2 Level', min: 0, max: 1, defaultValue: 0.8 },
    { id: 'osc3_wave', label: 'Osc 3 Wave', min: 0, max: 3, defaultValue: 0, step: 1 },
    { id: 'osc3_tune', label: 'Osc 3 Tune', min: -1200, max: 1200, defaultValue: 0, unit: 'ct' },
    { id: 'osc3_level', label: 'Osc 3 Level', min: 0, max: 1, defaultValue: 0 },
    { id: 'osc4_wave', label: 'Osc 4 Wave', min: 0, max: 3, defaultValue: 0, step: 1 },
    { id: 'osc4_tune', label: 'Osc 4 Tune', min: -1200, max: 1200, defaultValue: 0, unit: 'ct' },
    { id: 'osc4_level', label: 'Osc 4 Level', min: 0, max: 1, defaultValue: 0 },
    { id: 'filter_cutoff', label: 'Cutoff', min: 20, max: 20000, defaultValue: 8000, unit: 'Hz' },
    { id: 'filter_resonance', label: 'Resonance', min: 0, max: 1, defaultValue: 0.2 },
    { id: 'env_attack', label: 'Attack', min: 0, max: 5000, defaultValue: 10, unit: 'ms' },
    { id: 'env_decay', label: 'Decay', min: 0, max: 5000, defaultValue: 200, unit: 'ms' },
    { id: 'env_sustain', label: 'Sustain', min: 0, max: 1, defaultValue: 0.7 },
    { id: 'env_release', label: 'Release', min: 0, max: 10000, defaultValue: 300, unit: 'ms' },
    { id: 'master_level', label: 'Level', min: 0, max: 1, defaultValue: 0.8 },
  ],
};

/** Sampler — single-sample instrument for user sample content (T40). */
export const SAMPLER: InstrumentDescriptor = {
  pluginUid: 'void.builtin.sampler',
  name: 'Sampler',
  params: [
    // The asset itself is attached through the project asset store +
    // engine slot, not a param — params cover playback shaping only.
    { id: 'root_note', label: 'Root Note', min: 0, max: 127, defaultValue: 60, step: 1 },
    { id: 'gain', label: 'Gain', min: -24, max: 24, defaultValue: 0, unit: 'dB' },
    { id: 'env_attack', label: 'Attack', min: 0, max: 2000, defaultValue: 0, unit: 'ms' },
    { id: 'env_release', label: 'Release', min: 0, max: 10000, defaultValue: 100, unit: 'ms' },
    { id: 'loop', label: 'Loop', min: 0, max: 1, defaultValue: 0, step: 1 },
    { id: 'master_level', label: 'Level', min: 0, max: 1, defaultValue: 0.8 },
  ],
};

/** The builtin instrument catalog. */
export const BUILTIN_INSTRUMENTS: readonly InstrumentDescriptor[] = [
  FOUR_OSC,
  SAMPLER,
];

export function descriptorByUid(
  pluginUid: string,
  catalog: readonly InstrumentDescriptor[] = BUILTIN_INSTRUMENTS,
): InstrumentDescriptor | undefined {
  return catalog.find((d) => d.pluginUid === pluginUid);
}

export function paramOf(
  descriptor: InstrumentDescriptor,
  paramId: string,
): ParamDescriptor | undefined {
  return descriptor.params.find((p) => p.id === paramId);
}

/** Clamp + quantize a value to a param's declared range. */
export function clampParam(desc: ParamDescriptor, value: number): number {
  if (!Number.isFinite(value)) {
    throw new Error(`param ${desc.id}: value must be finite, got ${value}`);
  }
  const clamped = Math.min(desc.max, Math.max(desc.min, value));
  if (desc.step && desc.step > 0) {
    // Round to the nearest step inside the range.
    const snapped = Math.round(clamped / desc.step) * desc.step;
    return Math.min(desc.max, Math.max(desc.min, snapped));
  }
  return clamped;
}

/**
 * InsertPluginOp for one builtin instrument instance. `instanceId` is a
 * caller-allocated UUID — reused as the plugin_instance_id everywhere
 * else (params, editor open/close, removal).
 */
export function insertInstrumentOp(
  trackId: string,
  descriptor: InstrumentDescriptor,
  instanceId: string,
  slot?: number,
): PersistentOp {
  return {
    InsertPluginOp: {
      track_id: trackId,
      slot,
      plugin_instance_id: instanceId,
      format: BUILTIN_PLUGIN_FORMAT,
      plugin_uid: descriptor.pluginUid,
    },
  };
}

/** SetPluginParamOp with descriptor-driven clamping/quantization. */
export function setInstrumentParamOp(
  descriptor: InstrumentDescriptor,
  instanceId: string,
  paramId: string,
  value: number,
): PersistentOp {
  const p = paramOf(descriptor, paramId);
  if (!p) {
    throw new Error(
      `param ${paramId} not exposed by ${descriptor.name} — refusing to send an unknown param id`,
    );
  }
  return {
    SetPluginParamOp: {
      plugin_instance_id: instanceId,
      param_id: paramId,
      value: clampParam(p, value),
    },
  };
}

/** RemovePluginOp for one instance. */
export function removeInstrumentOp(instanceId: string): PersistentOp {
  return { RemovePluginOp: { plugin_instance_id: instanceId } };
}

/** Default-value ops for every descriptor param (preset "init"). */
export function defaultParamOps(
  descriptor: InstrumentDescriptor,
  instanceId: string,
): PersistentOp[] {
  return descriptor.params.map((p) => ({
    SetPluginParamOp: {
      plugin_instance_id: instanceId,
      param_id: p.id,
      value: p.defaultValue,
    },
  }));
}
