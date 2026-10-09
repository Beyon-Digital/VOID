// Chord identification (INTEL-02): pitch collection → ranked candidates.
//
// Scores are honest about ambiguity: confidence = coverage × precision
// (how much of the chord is present × how much of the input is chord
// tones). A C-E-G collection ranks Cmaj first but Am7 still appears —
// real ambiguity stays visible. Low-confidence results are SUGGESTIONS
// ONLY: isConfident() is the gate, and nothing here ever applies a
// reharmonization (T57/T58/T59; INTEL-02 acceptance).

import {
  ALL_QUALITIES,
  pitchClasses,
  type ChordSymbol,
} from './chords';

export interface ChordGuess {
  chord: ChordSymbol;
  /** 0..1: coverage × precision (see below). */
  confidence: number;
  /** Chord tones found in the input (pitch classes). */
  covered: number[];
  /** Input pitch classes NOT in the chord. */
  extra: number[];
}

/** Below this a guess is display-only — the UI must label it tentative. */
export const CONFIDENCE_SUGGESTION = 0.6;

export function isConfident(g: ChordGuess): boolean {
  return g.confidence >= CONFIDENCE_SUGGESTION;
}

/**
 * Identify from a pitch collection (MIDI pitches — pcs derived mod 12).
 * Returns every candidate sorted by confidence desc, capped at `limit`.
 */
export function identifyChord(pitches: number[], limit = 4): ChordGuess[] {
  const pcs = new Set(
    pitches
      .filter((p) => Number.isFinite(p))
      .map((p) => ((Math.round(p) % 12) + 12) % 12),
  );
  if (pcs.size === 0) return [];
  const input = [...pcs];
  const guesses: ChordGuess[] = [];
  for (let rootPc = 0; rootPc < 12; rootPc++) {
    for (const quality of ALL_QUALITIES) {
      const chord: ChordSymbol = { rootPc, quality };
      const tones = pitchClasses(chord);
      const covered = input.filter((pc) => tones.has(pc));
      if (covered.length === 0) continue;
      const extra = input.filter((pc) => !tones.has(pc));
      const coverage = covered.length / tones.size;
      const precision = covered.length / input.length;
      const confidence = coverage * precision;
      guesses.push({ chord, confidence, covered, extra });
    }
  }
  guesses.sort((a, b) => {
    if (b.confidence !== a.confidence) return b.confidence - a.confidence;
    // Deterministic tie-break: fewer extra notes, then simpler chord.
    if (a.extra.length !== b.extra.length) return a.extra.length - b.extra.length;
    return (
      pitchClasses(a.chord).size - pitchClasses(b.chord).size ||
      a.chord.rootPc - b.chord.rootPc
    );
  });
  return guesses.slice(0, Math.max(1, limit));
}
