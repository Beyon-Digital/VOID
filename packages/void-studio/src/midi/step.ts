// Step editor (W18; MIDI-04): step-entry model.
//
// A step cursor sits at a tick position with a current step size,
// duration fraction and velocity. `stepNote` inserts a note at the
// cursor and advances by the step; `stepRest` advances without
// inserting; `stepBack` retreats one step. Chord mode (`chord: true`)
// inserts at the cursor without advancing, so consecutive entries pile
// on the same start — `endChord` then advances once.
//
// The model is pure and tick-exact; velocity and duration are the
// entry parameters a hardware-style step editor would carry.

import { i64str, parseI64 } from 'void-client';
import type { MidiNote } from './types';
import { insertNote, MidiModelError } from './events';
import { VELOCITY_MAX, VELOCITY_MIN } from './types';

export interface StepCursor {
  /** Current entry position (i64 decimal string). */
  atTicks: string;
  /** Step size in ticks (the grid the cursor advances by). */
  stepTicks: string;
  /** Note length as a fraction of the step, 0 < f ≤ 1 (legato = 1). */
  durationFraction: number;
  velocity: number; // 1..127
  /** When true, entries stack at the cursor without advancing. */
  chord: boolean;
  /** Next sequential note id suffix. */
  nextId: number;
}

export function makeCursor(
  atTicks: string,
  stepTicks: string,
  opts: { durationFraction?: number; velocity?: number } = {},
): StepCursor {
  const step = parseI64(stepTicks);
  if (step <= 0n) {
    throw new MidiModelError(`step must be > 0, got ${stepTicks}`);
  }
  const f = opts.durationFraction ?? 0.9;
  if (!(f > 0 && f <= 1)) {
    throw new MidiModelError(`durationFraction must be in (0,1], got ${f}`);
  }
  const v = opts.velocity ?? 100;
  if (!Number.isInteger(v) || v < VELOCITY_MIN || v > VELOCITY_MAX) {
    throw new MidiModelError(`velocity out of range: ${v}`);
  }
  return {
    atTicks: i64str(atTicks),
    stepTicks: i64str(step),
    durationFraction: f,
    velocity: v,
    chord: false,
    nextId: 1,
  };
}

function noteLength(c: StepCursor): bigint {
  const step = parseI64(c.stepTicks);
  const len = (step * BigInt(Math.round(c.durationFraction * 1_000_000))) / 1_000_000n;
  return len > 0n ? len : 1n;
}

/** Insert a note at the cursor; advances unless in chord mode. */
export function stepNote(
  notes: MidiNote[],
  c: StepCursor,
  pitch: number,
  opts: { velocity?: number } = {},
): { notes: MidiNote[]; cursor: StepCursor; note: MidiNote } {
  const note: MidiNote = {
    id: `step-${c.nextId}`,
    pitch,
    velocity: opts.velocity ?? c.velocity,
    startTicks: c.atTicks,
    lengthTicks: i64str(noteLength(c)),
  };
  const nextNotes = insertNote(notes, note);
  const cursor: StepCursor = { ...c, nextId: c.nextId + 1 };
  if (!c.chord) {
    cursor.atTicks = i64str(parseI64(c.atTicks) + parseI64(c.stepTicks));
  }
  return { notes: nextNotes, cursor, note };
}

/** Advance the cursor without inserting. */
export function stepRest(c: StepCursor, steps = 1): StepCursor {
  return {
    ...c,
    atTicks: i64str(parseI64(c.atTicks) + parseI64(c.stepTicks) * BigInt(steps)),
  };
}

/** Retreat the cursor one step (floor at 0). */
export function stepBack(c: StepCursor): StepCursor {
  const at = parseI64(c.atTicks) - parseI64(c.stepTicks);
  return { ...c, atTicks: i64str(at < 0n ? 0n : at) };
}

export function setChord(c: StepCursor, chord: boolean): StepCursor {
  return { ...c, chord };
}

/** After a chord run, advance once for the whole chord. */
export function endChord(c: StepCursor): StepCursor {
  if (!c.chord) return c;
  return {
    ...c,
    chord: false,
    atTicks: i64str(parseI64(c.atTicks) + parseI64(c.stepTicks)),
  };
}
