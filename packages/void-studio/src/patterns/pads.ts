// Drum pads (SND-02, T59): pad bank → note events + optional slice refs.
//
// A pad is a trigger surface: it produces a RawGestureNote (one hit)
// that flows through the shared gesture commit path — ordinary notes,
// ordinary undo. When a pad maps to a sliced asset it carries the
// slice's asset/slice ids so a lane with audio playback can audition
// the right region; the NOTE still records what the user played.

import {
  clampGesturePitch,
  clampGestureVelocity,
  type RawGestureNote,
} from '../gestures/types';

export interface DrumPad {
  /** Stable id — also the choke/release key. */
  padId: string;
  /** GM-style trigger pitch the pad inserts. */
  pitch: number;
  label: string;
  /** Optional pattern-row link (StepGrid row this pad edits). */
  rowId?: string;
  /** Slice/asset binding for quick-sampled sounds (SND-02). */
  assetId?: string;
  sliceId?: string;
  /** Choke group: a hit in the same group silences siblings (engine
   *  concern; the data rides the pad so the surface can show it). */
  chokeGroup?: number;
}

/** General-Midi-flavoured 4×4 default bank — real pitches, labels honest. */
export function defaultPadBank(): DrumPad[] {
  const defs: [number, string, number?][] = [
    [36, 'Kick'],
    [38, 'Snare'],
    [40, 'Rim/SS'],
    [39, 'Clap'],
    [42, 'HH closed', 1],
    [46, 'HH open', 1],
    [44, 'HH pedal', 1],
    [41, 'Tom low'],
    [43, 'Tom mid'],
    [45, 'Tom mid hi'],
    [48, 'Tom hi'],
    [49, 'Crash'],
    [51, 'Ride'],
    [37, 'Sidestick'],
    [56, 'Cowbell'],
    [75, 'Claves'],
  ];
  return defs.map(([pitch, label, choke], i) => ({
    padId: `pad-${i}`,
    pitch,
    label,
    chokeGroup: choke,
  }));
}

export function findPad(pads: DrumPad[], padId: string): DrumPad | undefined {
  return pads.find((p) => p.padId === padId);
}

/** Pads that would choke `pad` — same nonzero group, different pad. */
export function chokedBy(pads: DrumPad[], pad: DrumPad): DrumPad[] {
  if (!pad.chokeGroup) return [];
  return pads.filter(
    (p) => p.padId !== pad.padId && p.chokeGroup === pad.chokeGroup,
  );
}

/** Bind a slice to a pad (quick-sampling mapping, SND-02). */
export function bindSliceToPad(
  pad: DrumPad,
  ref: { assetId: string; sliceId: string },
): DrumPad {
  return { ...pad, assetId: ref.assetId, sliceId: ref.sliceId };
}

/**
 * Pad hit → one raw note event for the shared commit path. Velocity is
 * the hit intensity (pointer pressure / key velocity, default 100);
 * `len` is the pad's fixed gate (drums are short — default 30ms-ish
 * ticks is the caller's choice).
 */
export function padToNote(
  pad: DrumPad,
  opts: { atTicks: string; velocity?: number; lengthTicks: string },
): RawGestureNote {
  return {
    index: 0,
    lengthTicks: opts.lengthTicks,
    rawStartTicks: opts.atTicks,
    rawPitch: clampGesturePitch(pad.pitch),
    rawVelocity: clampGestureVelocity(opts.velocity ?? 100),
  };
}
