// Chord symbols and pitch-class sets (TIME-05, INTEL-02, T57/T59).
//
// Real pitch-class arithmetic — no lookup fakery: each quality maps to
// its semitone intervals from the root, `pitchClasses` reduces mod 12.
// Parsing is strict: an unrecognized suffix returns null rather than a
// guessed chord (INTEL-02: wrong harmony is worse than no harmony).

export type ChordQuality =
  | 'maj'
  | 'min'
  | 'dim'
  | 'aug'
  | 'sus2'
  | 'sus4'
  | '6'
  | 'm6'
  | '7'
  | 'maj7'
  | 'min7'
  | 'min7b5'
  | 'dim7'
  | 'add9';

export interface ChordSymbol {
  /** Root pitch class 0..11 (0 = C). */
  rootPc: number;
  quality: ChordQuality;
  /** Present when parsed/written with a flat root spelling (Bb, Eb…)
   *  — formatChordSymbol preserves the user's enharmonic choice. */
  flat?: true;
}

/** Semitone intervals above the root for each quality. */
export const CHORD_INTERVALS: Record<ChordQuality, readonly number[]> = {
  maj: [0, 4, 7],
  min: [0, 3, 7],
  dim: [0, 3, 6],
  aug: [0, 4, 8],
  sus2: [0, 2, 7],
  sus4: [0, 5, 7],
  '6': [0, 4, 7, 9],
  m6: [0, 3, 7, 9],
  '7': [0, 4, 7, 10],
  maj7: [0, 4, 7, 11],
  min7: [0, 3, 7, 10],
  min7b5: [0, 3, 6, 10],
  dim7: [0, 3, 6, 9],
  add9: [0, 2, 4, 7],
};

export const ALL_QUALITIES: ChordQuality[] = [
  'maj', 'min', 'dim', 'aug', 'sus2', 'sus4', '6', 'm6',
  '7', 'maj7', 'min7', 'min7b5', 'dim7', 'add9',
];

const ROOT_NAMES = ['C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B'] as const;
const ROOT_NAMES_FLAT = ['C', 'Db', 'D', 'Eb', 'E', 'F', 'Gb', 'G', 'Ab', 'A', 'Bb', 'B'] as const;

const ROOT_PC: Record<string, number> = {
  c: 0, 'c#': 1, db: 1, d: 2, 'd#': 3, eb: 3, e: 4, fb: 4,
  f: 5, 'f#': 6, gb: 6, g: 7, 'g#': 8, ab: 8, a: 9, 'a#': 10,
  bb: 10, b: 11, cb: 11, 'e#': 5, 'b#': 0,
};

/** Pitch classes sounded by the chord (set of 0..11). */
export function pitchClasses(chord: ChordSymbol): Set<number> {
  const root = ((chord.rootPc % 12) + 12) % 12;
  return new Set(CHORD_INTERVALS[chord.quality].map((i) => (root + i) % 12));
}

/** Actual chord-tone pitches within [lo, hi] MIDI range. */
export function chordTonePitches(chord: ChordSymbol, lo = 0, hi = 127): number[] {
  const pcs = pitchClasses(chord);
  const out: number[] = [];
  for (let p = Math.max(0, lo); p <= Math.min(127, hi); p++) {
    if (pcs.has(((p % 12) + 12) % 12)) out.push(p);
  }
  return out;
}

const SUFFIX_MAP: Record<string, ChordQuality> = {
  '': 'maj', maj: 'maj', m: 'min', min: 'min', '-': 'min',
  dim: 'dim', '°': 'dim', aug: 'aug', '+': 'aug',
  sus2: 'sus2', sus4: 'sus4', sus: 'sus4',
  '6': '6', maj6: '6', m6: 'm6', min6: 'm6',
  '7': '7', dom7: '7', maj7: 'maj7', m7: 'min7', min7: 'min7',
  'Δ7': 'maj7', M7: 'maj7', '△7': 'maj7',
  'm7b5': 'min7b5', min7b5: 'min7b5', 'ø7': 'min7b5', 'ø': 'min7b5',
  dim7: 'dim7', '°7': 'dim7',
  add9: 'add9',
};

const QUALITY_SUFFIX: Record<ChordQuality, string> = {
  maj: '', min: 'm', dim: 'dim', aug: 'aug', sus2: 'sus2', sus4: 'sus4',
  '6': '6', m6: 'm6', '7': '7', maj7: 'maj7', min7: 'm7',
  min7b5: 'm7b5', dim7: 'dim7', add9: 'add9',
};

/**
 * Parse "C", "F#m", "Bb7", "Gsus4", "Edim", "Cm7b5", "Amaj7" → symbol.
 * Strict: returns null for anything unrecognized (no guessing).
 */
export function parseChordSymbol(text: string): ChordSymbol | null {
  const t = text.trim().replace(/[♯]/g, '#').replace(/[♭]/g, 'b');
  const m = /^([A-Ga-g])(#|b)?(.*)$/.exec(t);
  if (!m) return null;
  const rootName = (m[1].toLowerCase() + (m[2] ?? '')).toLowerCase();
  const rootPc = ROOT_PC[rootName];
  if (rootPc === undefined) return null;
  const suffix = (m[3] ?? '').trim();
  const quality = SUFFIX_MAP[suffix] ?? SUFFIX_MAP[suffix.toLowerCase()];
  if (quality === undefined) return null;
  const flat = (m[2] === 'b') || undefined;
  return flat ? { rootPc, quality, flat: true } : { rootPc, quality };
}

/** Canonical display: "C", "F#m", "Bb7", "Cm7b5" — preserves the parsed
 *  enharmonic spelling (flat flag) and defaults to sharps otherwise. */
export function formatChordSymbol(chord: ChordSymbol): string {
  const pc = ((chord.rootPc % 12) + 12) % 12;
  const root = (chord.flat ? ROOT_NAMES_FLAT : ROOT_NAMES)[pc];
  return `${root}${QUALITY_SUFFIX[chord.quality]}`;
}

/** Named scale pitch-class sets for constraint/fallback use. */
export const SCALES: Record<string, readonly number[]> = {
  major: [0, 2, 4, 5, 7, 9, 11],
  minor: [0, 2, 3, 5, 7, 8, 10],
  dorian: [0, 2, 3, 5, 7, 9, 10],
  mixolydian: [0, 2, 4, 5, 7, 9, 10],
  locrian: [0, 1, 3, 5, 6, 8, 10],
  pentMaj: [0, 2, 4, 7, 9],
  pentMin: [0, 3, 5, 7, 10],
  wholeTone: [0, 2, 4, 6, 8, 10],
  dimWH: [0, 2, 3, 5, 6, 8, 9, 11],
  chromatic: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
};

/** The scale a chord implies for 'scale'-mode snapping — a documented
 *  convention, not a universal truth (real harmonic context wins in F3). */
export function impliedScale(chord: ChordSymbol): number[] {
  switch (chord.quality) {
    case 'maj':
    case 'maj7':
    case '6':
    case 'add9':
      return [...SCALES.major];
    case 'min':
    case 'm6':
      return [...SCALES.minor];
    case 'min7':
      return [...SCALES.dorian];
    case 'min7b5':
      return [...SCALES.locrian];
    case 'dim':
    case 'dim7':
      return [...SCALES.dimWH];
    case 'aug':
      return [...SCALES.wholeTone];
    case '7':
    case 'sus2':
    case 'sus4':
      return [...SCALES.mixolydian];
  }
}

export function scaleSet(rootPc: number, scale: readonly number[]): Set<number> {
  const r = ((rootPc % 12) + 12) % 12;
  return new Set(scale.map((i) => (r + i) % 12));
}
