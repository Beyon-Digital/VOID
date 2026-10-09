// Event editor model (W18; MIDI-04, T71): insert/move/resize/quantize
// on string-int64 ticks. Model functions are pure — each returns a new
// array; wire ops reuse the piano-roll builders (insertNoteOp,
// setNoteOp, removeNoteOp) since the protocol's note ops cover note
// geometry only. Channel-level events and per-note expression have no
// wire op in major.1 — that is a recorded NEEDS gap, not a fake path.

import { i64str, parseI64 } from 'void-client';
import type { MidiClipModel, MidiNote } from './types';
import {
  BEND_MAX,
  BEND_MIN,
  CC_MAX,
  CC_MIN,
  CHANNEL_MAX,
  CHANNEL_MIN,
  PITCH_MAX,
  PITCH_MIN,
  VELOCITY_MAX,
  VELOCITY_MIN,
} from './types';
import type { MidiEvent } from './types';

export class MidiModelError extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = 'MidiModelError';
  }
}

export function checkNote(n: MidiNote): void {
  if (!n.id) throw new MidiModelError('note needs an id');
  if (!Number.isInteger(n.pitch) || n.pitch < PITCH_MIN || n.pitch > PITCH_MAX) {
    throw new MidiModelError(`pitch must be int in [0,127], got ${n.pitch}`);
  }
  if (
    !Number.isInteger(n.velocity) ||
    n.velocity < VELOCITY_MIN ||
    n.velocity > VELOCITY_MAX
  ) {
    throw new MidiModelError(`velocity must be int in [1,127], got ${n.velocity}`);
  }
  parseI64(n.startTicks);
  if (parseI64(n.lengthTicks) <= 0n) {
    throw new MidiModelError(`note ${n.id} length must be > 0`);
  }
  if (n.channel !== undefined) {
    checkChannel(n.channel);
  }
  if (n.expression) checkExpression(n);
}

export function checkChannel(ch: number): void {
  if (!Number.isInteger(ch) || ch < CHANNEL_MIN || ch > CHANNEL_MAX) {
    throw new MidiModelError(`channel must be int in [0,15], got ${ch}`);
  }
}

function checkExpression(n: MidiNote): void {
  const e = n.expression!;
  for (const p of e.pitchBend ?? []) {
    if (!Number.isFinite(p.value) || p.value < BEND_MIN || p.value > BEND_MAX) {
      throw new MidiModelError(`per-note bend out of 14-bit range on ${n.id}`);
    }
    parseI64(p.ticksOffset);
  }
  for (const p of e.pressure ?? []) {
    if (!Number.isInteger(p.value) || p.value < 0 || p.value > 127) {
      throw new MidiModelError(`poly pressure out of range on ${n.id}`);
    }
    parseI64(p.ticksOffset);
  }
  for (const c of e.cc ?? []) {
    if (!Number.isInteger(c.controller) || c.controller < CC_MIN || c.controller > CC_MAX) {
      throw new MidiModelError(`cc controller out of range on ${n.id}`);
    }
    for (const p of c.points) {
      if (!Number.isInteger(p.value) || p.value < 0 || p.value > 127) {
        throw new MidiModelError(`cc value out of range on ${n.id}`);
      }
      parseI64(p.ticksOffset);
    }
  }
}

const byStartThenId = <T extends { id: string; startTicks: string }>(
  a: T,
  b: T,
): number => {
  const x = parseI64(a.startTicks);
  const y = parseI64(b.startTicks);
  return x < y ? -1 : x > y ? 1 : a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
};

/** Insert a note (sorted); duplicate id is an error. */
export function insertNote(notes: MidiNote[], note: MidiNote): MidiNote[] {
  checkNote(note);
  if (notes.some((n) => n.id === note.id)) {
    throw new MidiModelError(`duplicate note id '${note.id}'`);
  }
  return [...notes, note].sort(byStartThenId);
}

/**
 * Move a note to a new start (model-side). Articulation, channel and
 * per-note expression travel with the note — expression offsets are
 * relative to the note start by construction.
 */
export function moveNote(
  notes: MidiNote[],
  id: string,
  newStartTicks: string,
): MidiNote[] {
  const t = i64str(newStartTicks);
  return notes
    .map((n) => (n.id === id ? { ...n, startTicks: t } : n))
    .sort(byStartThenId);
}

/** Resize a note (length must stay > 0). */
export function resizeNote(
  notes: MidiNote[],
  id: string,
  newLengthTicks: string,
): MidiNote[] {
  const len = parseI64(newLengthTicks);
  if (len <= 0n) {
    throw new MidiModelError(`note ${id} length must be > 0`);
  }
  return notes.map((n) =>
    n.id === id ? { ...n, lengthTicks: i64str(len) } : n,
  );
}

export function removeNote(notes: MidiNote[], id: string): MidiNote[] {
  return notes.filter((n) => n.id !== id);
}

export function insertEvent(events: MidiEvent[], ev: MidiEvent): MidiEvent[] {
  checkChannel(ev.channel);
  if (events.some((e) => e.id === ev.id)) {
    throw new MidiModelError(`duplicate event id '${ev.id}'`);
  }
  return [...events, ev].sort(byStartThenId);
}

export function moveEvent(
  events: MidiEvent[],
  id: string,
  newStartTicks: string,
): MidiEvent[] {
  const t = i64str(newStartTicks);
  return events
    .map((e) => (e.id === id ? { ...e, startTicks: t } : e))
    .sort(byStartThenId);
}

/**
 * Quantize note start positions onto a grid. `strengthPpm` is a
 * parts-per-million pull toward the grid (1_000_000 = snap). Rounding
 * is nearest-ties-away from zero at both stages; all math is bigint.
 */
export function quantizeNotes(
  notes: MidiNote[],
  ids: string[],
  gridTicks: string,
  opts: { strengthPpm?: number; quantizeEnd?: boolean } = {},
): MidiNote[] {
  const grid = parseI64(gridTicks);
  if (grid <= 0n) {
    throw new MidiModelError(`grid must be > 0, got ${gridTicks}`);
  }
  const strength = BigInt(opts.strengthPpm ?? 1_000_000);
  if (strength < 0n || strength > 1_000_000n) {
    throw new MidiModelError(`strength ppm out of range: ${opts.strengthPpm}`);
  }
  const wanted = new Set(ids);
  const quantize = (t: string): string => {
    const v = parseI64(t);
    // nearest multiple, ties away from zero
    const q = ((2n * v + (v >= 0n ? grid : -grid)) / (2n * grid)) * grid;
    const moved = v + ((q - v) * strength) / 1_000_000n;
    return i64str(moved);
  };
  return notes
    .map((n) => {
      if (!wanted.has(n.id)) return n;
      const startTicks = quantize(n.startTicks);
      let lengthTicks = n.lengthTicks;
      if (opts.quantizeEnd) {
        const endQ = quantize(i64str(parseI64(n.startTicks) + parseI64(n.lengthTicks)));
        lengthTicks = i64str(parseI64(endQ) - parseI64(startTicks));
        if (parseI64(lengthTicks) <= 0n) {
          throw new MidiModelError(`quantizeEnd collapsed note ${n.id}`);
        }
      }
      return { ...n, startTicks, lengthTicks };
    })
    .sort(byStartThenId);
}

/** Replace the whole clip model defensively (sorted, validated). */
export function checkModel(m: MidiClipModel): MidiClipModel {
  if (!m.clipId) throw new MidiModelError('clip model needs clipId');
  for (const n of m.notes) checkNote(n);
  return m;
}
