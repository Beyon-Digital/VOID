// Automation lane types (W18; MIX-05/MIX-06, T70).
//
// Wire reality (protocol major.1): there is NO automation-point op in
// void_control.fbs — during a write pass the mode machine emits real
// parameter ops (SetTrackGainOp / SetTrackPanOp / SetTrackMuteOp /
// SetPluginParamOp) that reach the engine as immediate parameter
/// values, while the recorded *curve* is a view-state model pending the
/// engine surface (docs/mix/NEEDS.md). Never presented as persisted.

import type { I64 } from 'void-client';

/**
 * Write modes — the classic automation state machine (MIX-05):
 * - `off`   : lane exists but neither reads nor writes.
 * - `read`  : lane plays back; gestures are ignored (never write).
 * - `write` : a pass continuously overwrites the covered range.
 * - `touch` : writes only while the control is touched; releases back
 *             to the underlying curve.
 * - `latch` : writes while touched; on release keeps writing the last
 *             touched value until the pass ends.
 * - `trim`  : latch-style pass that writes *relative offsets* into the
 *             lane's trim curve — the base curve is never baked over
 *             (MIX-06).
 */
export type AutomationMode = 'off' | 'read' | 'touch' | 'latch' | 'write' | 'trim';

/** Interpolation between recorded points (documented, both tested). */
export type Interpolation = 'linear' | 'stepped';

/** Addressable parameter — a track mixer field or a plugin param. */
export interface ParamRef {
  kind: 'trackGain' | 'trackPan' | 'trackMute' | 'pluginParam';
  trackId: string;
  /** pluginParam only. */
  pluginInstanceId?: string;
  paramId?: string;
}

/** Stable identity for maps/store keys. */
export function paramKey(p: ParamRef): string {
  return p.kind === 'pluginParam'
    ? `pluginParam:${p.pluginInstanceId ?? ''}:${p.paramId ?? ''}`
    : `${p.kind}:${p.trackId}`;
}

/** One recorded breakpoint. `ticks` is a decimal-string int64. */
export interface AutomationPoint {
  ticks: I64;
  value: number;
}

/**
 * A lane = base curve + trim curve, each sorted ascending by ticks.
 * The effective value is `base(t) + trim(t)` — trim is a relative
 * offset layer that is never baked into base (MIX-06).
 */
export interface AutomationLane {
  id: string;
  param: ParamRef;
  interpolation: Interpolation;
  mode: AutomationMode;
  base: AutomationPoint[];
  trim: AutomationPoint[];
}

export function makeLane(
  id: string,
  param: ParamRef,
  init?: Partial<Pick<AutomationLane, 'interpolation' | 'mode' | 'base' | 'trim'>>,
): AutomationLane {
  return {
    id,
    param,
    interpolation: init?.interpolation ?? 'linear',
    mode: init?.mode ?? 'read',
    base: init?.base ?? [],
    trim: init?.trim ?? [],
  };
}
