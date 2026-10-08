// MIDI-learn / macro types (MIDI-02, MIX-07, T58).
//
// A Mapping binds an input CONTROL (MIDI CC, note, touch axis, or
// keyboard key) to a TARGET (a parameter or a macro). It is a spec —
// this package never touches MIDI plumbing or the engine; dispatch is
// pure value math over the registry, so the same semantics apply to
// touch, trackpad, and MIDI sources alike.
//
// Arming contract (T58): a control only moves a parameter while that
// parameter's mapping target is ARMED. Arming is explicit and single —
// navigation gestures on other UI never reach music parameters.
// Disconnect/focus-loss calls releaseAll(), which disarms and releases
// every held control (documented all-notes-off semantics); PANIC does
// the same with priority over any queued event.

export type ControlKind = 'cc' | 'note' | 'axis' | 'key';

/** One addressable input control. */
export interface ControlSpec {
  kind: ControlKind;
  /** MIDI channel 0..15 (cc/note only; ignored otherwise). */
  channel?: number;
  /** CC number 0..127, or note number 0..127, or axis id, or key name. */
  id: string;
}

export type TargetKind = 'param' | 'macro';

/** What a control moves: a single parameter or a macro fan-out. */
export interface TargetSpec {
  kind: TargetKind;
  /** Parameter id (param) or macro id (macro). */
  id: string;
}

export type ValueCurve = 'linear' | 'exponential' | 'logarithmic' | 'toggle';

/** Control→parameter mapping with a real value transform. */
export interface Mapping {
  mappingId: string;
  control: ControlSpec;
  target: TargetSpec;
  /** Input is normalized 0..1; curve shapes it before range mapping. */
  curve: ValueCurve;
  /** Output range after curve. min<=max; both finite. */
  min: number;
  max: number;
  /** Invert input direction (1-x) before the curve. */
  invert: boolean;
  /** Clamp policy for out-of-range INPUT: 'clamp' bounds it, 'reject'
   *  drops the event (MIX-07). */
  outOfRange: 'clamp' | 'reject';
}

/** One member of a macro — value passes through its own curve+range. */
export interface MacroMember {
  paramId: string;
  curve: ValueCurve;
  min: number;
  max: number;
}

export interface Macro {
  macroId: string;
  name: string;
  members: MacroMember[];
}

/** A raw control event as delivered by the input layer. */
export interface ControlEvent {
  control: ControlSpec;
  /** Normalized 0..1, or raw when kind==='note' (velocity/127). */
  value: number;
  /** Whether a momentary control went down (true) or up (false).
   *  Continuous controls send value-only events with phase 'value'. */
  phase: 'down' | 'value' | 'up';
  atMs: number;
}

/** Result of dispatching a control event through a mapping. */
export interface ParamChange {
  paramId: string;
  value: number;
  mappingId: string;
  atMs: number;
}

export function controlKey(c: ControlSpec): string {
  return `${c.kind}:${c.channel ?? ''}:${c.id}`;
}

export function sameControl(a: ControlSpec, b: ControlSpec): boolean {
  return controlKey(a) === controlKey(b);
}
