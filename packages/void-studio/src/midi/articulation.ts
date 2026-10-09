// Articulation sets + keyswitch maps (W18; MIDI-06, T71).
//
// An ArticulationSet names articulations (legato/staccato/pizz/…) and
// the keyswitches that trigger them in the patch. A note carries an
// articulationId; editing ops keep it (articulation + expression are
// note data and move with the note).
//
// `keyswitchesFor` derives the switch events a MIDI 1.0 export must
// emit: walking notes in start order, whenever the articulation
// changes a KeyswitchEvent is emitted `leadTicks` before the note's
// start. A KeyswitchEvent is its own kind — it is neither a normal CC
// nor a normal note, so the event model does not pretend it is one.

import { i64str, parseI64 } from 'void-client';
import type { MidiEvent, MidiNote } from './types';
import { CHANNEL_MAX, CHANNEL_MIN, PITCH_MAX, PITCH_MIN } from './types';
import { MidiModelError } from './events';

/** How a keyswitch is emitted on the wire. */
export type KeyswitchForm =
  | { kind: 'noteOn'; pitch: number } // dedicated switch key
  | { kind: 'cc'; controller: number; value: number };

export interface ArticulationDef {
  id: string;
  name: string;
  /** Which keyswitch selects this articulation. */
  keyswitch: KeyswitchForm;
  /** Optional per-articulation transposition (e.g. octave-shifted strings). */
  transposeSemis?: number;
}

export interface ArticulationSet {
  id: string;
  name: string;
  /** 0..15 — the channel keyswitches are emitted on. */
  channel: number;
  articulations: ArticulationDef[];
}

export function checkSet(set: ArticulationSet): void {
  if (
    !Number.isInteger(set.channel) ||
    set.channel < CHANNEL_MIN ||
    set.channel > CHANNEL_MAX
  ) {
    throw new MidiModelError('articulation set channel out of range');
  }
  const ids = new Set<string>();
  for (const a of set.articulations) {
    if (!a.id || ids.has(a.id)) {
      throw new MidiModelError(`bad/duplicate articulation id '${a.id}'`);
    }
    ids.add(a.id);
    const k = a.keyswitch;
    if (
      k.kind === 'noteOn' &&
      (!Number.isInteger(k.pitch) || k.pitch < PITCH_MIN || k.pitch > PITCH_MAX)
    ) {
      throw new MidiModelError(`keyswitch pitch out of range for '${a.id}'`);
    }
    if (
      k.kind === 'cc' &&
      (!Number.isInteger(k.value) || k.value < 0 || k.value > 127)
    ) {
      throw new MidiModelError(`keyswitch cc value out of range for '${a.id}'`);
    }
  }
}

export function articulationOf(
  set: ArticulationSet,
  id: string,
): ArticulationDef | undefined {
  return set.articulations.find((a) => a.id === id);
}

/** Assign (or clear) an articulation on a note; validates against the set. */
export function setArticulation(
  set: ArticulationSet,
  notes: MidiNote[],
  noteId: string,
  articulationId: string | undefined,
): MidiNote[] {
  if (articulationId !== undefined && !articulationOf(set, articulationId)) {
    throw new MidiModelError(
      `articulation '${articulationId}' not in set '${set.id}'`,
    );
  }
  return notes.map((n) =>
    n.id === noteId ? { ...n, articulationId } : n,
  );
}

/**
 * A keyswitch is neither a normal CC nor a normal note in the event
 * model — it is its own kind so exports can render it faithfully.
 */
export interface KeyswitchEvent extends MidiEvent {
  kind: 'keyswitch';
  keyswitch: KeyswitchForm;
}

export function makeKeyswitchEvent(
  id: string,
  ticks: string,
  channel: number,
  keyswitch: KeyswitchForm,
): KeyswitchEvent {
  return { kind: 'keyswitch', keyswitch, id, startTicks: ticks, channel };
}

export function isKeyswitchEvent(e: MidiEvent): e is KeyswitchEvent {
  return e.kind === 'keyswitch';
}

/**
 * Keyswitch events needed before the first note of each articulation
 * run. Walks notes in start order; whenever the articulation *changes*
 * a switch is emitted `leadTicks` before the note's start (clamped at
 * 0). Untagged notes keep the previous articulation active (they do
 * not re-trigger a switch).
 */
export function keyswitchesFor(
  set: ArticulationSet,
  notes: MidiNote[],
  opts: { leadTicks?: string } = {},
): KeyswitchEvent[] {
  checkSet(set);
  const lead = parseI64(opts.leadTicks ?? '0');
  const sorted = [...notes].sort((a, b) => {
    const x = parseI64(a.startTicks);
    const y = parseI64(b.startTicks);
    return x < y ? -1 : x > y ? 1 : a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
  });
  const events: KeyswitchEvent[] = [];
  let active: string | undefined;
  let seq = 0;
  for (const n of sorted) {
    const art = n.articulationId;
    if (art === undefined || art === active) continue;
    const def = articulationOf(set, art);
    if (!def) continue;
    active = art;
    const at = parseI64(n.startTicks) - lead;
    events.push(
      makeKeyswitchEvent(
        `ks-${seq++}`,
        i64str(at < 0n ? 0n : at),
        set.channel,
        def.keyswitch,
      ),
    );
  }
  return events;
}
