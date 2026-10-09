// Gesture types (W14/GEST-01, GEST-05) — raw input plus its rendered
// musical interpretation.
//
// OWNERSHIP: everything here is transient input state / pending preview
// (CONTRACTS.md WebView row). A gesture never carries authority — commit
// produces ordinary InsertNoteOps through the normal command path, and a
// preview is a ghost layer that is never sent to the engine.
//
// RAW RETENTION RULE (T57): `raw*` fields hold the captured intention
// exactly as received. Every rendered field is recomputed from raw by
// the transform chain — transforms never feed back into raw, so changing
// quantize strength can never compound earlier quantization (MIDI-02).

/** One pointer sample of a contour draw (ms clock, surface px). */
export interface GesturePoint {
  /** Milliseconds on the capture clock (performance.now()-like). */
  tMs: number;
  /** Surface coordinates — x maps to time, y maps to pitch. */
  x: number;
  y: number;
}

/** One rhythm tap. `velocity` may come from pressure/key repeat; absent
 *  falls back to the capture's default. */
export interface TapEvent {
  /** Milliseconds on the capture clock. */
  tMs: number;
  /** 1..127 when the input carries intensity. */
  velocity?: number;
}

/** Which capture mode produced the phrase. */
export type GestureKind = 'contour' | 'rhythm';

/**
 * A note event derived from raw gesture input. `raw*` is immutable
 * captured data; `pitch`/`velocity`/`startTicks` are the rendered
 * interpretation after the current transform + harmonic constraint.
 */
export interface GestureNote {
  /** Position in the phrase — also the deterministic humanize lane. */
  index: number;
  /** Rendered pitch, 0..127 (after optional constraint). */
  pitch: number;
  /** Rendered velocity, 1..127 (after optional humanize). */
  velocity: number;
  /** Rendered onset, ticks (after quantize → swing → humanize). */
  startTicks: string;
  /** Rendered length, ticks. */
  lengthTicks: string;
  /** Captured onset before any transform — always preserved. */
  rawStartTicks: string;
  /** Captured pitch before any constraint. */
  rawPitch: number;
  /** Captured velocity before humanize. */
  rawVelocity: number;
}

/** The raw-only half of a GestureNote — what the capture step emits. */
export type RawGestureNote = Pick<
  GestureNote,
  'index' | 'lengthTicks' | 'rawStartTicks' | 'rawPitch' | 'rawVelocity'
>;

/** Optional pitch constraint hook (harmony/ supplies the real one). */
export type PitchConstraint = (pitch: number, onsetTicks: string) => number;

/**
 * Reversible timing transform (MIDI-02). Everything is integer math in
 * ticks; "ppm" fields are parts-per-million so the settings survive
 * JSON round-trips without float drift.
 */
export interface RhythmTransform {
  /** Grid step in ticks; '0' disables quantization. */
  gridTicks: string;
  /** Quantize strength 0..1_000_000: 0 = raw timing, 1e6 = full snap. */
  quantizeStrengthPpm: number;
  /**
   * Swing placement for the second half of each grid pair, 0..1_000_000.
   * 500_000 = straight (delta 0); ~666_667 ≈ triplet feel. Values below
   * 500_000 push off-beats early (negative swing — legal, documented).
   */
  swingPpm: number;
  /** Max ± humanize timing jitter in ticks; '0' disables. */
  humanizeTimingTicks: string;
  /** Max ± humanize velocity jitter; 0 disables. */
  humanizeVelocity: number;
  /** Deterministic seed — same seed + note index ⇒ same jitter. */
  seed: string;
}

export const PPM = 1_000_000;
/** Straight swing — the neutral value, not a magic default elsewhere. */
export const SWING_STRAIGHT_PPM = 500_000;

export function defaultTransform(gridTicks: string = '240000'): RhythmTransform {
  return {
    gridTicks,
    quantizeStrengthPpm: 0,
    swingPpm: SWING_STRAIGHT_PPM,
    humanizeTimingTicks: '0',
    humanizeVelocity: 0,
    seed: 'void-gesture',
  };
}

/** Clamp helper shared by capture code; mirrors piano-roll ranges. */
export function clampGesturePitch(p: number): number {
  if (!Number.isFinite(p)) return 0;
  return Math.min(127, Math.max(0, Math.round(p)));
}

export function clampGestureVelocity(v: number): number {
  if (!Number.isFinite(v)) return 1;
  return Math.min(127, Math.max(1, Math.round(v)));
}
