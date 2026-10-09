// Read/touch/latch/write/trim write-pass state machine (W18; MIX-05,
// T70).
//
// A "pass" is one transport write pass over a lane. The pass records
// points sparsely — a breakpoint is written on touch, on value change,
// and at the pass boundary that needs it (latch's hold-to-end). When
// the pass ends, `commitPass` splices the written range into the lane:
//
// - write : replaces the whole pass range [start, end]
// - touch : replaces only [firstWritten, lastWritten]
// - latch : replaces [firstWritten, passEnd], holding the release value
// - trim  : writes relative offsets into lane.trim in the same span —
//           the base curve is never touched (MIX-06)
//
// `read`/`off` lanes reject `beginPass` outright — the intended gesture
// boundary is checked before any point exists.

import { i64str, parseI64 } from 'void-client';
import type { AutomationLane, AutomationMode, AutomationPoint } from './types';
import { insertPoint, spliceRange } from './lane';

export class WriteNotArmed extends Error {
  constructor(mode: AutomationMode) {
    super(`automation mode '${mode}' does not write`);
    this.name = 'WriteNotArmed';
  }
}

export type PassMode = 'write' | 'touch' | 'latch' | 'trim';

/** One transport write pass over a lane. */
export interface WritePass {
  laneId: string;
  mode: PassMode;
  /** Pass bounds: start of the pass, end when punched out/stopped. */
  startTicks: string;
  endTicks?: string;
  /** Whether a control is currently held. */
  touched: boolean;
  /** Whether the pass is currently emitting values. */
  writing: boolean;
  /** Last touched value — what latch/trim hold after release. */
  holdValue?: number;
  /** Points recorded during the pass (thinned: only changes). */
  points: AutomationPoint[];
  /** Ticks of the first/last recorded point. */
  firstWritten?: string;
  lastWritten?: string;
}

function checkMode(mode: AutomationMode): asserts mode is PassMode {
  if (mode === 'read' || mode === 'off') {
    throw new WriteNotArmed(mode);
  }
}

/**
 * Start a write pass. `write` mode begins emitting immediately at
 * `initialValue` (a write pass overwrites as the transport rolls);
 * touch/latch/trim wait for the first touch.
 */
export function beginPass(
  lane: AutomationLane,
  mode: AutomationMode,
  atTicks: string,
  initialValue?: number,
): WritePass {
  checkMode(mode);
  const pass: WritePass = {
    laneId: lane.id,
    mode,
    startTicks: i64str(atTicks),
    touched: false,
    writing: mode === 'write',
    points: [],
  };
  if (mode === 'write' && initialValue !== undefined) {
    record(pass, atTicks, initialValue);
  }
  return pass;
}

function record(pass: WritePass, ticks: string, value: number): void {
  if (!Number.isFinite(value)) {
    throw new Error(`automation value must be finite, got ${value}`);
  }
  const t = i64str(ticks);
  const last = pass.points[pass.points.length - 1];
  // Thin the curve: only record a breakpoint on change or position.
  if (last && last.value === value && last.ticks === t) return;
  if (last && last.value === value) {
    // same value, later position — update the open segment end only for
    // latch/trim (the held value extends implicitly), so just move the
    // marker forward without adding a duplicate-valued point.
    pass.lastWritten = t;
    return;
  }
  pass.points.push({ ticks: t, value });
  if (pass.firstWritten === undefined) pass.firstWritten = t;
  pass.lastWritten = t;
}

/** Begin/continue holding a control at `value`. */
export function touch(pass: WritePass, atTicks: string, value: number): WritePass {
  if (pass.endTicks !== undefined) {
    throw new Error('pass already ended');
  }
  pass.touched = true;
  pass.writing = true;
  pass.holdValue = value;
  record(pass, atTicks, value);
  return pass;
}

/** Release the control. */
export function release(pass: WritePass, atTicks: string): WritePass {
  if (pass.endTicks !== undefined) {
    throw new Error('pass already ended');
  }
  pass.touched = false;
  if (pass.mode === 'touch') {
    // Touch releases back to the underlying curve: writing stops at the
    // release point.
    pass.writing = false;
    pass.lastWritten = i64str(atTicks);
  } else if (pass.mode === 'latch' || pass.mode === 'trim') {
    // Keep writing the held value until the pass ends (recorded at end).
    pass.writing = true;
  }
  return pass;
}

/** Move the current write value (gesture drag) while touched. */
export function moveValue(pass: WritePass, atTicks: string, value: number): WritePass {
  if (pass.endTicks !== undefined) {
    throw new Error('pass already ended');
  }
  if (!pass.writing) return pass;
  if (pass.mode !== 'write' && !pass.touched) return pass;
  pass.holdValue = value;
  record(pass, atTicks, value);
  return pass;
}

/** Write-mode transport advance: the current value keeps overwriting. */
export function advance(pass: WritePass, atTicks: string, value: number): WritePass {
  if (pass.endTicks !== undefined) {
    throw new Error('pass already ended');
  }
  if (!pass.writing) return pass;
  if (pass.mode === 'write' || pass.touched) {
    pass.holdValue = value;
    record(pass, atTicks, value);
  }
  return pass;
}

/**
 * Close the pass at `atTicks`. For latch/trim the held value is carried
 * to the pass end with an explicit endpoint breakpoint.
 */
export function endPass(pass: WritePass, atTicks: string): WritePass {
  const end = i64str(atTicks);
  if (parseI64(end) < parseI64(pass.startTicks)) {
    throw new Error(`pass end ${end} before start ${pass.startTicks}`);
  }
  if ((pass.mode === 'latch' || pass.mode === 'trim') && pass.writing && pass.holdValue !== undefined) {
    if (!pass.points.length || pass.points[pass.points.length - 1].ticks !== end) {
      pass.points.push({ ticks: end, value: pass.holdValue });
      if (pass.firstWritten === undefined) pass.firstWritten = end;
      pass.lastWritten = end;
    }
  }
  pass.endTicks = end;
  return pass;
}

/**
 * Splice a closed pass into its lane (immutable — returns a new lane).
 * Mode decides the replaced range:
 * - write : [pass.start, pass.end]
 * - touch : [firstWritten, lastWritten]
 * - latch : [firstWritten, pass.end]
 * - trim  : same span as latch, written into `trim` (base untouched)
 */
export function commitPass(lane: AutomationLane, pass: WritePass): AutomationLane {
  if (pass.laneId !== lane.id) {
    throw new Error(`pass ${pass.laneId} does not belong to lane ${lane.id}`);
  }
  if (pass.endTicks === undefined) {
    throw new Error('commitPass: pass is still open — call endPass first');
  }
  if (pass.points.length === 0) return lane; // read-only pass committed nothing

  const end = pass.endTicks;
  const start =
    pass.mode === 'write'
      ? pass.startTicks
      : pass.firstWritten ?? pass.startTicks;

  if (pass.mode === 'trim') {
    return { ...lane, trim: spliceRange(lane.trim, start, end === start ? i64str(parseI64(start) + 1n) : end, pass.points) };
  }
  const spliceEnd =
    pass.mode === 'latch' ? end : pass.mode === 'touch' ? pass.lastWritten ?? end : end;
  const safeEnd =
    parseI64(spliceEnd) <= parseI64(start)
      ? i64str(parseI64(start) + 1n)
      : spliceEnd;
  return { ...lane, base: spliceRange(lane.base, start, safeEnd, pass.points) };
}
