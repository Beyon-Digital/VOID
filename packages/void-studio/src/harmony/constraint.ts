// Harmonic constraint for gesture/step input (TIME-05, T57/T59).
//
// The constraint is a PITCH-CLASS SNAP applied at render time — part of
// the reversible transform chain, so toggling it never rewrites raw
// input: with mode 'off' the raw pitch passes through verbatim.
// Snapping targets the chord region covering the note's onset; notes
// outside every region fall back to the configured scale (or pass
// through when none is set — an uncovered region is honest, not
// silently chromatic-clamped to a wrong chord).

import {
  impliedScale,
  pitchClasses,
  scaleSet,
  SCALES,
  type ChordSymbol,
} from './chords';
import { chordAt, type ChordTrack } from './track';

export type ConstraintMode = 'off' | 'chord' | 'scale';

export interface HarmonyConstraintSpec {
  mode: ConstraintMode;
  track: ChordTrack;
  /** Scale used when no chord region covers the onset (or as the
   *  'scale' fallback policy). rootPc + one of SCALES. */
  fallbackScale?: { rootPc: number; scale: keyof typeof SCALES | number[] };
  /** Max snap distance in semitones (default 6 — never leap a tritone+). */
  limitSemitones?: number;
}

/**
 * Nearest pitch whose class is in `pcs`, within `limit` semitones.
 * Ties prefer the LOWER pitch (documented). Returns the input when the
 * set is empty or nothing is within range — constraint never invents.
 */
export function snapPitchToSet(
  pitch: number,
  pcs: ReadonlySet<number>,
  limitSemitones = 6,
): number {
  const p = Math.min(127, Math.max(0, Math.round(pitch)));
  if (pcs.size === 0) return p;
  const lim = Math.max(0, Math.min(12, Math.round(limitSemitones)));
  for (let d = 0; d <= lim; d++) {
    const down = p - d;
    const up = p + d;
    if (down >= 0 && pcs.has(((down % 12) + 12) % 12)) return down;
    if (up <= 127 && pcs.has(((up % 12) + 12) % 12)) return up;
  }
  return p;
}

function fallbackSet(spec: HarmonyConstraintSpec): Set<number> | null {
  const f = spec.fallbackScale;
  if (!f) return null;
  const scale = Array.isArray(f.scale) ? f.scale : SCALES[f.scale];
  return scale ? scaleSet(f.rootPc, scale) : null;
}

/** The pitch-class set governing an onset under `spec` (null = free). */
export function governingSet(
  onsetTicks: string,
  spec: HarmonyConstraintSpec,
): Set<number> | null {
  if (spec.mode === 'off') return null;
  const region = chordAt(spec.track, onsetTicks);
  if (region) {
    return spec.mode === 'chord'
      ? pitchClasses(region.chord)
      : new Set(impliedScale(region.chord).map((i) => (region.chord.rootPc + i) % 12));
  }
  return fallbackSet(spec);
}

/**
 * Constraint as a PitchConstraint hook for the gesture render chain.
 * Snaps against the set governing each note's RAW onset.
 */
export function makeConstraint(
  spec: HarmonyConstraintSpec,
): ((pitch: number, onsetTicks: string) => number) | null {
  if (spec.mode === 'off') return null;
  return (pitch, onsetTicks) => {
    const set = governingSet(onsetTicks, spec);
    if (!set) return pitch;
    return snapPitchToSet(pitch, set, spec.limitSemitones ?? 6);
  };
}

