// MIDI event model (W18; MIDI-04/05/06, T71).
//
// All ticks are decimal-string int64 (CONTRACTS §1). A note carries its
// identity, geometry and — when present — an articulation id and an MPE
// per-note expression bundle. Channel-level events (CC, bend, pressure)
// are a separate list: they are never merged into notes and notes never
// collapse into channel events — each kind edits independently.

import type { I64 } from 'void-client';
import {
  PITCH_MAX,
  PITCH_MIN,
  VELOCITY_MAX,
  VELOCITY_MIN,
} from '../piano-roll/notes';

export type Ticks = I64;

/** One expression breakpoint, offset from the owning note's start. */
export interface ExprPoint {
  /** Offset from note startTicks (non-negative i64 decimal string). */
  ticksOffset: Ticks;
  value: number;
}

/**
 * MPE per-note expression — rides the note's assigned member channel.
 * pitchBend is 14-bit (-8192..8191); pressure is 7-bit poly aftertouch;
 * cc entries are per-note controllers (e.g. CC74 timbre).
 */
export interface NoteExpression {
  pitchBend?: ExprPoint[];
  pressure?: ExprPoint[];
  cc?: { controller: number; points: ExprPoint[] }[];
}

export interface MidiNote {
  id: string;
  pitch: number; // 0..127
  velocity: number; // 1..127
  startTicks: Ticks;
  lengthTicks: Ticks;
  /** 0..15; assigned by the MPE allocator or left undefined. */
  channel?: number;
  /** Articulation id resolved through the clip's ArticulationSet. */
  articulationId?: string;
  expression?: NoteExpression;
}

/** Channel-level (non-note) events in the event-list model. */
export type MidiEventKind =
  | 'cc'
  | 'pitchBend'
  | 'channelPressure'
  | 'polyPressure'
  | 'programChange'
  /** Keyswitch emitted by an articulation map (see articulation.ts). */
  | 'keyswitch';

export interface MidiEvent {
  kind: MidiEventKind;
  id: string;
  startTicks: Ticks;
  channel: number; // 0..15
  /** cc only: controller number 0..127. */
  controller?: number;
  /** cc/channelPressure: 0..127; pitchBend: -8192..8191; polyPressure: 0..127. */
  value?: number;
  /** polyPressure only: which note pitch the pressure addresses. */
  notePitch?: number;
  /** programChange only: program number 0..127. */
  program?: number;
}

/** The clip's editable MIDI content — notes plus channel events. */
export interface MidiClipModel {
  clipId: string;
  notes: MidiNote[];
  events: MidiEvent[];
}

export { PITCH_MIN, PITCH_MAX, VELOCITY_MIN, VELOCITY_MAX };
export const CHANNEL_MIN = 0;
export const CHANNEL_MAX = 15;
export const CC_MIN = 0;
export const CC_MAX = 127;
export const BEND_MIN = -8192;
export const BEND_MAX = 8191;
