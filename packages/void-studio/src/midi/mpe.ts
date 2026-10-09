// MPE per-note channel model (W18; MIDI-07, T71).
//
// MPE (MIDI 1.0 profile) puts per-note expression on per-note member
// channels inside a zone: channel 1 is the zone master, channels 2–15
// are member channels in a standard full-range MPE zone (lower zone,
// master channel 0, 15 members — MIDI channels are 0-indexed here).
//
// `allocateZone` assigns a member channel to each note for its
// lifetime — notes whose time ranges overlap must get different
// channels. It is deterministic (sorted walk, lowest free channel),
// not a live-performance allocator: editing a note re-allocates from
// scratch, which keeps serialize→edit→serialize round-trips stable.
//
// `flattenToMidi1` emits the MIDI 1.0 form: notes land on their member
// channel with their per-note bend/pressure/CC translated to channel
// messages on that channel. Losses are explicit `MpeLoss` entries —
// nothing is dropped silently (T71). What survives: per-note bend,
// per-note pressure (poly aftertouch), per-note CC. What is lost:
// expression on a note that could not get a channel (channel
// exhaustion beyond 15 overlapping notes), and anything the format
// cannot carry at all (nothing else — MIDI 1.0 carries all three).
//
// MIDI 2.0 is a DECLARED MODEL ONLY: `MIDI2_TRANSPORT` is false and
// `midi2Envelope` builds the descriptor but never claims wire support.

import { i64str, parseI64 } from 'void-client';
import type { ExprPoint, MidiClipModel, MidiEvent, MidiNote } from './types';
import { CHANNEL_MAX, CHANNEL_MIN } from './types';
import { MidiModelError } from './events';

/** Standard MPE zone: master ch 0 + 15 member channels (1..15). */
export interface MpeZone {
  masterChannel: number; // usually 0 (lower zone)
  memberLow: number; // usually 1
  memberHigh: number; // usually 15
}

export const STANDARD_ZONE: MpeZone = {
  masterChannel: 0,
  memberLow: 1,
  memberHigh: 15,
};

/**
 * Assign member channels. Returns notes with `channel` set, plus
 * `unassigned` — notes that couldn't get a channel because more than
 * 15 notes overlapped. Unassigned notes keep `channel: undefined`;
 * their expression is a declared loss at flatten time.
 */
export function allocateZone(
  notes: MidiNote[],
  zone: MpeZone = STANDARD_ZONE,
): { notes: MidiNote[]; unassigned: string[] } {
  const span = zone.memberHigh - zone.memberLow + 1;
  if (span <= 0) throw new MidiModelError('MPE zone has no member channels');
  const sorted = [...notes].sort((a, b) => {
    const x = parseI64(a.startTicks);
    const y = parseI64(b.startTicks);
    return x < y ? -1 : x > y ? 1 : a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
  });
  const channelEnd = new Map<number, bigint>(); // channel → end of last note on it
  const unassigned: string[] = [];
  const out = sorted.map((n) => {
    const s = parseI64(n.startTicks);
    const e = s + parseI64(n.lengthTicks);
    let ch: number | undefined;
    for (let c = zone.memberLow; c <= zone.memberHigh; c++) {
      const busyUntil = channelEnd.get(c);
      // A channel is free when no note is on it yet, or its last note
      // ended at or before this note's start (end-exclusive overlap).
      if (busyUntil === undefined || s >= busyUntil) {
        ch = c;
        break;
      }
    }
    if (ch === undefined) {
      unassigned.push(n.id);
      return { ...n, channel: undefined };
    }
    channelEnd.set(ch, e > (channelEnd.get(ch) ?? 0n) ? e : channelEnd.get(ch)!);
    return { ...n, channel: ch };
  });
  return { notes: out, unassigned };
}

/** A documented MIDI-1.0-flattening loss. Never silently dropped. */
export interface MpeLoss {
  /** Stable machine code for tests/reports. */
  code:
    | 'CHANNEL_EXHAUSTED' // >15 overlapping notes; note+expression lost
    | 'NO_CHANNEL' // note had no member channel to carry expression
    | 'FORMAT_LIMIT'; // the format cannot express this datum
  noteId?: string;
  detail: string;
}

export interface Midi1Clip {
  clipId: string;
  /** Note on/off represented as compact note rows on member channels. */
  notes: MidiNote[];
  /** Translated channel events (incl. per-note expression on member ch). */
  events: MidiEvent[];
  losses: MpeLoss[];
}

/**
 * Flatten a clip to plain MIDI 1.0 channel messages. Per-note bend →
 * pitchBend on the member channel at absolute ticks; per-note pressure
 * → polyPressure; per-note CC → cc. Losses are enumerated, not hidden.
 */
export function flattenToMidi1(
  clip: MidiClipModel,
  zone: MpeZone = STANDARD_ZONE,
): Midi1Clip {
  const { notes, unassigned } = allocateZone(clip.notes, zone);
  const unassignedSet = new Set(unassigned);
  const losses: MpeLoss[] = unassigned.map((id) => ({
    code: 'CHANNEL_EXHAUSTED',
    noteId: id,
    detail: `note '${id}' could not get an MPE member channel (>15 overlapping); its per-note expression is lost on flatten`,
  }));
  const events: MidiEvent[] = [...clip.events];
  const kept: MidiNote[] = [];
  for (const n of notes) {
    if (n.channel === undefined) {
      if (!unassignedSet.has(n.id)) {
        losses.push({
          code: 'NO_CHANNEL',
          noteId: n.id,
          detail: `note '${n.id}' has no member channel; expression dropped`,
        });
      }
      continue; // note itself is lost too — channel is its identity
    }
    kept.push(n);
    const expr = n.expression;
    if (!expr) continue;
    const base = parseI64(n.startTicks);
    let seq = 0;
    for (const p of expr.pitchBend ?? []) {
      events.push({
        kind: 'pitchBend',
        id: `mpe-${n.id}-b${seq++}`,
        startTicks: i64str(base + parseI64(p.ticksOffset)),
        channel: n.channel,
        value: Math.round(p.value),
      });
    }
    for (const p of expr.pressure ?? []) {
      events.push({
        kind: 'polyPressure',
        id: `mpe-${n.id}-p${seq++}`,
        startTicks: i64str(base + parseI64(p.ticksOffset)),
        channel: n.channel,
        notePitch: n.pitch,
        value: p.value,
      });
    }
    for (const c of expr.cc ?? []) {
      for (const p of c.points) {
        events.push({
          kind: 'cc',
          id: `mpe-${n.id}-c${seq++}`,
          startTicks: i64str(base + parseI64(p.ticksOffset)),
          channel: n.channel,
          controller: c.controller,
          value: p.value,
        });
      }
    }
  }
  events.sort((a, b) => {
    const x = parseI64(a.startTicks);
    const y = parseI64(b.startTicks);
    return x < y ? -1 : x > y ? 1 : a.id < b.id ? -1 : a.id > b.id ? 1 : 0;
  });
  return { clipId: clip.clipId, notes: kept, events, losses };
}

// ---- MIDI 2.0: declared model only -------------------------------------

/** False forever in this package — MIDI 2.0 transport is NOT wired. */
export const MIDI2_TRANSPORT = false;

/**
 * The declared MIDI 2.0 form: per-note 32-bit bend, 32-bit pressure and
 * per-note controllers ride the note itself (no channel allocation).
 * This is a model descriptor for forward work, never a transport claim
 * — there is no encoder, and `MIDI2_TRANSPORT` stays false.
 */
export interface Midi2NoteDescriptor {
  noteId: string;
  pitch: number;
  attributeType?: number; // MIDI 2.0 note attribute (e.g. pitch-7.9)
  attributeData?: number;
  perNoteBend?: ExprPoint[]; // declared 32-bit domain (value in ticks)
  perNotePressure?: ExprPoint[];
  perNoteControllers?: { index: number; points: ExprPoint[] }[];
}

export function midi2Descriptor(n: MidiNote): Midi2NoteDescriptor {
  return {
    noteId: n.id,
    pitch: n.pitch,
    perNoteBend: n.expression?.pitchBend,
    perNotePressure: n.expression?.pressure,
    perNoteControllers: n.expression?.cc?.map((c) => ({
      index: c.controller,
      points: c.points,
    })),
  };
}
