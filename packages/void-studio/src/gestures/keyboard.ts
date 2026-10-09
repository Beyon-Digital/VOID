// Keyboard/numeric equivalents (GEST-06, T57/T39).
//
// Every gesture has a non-pointer path that produces the SAME raw note
// list the pointer path does — parity, not a degraded mode:
//
//   contour draw  ⇔  step entry: arrows move a cursor, keys set
//                    pitch/velocity, Enter places a note
//   tap rhythm    ⇔  numeric onset list (onsetsToRawNotes) or a key
//                    that taps on the capture clock
//
// The step-entry model is pure state — a React layer renders the cursor
// and feeds keys; tests drive it headless.

import { parseI64 } from 'void-client';
import { clampGesturePitch, clampGestureVelocity } from './types';
import type { RawGestureNote } from './types';

export interface StepEntryState {
  /** Insert position in ticks. */
  cursorTicks: string;
  /** Working pitch for the next placed note. */
  pitch: number;
  /** Working velocity 1..127. */
  velocity: number;
  /** Working note length in ticks. */
  lengthTicks: string;
  /** Cursor step in ticks (arrow granularity — usually the grid step). */
  stepTicks: string;
  /** Placed notes — RawGestureNote, identical shape to gesture output. */
  notes: RawGestureNote[];
}

export function initialStepEntry(stepTicks = '240000'): StepEntryState {
  return {
    cursorTicks: '0',
    pitch: 60,
    velocity: 100,
    lengthTicks: stepTicks,
    stepTicks,
    notes: [],
  };
}

/** Numeric entry — set a working field by value (bounded input field). */
export function stepEntrySet(
  s: StepEntryState,
  fields: Partial<
    Pick<
      StepEntryState,
      'cursorTicks' | 'pitch' | 'velocity' | 'lengthTicks' | 'stepTicks'
    >
  >,
): StepEntryState {
  const next = { ...s, ...fields };
  next.pitch = clampGesturePitch(next.pitch);
  next.velocity = clampGestureVelocity(next.velocity);
  if (parseI64(next.lengthTicks) < 1n) next.lengthTicks = '1';
  if (parseI64(next.stepTicks) < 1n) next.stepTicks = '1';
  return next;
}

/**
 * One key press → updated state (or null for a key the editor owns).
 *   ArrowLeft/Right  cursor −/+ one step
 *   ArrowUp/Down     pitch ±1 (Shift ±12)
 *   PageUp/Down      velocity ±8 (Shift ±1)
 *   Enter/Space      place note at cursor; cursor advances by length
 *   Backspace/Delete remove the last placed note (cursor backs up)
 *   Home             cursor to 0
 */
export function stepEntryKey(
  s: StepEntryState,
  key: string,
  mods: { shift?: boolean } = {},
): StepEntryState | null {
  const step = parseI64(s.stepTicks);
  switch (key) {
    case 'ArrowLeft':
      return {
        ...s,
        cursorTicks: (parseI64(s.cursorTicks) - step).toString(10),
      };
    case 'ArrowRight':
      return {
        ...s,
        cursorTicks: (parseI64(s.cursorTicks) + step).toString(10),
      };
    case 'ArrowUp':
      return { ...s, pitch: clampGesturePitch(s.pitch + (mods.shift ? 12 : 1)) };
    case 'ArrowDown':
      return { ...s, pitch: clampGesturePitch(s.pitch - (mods.shift ? 12 : 1)) };
    case 'PageUp':
      return {
        ...s,
        velocity: clampGestureVelocity(s.velocity + (mods.shift ? 1 : 8)),
      };
    case 'PageDown':
      return {
        ...s,
        velocity: clampGestureVelocity(s.velocity - (mods.shift ? 1 : 8)),
      };
    case 'Home':
      return { ...s, cursorTicks: '0' };
    case 'Enter':
    case ' ': {
      const note: RawGestureNote = {
        index: s.notes.length,
        lengthTicks: s.lengthTicks,
        rawStartTicks: s.cursorTicks,
        rawPitch: s.pitch,
        rawVelocity: s.velocity,
      };
      return {
        ...s,
        notes: [...s.notes, note],
        cursorTicks: (parseI64(s.cursorTicks) + parseI64(s.lengthTicks)).toString(
          10,
        ),
      };
    }
    case 'Backspace':
    case 'Delete': {
      const last = s.notes[s.notes.length - 1];
      if (!last) return s;
      return {
        ...s,
        notes: s.notes.slice(0, -1),
        cursorTicks: last.rawStartTicks,
      };
    }
    default:
      return null;
  }
}

/** Sort step-entry notes by onset and re-index (mirrors capture order). */
export function sortStepEntryNotes(notes: RawGestureNote[]): RawGestureNote[] {
  return [...notes]
    .sort((a, b) => {
      const d = parseI64(a.rawStartTicks) - parseI64(b.rawStartTicks);
      return d < 0n ? -1 : d > 0n ? 1 : 0;
    })
    .map((n, i) => ({ ...n, index: i }));
}
