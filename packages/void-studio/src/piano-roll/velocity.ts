// Velocity lane model — the bar strip under the note grid.
//
// One bar per visible note, height = velocity / 127 of the lane. Click or
// drag inside a bar sets that note's velocity via SetNoteOp. Pure math —
// no DOM, no state.

import { clampVelocity, VELOCITY_MAX, VELOCITY_MIN, type NoteView } from './notes';
import { parseI64 } from 'void-client';

/** px height of a velocity bar inside a lane `laneHeightPx` tall. */
export function velocityBarPx(velocity: number, laneHeightPx: number): number {
  const v = clampVelocity(velocity);
  return Math.max(1, Math.round((v / VELOCITY_MAX) * laneHeightPx));
}

/** velocity for a pointer y inside the lane (y measured from lane top). */
export function velocityAtPy(py: number, laneHeightPx: number): number {
  const frac = 1 - Math.min(1, Math.max(0, py / laneHeightPx));
  return clampVelocity(Math.round(frac * VELOCITY_MAX) || VELOCITY_MIN);
}

/**
 * Which note's velocity bar did px land on? Bars share the note grid's
 * horizontal projection, so we find the note whose [x, x+w) contains px.
 */
export function velocityHit(
  notes: { note: NoteView; x: number; w: number }[],
  px: number,
): NoteView | null {
  for (const n of notes) {
    if (px >= n.x && px <= n.x + n.w) return n.note;
  }
  return null;
}

/** Bars to paint: note rect x/w plus its height and value, pre-computed. */
export function velocityBars(
  notes: NoteView[],
  laneHeightPx: number,
): Array<{ note: NoteView; heightPx: number; velocity: number }> {
  return notes.map((note) => ({
    note,
    heightPx: velocityBarPx(note.velocity, laneHeightPx),
    velocity: note.velocity,
  }));
}

/** Adjust a velocity toward bounds — used by keyboard ± intents. */
export function adjustedVelocity(current: number, delta: number): number {
  return clampVelocity(current + delta);
}

export { VELOCITY_MIN, VELOCITY_MAX };
