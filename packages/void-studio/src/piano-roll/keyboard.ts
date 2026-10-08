// Piano-roll keyboard model — musical typing + edit intents.
//
// Two things live here, both pure and testable:
//  1) the DAW-style musical-typing map (z-row = lower octave, q-row =
//     upper octave — the layout Logic/FL-style editors use), and
//  2) arrow/edit intents for the selected notes, expressed in ticks +
//     semitones so the React layer stays a thin dispatcher.

import type { SnapSettings } from '../workspaces/editorStore';
import { gridStepTicks } from '../timeline/snap';
import { clampPitch, clampVelocity } from './notes';

// Lower row:  z s x d c v g b h n j m  -> C..B (one octave + C)
// Upper row:  q 2 w 3 e r 5 t 6 y 7 u  -> +12 semitones, i o/9 p/0 extend
const KEY_OFFSETS: Record<string, number> = {
  z: 0, s: 1, x: 2, d: 3, c: 4, v: 5, g: 6, b: 7, h: 8, n: 9, j: 10, m: 11,
  ',': 12,
  q: 12, '2': 13, w: 14, '3': 15, e: 16, r: 17, '5': 18, t: 19, '6': 20,
  y: 21, '7': 22, u: 23, i: 24, '9': 25, o: 26, '0': 27, p: 28,
};

/**
 * Musical-typing key -> MIDI pitch. `basePitch` is the note for 'z'
 * (convention: 60 = C4 so 'z' plays C4). Returns null for non-note keys.
 */
export function pitchForKey(key: string, basePitch = 60): number | null {
  const off = KEY_OFFSETS[key.toLowerCase()];
  if (off === undefined) return null;
  return clampPitch(basePitch + off);
}

export type NoteEditIntent =
  | { type: 'move'; dTicks: string; dPitch: number }
  | { type: 'resize-end'; dTicks: string }
  | { type: 'velocity'; dVelocity: number }
  | { type: 'delete' }
  | { type: 'noop' };

/**
 * Map an editing key press to a note-edit intent for the selection.
 * Grid step comes from the snap settings (beat/1/8/…) so keyboard moves
 * land on the same grid pointer gestures do.
 *   ArrowLeft/Right         -> move by one grid step
 *   ArrowUp/Down            -> move by one semitone (Shift = octave)
 *   Alt+ArrowLeft/Right     -> resize end by one grid step
 *   + / - (= / -)           -> velocity ±8 (Shift = ±1)
 *   Delete/Backspace        -> delete
 */
export function noteEditIntent(
  key: string,
  mods: { shift?: boolean; alt?: boolean },
  snap: SnapSettings,
): NoteEditIntent {
  const step = gridStepTicks(snap).toString(10);
  switch (key) {
    case 'ArrowLeft':
      return mods.alt
        ? { type: 'resize-end', dTicks: `-${step}` }
        : { type: 'move', dTicks: `-${step}`, dPitch: 0 };
    case 'ArrowRight':
      return mods.alt
        ? { type: 'resize-end', dTicks: step }
        : { type: 'move', dTicks: step, dPitch: 0 };
    case 'ArrowUp':
      return { type: 'move', dTicks: '0', dPitch: mods.shift ? 12 : 1 };
    case 'ArrowDown':
      return { type: 'move', dTicks: '0', dPitch: mods.shift ? -12 : -1 };
    case 'Delete':
    case 'Backspace':
      return { type: 'delete' };
    case '=':
    case '+':
      return { type: 'velocity', dVelocity: mods.shift ? 1 : 8 };
    case '-':
    case '_':
      return { type: 'velocity', dVelocity: mods.shift ? -1 : -8 };
    default:
      return { type: 'noop' };
  }
}

export { clampPitch, clampVelocity };
