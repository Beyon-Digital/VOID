// Derived score view (S24 / UI-T34) — engine notes → honest score layout.
//
// `crates/void-notation` is the score-model authority, but protocol major.1
// carries no score document (`SCORE_OPS_WIRE_AVAILABLE = false`), so the
// screen derives its notation from the SAME engine notes the piano roll
// edits (NOTE_RANGE). Element identity therefore IS the engine note id —
// selection maps bidirectionally with the roll, and edits go through the
// shipped InsertNoteOp/SetNoteOp/RemoveNoteOp path, never a second model.
//
// Display quantization is PRESENTATION ONLY: it shifts which staff cell /
// notated duration the renderer draws; the raw ticks that carry timing are
// never rewritten by it (the UI-T34 invariant the tests assert).

import { parseI64 } from 'void-client';
import type { ClipView } from '../timeline/geometry';
import type { NoteView } from '../piano-roll/notes';
import { TICKS_PER_QUARTER } from '../viewport';
import type { PitchDto, StepLetter } from './types';

// ---------------------------------------------------------------------------
// Source projection — NOTE_RANGE reports timeline-absolute ticks already
// (the roll draws them straight into a clip-window viewport); the clip map
// is only a membership check — notes naming a clip the part does not
// report are dropped, not guessed (same defensive rule as parseNoteItem).
// ---------------------------------------------------------------------------

export interface ScoreNote {
  /** Engine note id — the stable id the piano roll selects by. */
  noteId: string;
  clipId: string;
  /** MIDI pitch 0..127. */
  pitch: number;
  velocity: number;
  /** Timeline-absolute onset (as reported by NOTE_RANGE). */
  absStartTicks: bigint;
  durationTicks: bigint;
}

export function notesToAbsolute(
  notes: readonly NoteView[],
  clipStartById: ReadonlyMap<string, bigint>,
): { notes: ScoreNote[]; dropped: number } {
  const out: ScoreNote[] = [];
  let dropped = 0;
  for (const n of notes) {
    const clipStart = clipStartById.get(n.clipId);
    if (clipStart === undefined) {
      dropped++;
      continue;
    }
    out.push({
      noteId: n.noteId,
      clipId: n.clipId,
      pitch: n.pitch,
      velocity: n.velocity,
      absStartTicks: parseI64(n.startTicks),
      durationTicks: parseI64(n.lengthTicks),
    });
  }
  out.sort((a, b) =>
    a.absStartTicks < b.absStartTicks
      ? -1
      : a.absStartTicks > b.absStartTicks
        ? 1
        : a.pitch - b.pitch,
  );
  return { notes: out, dropped };
}

export function clipStartMap(clips: readonly ClipView[]): Map<string, bigint> {
  const m = new Map<string, bigint>();
  for (const c of clips) m.set(c.clipId, parseI64(c.startTicks));
  return m;
}

// ---------------------------------------------------------------------------
// Display meter + display quantization (view settings — never written back)
// ---------------------------------------------------------------------------

export interface ScoreMeter {
  beatsPerBar: number;
  beatUnit: 4 | 8;
}

export const SCORE_METERS: readonly ScoreMeter[] = [
  { beatsPerBar: 4, beatUnit: 4 },
  { beatsPerBar: 3, beatUnit: 4 },
  { beatsPerBar: 2, beatUnit: 4 },
  { beatsPerBar: 6, beatUnit: 8 },
  { beatsPerBar: 12, beatUnit: 8 },
];

export function meterLabel(m: ScoreMeter): string {
  return `${m.beatsPerBar}/${m.beatUnit}`;
}

/** Ticks per measure — a beat is a quarter (beatUnit 4) or an eighth (8). */
export function measureTicks(m: ScoreMeter): bigint {
  const perBeat = TICKS_PER_QUARTER / BigInt(m.beatUnit / 4);
  return perBeat * BigInt(m.beatsPerBar);
}

export type DisplayQuantize = '1/4' | '1/8' | '1/16' | '1/32' | 'off';

export const DISPLAY_QUANTIZE_OPTIONS: readonly DisplayQuantize[] = [
  '1/4',
  '1/8',
  '1/16',
  '1/32',
  'off',
];

/** Grid size for a quantize choice; null means "show raw performance". */
export function quantizeGridTicks(q: DisplayQuantize): bigint | null {
  switch (q) {
    case '1/4':
      return TICKS_PER_QUARTER;
    case '1/8':
      return TICKS_PER_QUARTER / 2n;
    case '1/16':
      return TICKS_PER_QUARTER / 4n;
    case '1/32':
      return TICKS_PER_QUARTER / 8n;
    case 'off':
      return null;
  }
}

/** Round to the nearest grid cell; ties away from zero. */
export function quantizeTicks(v: bigint, grid: bigint | null): bigint {
  if (grid === null || grid <= 0n) return v;
  const base = (v / grid) * grid;
  const rem = v - base;
  return rem * 2n >= grid ? base + grid : base;
}

// ---------------------------------------------------------------------------
// Pitch spelling — MIDI → diatonic step/alter/octave (sounding octave, the
// same convention `pitch.octave` holds in the crate's DTOs).
// ---------------------------------------------------------------------------

export type SpellingPref = 'sharps' | 'flats';

const SHARP_SPELL: readonly (readonly [StepLetter, number])[] = [
  ['c', 0], ['c', 1], ['d', 0], ['d', 1], ['e', 0], ['f', 0],
  ['f', 1], ['g', 0], ['g', 1], ['a', 0], ['a', 1], ['b', 0],
];
const FLAT_SPELL: readonly (readonly [StepLetter, number])[] = [
  ['c', 0], ['d', -1], ['d', 0], ['e', -1], ['e', 0], ['f', 0],
  ['g', -1], ['g', 0], ['a', -1], ['a', 0], ['b', -1], ['b', 0],
];

export function spellPitch(midiPitch: number, pref: SpellingPref = 'sharps'): PitchDto {
  const pc = ((midiPitch % 12) + 12) % 12;
  const [step, alter] = (pref === 'flats' ? FLAT_SPELL : SHARP_SPELL)[pc];
  return { step, alter, octave: Math.floor(midiPitch / 12) - 1 };
}

const STEP_INDEX: Record<StepLetter, number> = {
  c: 0, d: 1, e: 2, f: 3, g: 4, a: 5, b: 6,
};

/** Staff position in half-line steps above the treble bottom line (E4). */
export function staffStepIndex(p: PitchDto): number {
  return p.octave * 7 + STEP_INDEX[p.step] - (4 * 7 + 2);
}

// ---------------------------------------------------------------------------
// Layout — notes → per-measure segments. Durations that cross a barline are
// split into tied segments (same engraving rule the crate models).
// ---------------------------------------------------------------------------

export interface NoteSegment {
  noteId: string;
  clipId: string;
  measureIndex: number;
  /** Display offset inside the measure (quantized when a grid is set). */
  segOffsetTicks: bigint;
  segDurationTicks: bigint;
  /** Tied from a previous segment. */
  tieStop: boolean;
  /** Ties forward into a following segment. */
  tieStart: boolean;
  /** x-shift for second-interval collisions inside a chord (0 or 1). */
  xShift: 0 | 1;
  pitch: PitchDto;
  staffStep: number;
  velocity: number;
  /** The real performance values — display settings never touch these. */
  rawStartTicks: bigint;
  rawDurationTicks: bigint;
}

export interface MeasureLayout {
  index: number;
  segments: NoteSegment[];
  empty: boolean;
}

export interface ScoreLayout {
  measures: MeasureLayout[];
  measureLenTicks: bigint;
}

export function layoutScore(
  notes: readonly ScoreNote[],
  opts: { meter: ScoreMeter; quantize: DisplayQuantize; spelling?: SpellingPref },
): ScoreLayout {
  const pref = opts.spelling ?? 'sharps';
  const mLen = measureTicks(opts.meter);
  const grid = quantizeGridTicks(opts.quantize);

  const measureMap = new Map<number, NoteSegment[]>();
  let maxIndex = -1;
  const push = (mi: number, s: NoteSegment) => {
    const arr = measureMap.get(mi);
    if (arr) arr.push(s);
    else measureMap.set(mi, [s]);
    if (mi > maxIndex) maxIndex = mi;
  };

  for (const n of notes) {
    if (n.absStartTicks < 0n || n.durationTicks <= 0n) continue;
    const dispStart = quantizeTicks(n.absStartTicks, grid);
    let dispDur = quantizeTicks(n.durationTicks, grid);
    const minDur = grid ?? 1n;
    if (dispDur < minDur) dispDur = minDur;
    const pitched = spellPitch(n.pitch, pref);
    const staffStep = staffStepIndex(pitched);

    let cursor = dispStart;
    let remaining = dispDur;
    let first = true;
    while (remaining > 0n) {
      const mi = cursor / mLen;
      const within = cursor - mi * mLen;
      const room = mLen - within;
      const segDur = remaining < room ? remaining : room;
      push(Number(mi), {
        noteId: n.noteId,
        clipId: n.clipId,
        measureIndex: Number(mi),
        segOffsetTicks: within,
        segDurationTicks: segDur,
        tieStop: !first,
        tieStart: segDur < remaining,
        xShift: 0,
        pitch: pitched,
        staffStep,
        velocity: n.velocity,
        rawStartTicks: n.absStartTicks,
        rawDurationTicks: n.durationTicks,
      });
      cursor += segDur;
      remaining -= segDur;
      first = false;
    }
  }

  // Second-interval collision shifts: within each (measure, onset) chord
  // group, a notehead one staff step from its neighbour shifts right.
  for (const segs of measureMap.values()) {
    const byOnset = new Map<string, NoteSegment[]>();
    for (const s of segs) {
      const key = s.segOffsetTicks.toString(10);
      const g = byOnset.get(key);
      if (g) g.push(s);
      else byOnset.set(key, [s]);
    }
    for (const g of byOnset.values()) {
      if (g.length < 2) continue;
      g.sort((a, b) => b.staffStep - a.staffStep);
      for (let i = 1; i < g.length; i++) {
        const prev = g[i - 1];
        const cur = g[i];
        if (prev.staffStep - cur.staffStep <= 1 && prev.xShift === 0) {
          cur.xShift = 1;
        }
      }
    }
    segs.sort(
      (a, b) =>
        Number(a.segOffsetTicks - b.segOffsetTicks) || b.staffStep - a.staffStep,
    );
  }

  const measures: MeasureLayout[] = [];
  for (let i = 0; i <= maxIndex; i++) {
    const segments = measureMap.get(i) ?? [];
    measures.push({ index: i, segments, empty: segments.length === 0 });
  }
  return { measures, measureLenTicks: mLen };
}

// ---------------------------------------------------------------------------
// MusicXML 4.0 export (score-partwise). Emits the REAL performance: raw
// ticks become MusicXML durations verbatim; display quantization never
// enters the document. Overlapping onsets that are not chords are written
// to numbered voices with <backup> so the file stays rhythmically exact.
// ---------------------------------------------------------------------------

export interface MusicXmlPartInput {
  name: string;
  meter: ScoreMeter;
  notes: readonly ScoreNote[];
  spelling?: SpellingPref;
}

const STEP_UC: Record<StepLetter, string> = {
  c: 'C', d: 'D', e: 'E', f: 'F', g: 'G', a: 'A', b: 'B',
};

/** Standard note types a duration maps to exactly (base × dots). */
const NOTE_TYPES: readonly { name: string; ticks: bigint }[] = [
  { name: 'whole', ticks: TICKS_PER_QUARTER * 4n },
  { name: 'half', ticks: TICKS_PER_QUARTER * 2n },
  { name: 'quarter', ticks: TICKS_PER_QUARTER },
  { name: 'eighth', ticks: TICKS_PER_QUARTER / 2n },
  { name: '16th', ticks: TICKS_PER_QUARTER / 4n },
  { name: '32nd', ticks: TICKS_PER_QUARTER / 8n },
  { name: '64th', ticks: TICKS_PER_QUARTER / 16n },
];

function noteType(dur: bigint): { name: string; dots: number } | null {
  for (const t of NOTE_TYPES) {
    if (dur === t.ticks) return { name: t.name, dots: 0 };
    if (dur === t.ticks + t.ticks / 2n) return { name: t.name, dots: 1 };
    if (dur === t.ticks + t.ticks / 2n + t.ticks / 4n)
      return { name: t.name, dots: 2 };
  }
  return null;
}

function xmlEscape(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;');
}

interface XmlSeg {
  noteId: string;
  measureIndex: number;
  offset: bigint;
  dur: bigint;
  pitch: PitchDto;
  velocity: number;
  tieStart: boolean;
  tieStop: boolean;
}

function noteXml(s: XmlSeg, chord: boolean, voice: number): string {
  const t = noteType(s.dur);
  const dots = t ? '<dot/>'.repeat(t.dots) : '';
  const notations: string[] = [];
  if (s.tieStop) notations.push('<tied type="stop"/>');
  if (s.tieStart) notations.push('<tied type="start"/>');
  const tieEl = [
    s.tieStop ? '<tie type="stop"/>' : '',
    s.tieStart ? '<tie type="start"/>' : '',
  ].join('');
  const alter =
    s.pitch.alter !== 0 ? `<alter>${s.pitch.alter}</alter>` : '';
  return (
    `<note id="${xmlEscape(s.noteId)}" velocity="${s.velocity}">` +
    (chord ? '<chord/>' : '') +
    `<pitch><step>${STEP_UC[s.pitch.step]}</step>${alter}` +
    `<octave>${s.pitch.octave}</octave></pitch>` +
    `<duration>${s.dur.toString(10)}</duration>` +
    tieEl +
    `<voice>${voice}</voice><staff>1</staff>` +
    (t ? `<type>${t.name}</type>${dots}` : '') +
    (notations.length ? `<notations>${notations.join('')}</notations>` : '') +
    `</note>`
  );
}

/**
 * Serialize one derived part to MusicXML 4.0 score-partwise. Splits at
 * barlines into tied segments and packs simultaneous onsets into chords;
 * non-chord overlaps flow through voice 2+ with <backup> elements.
 */
export function exportMusicXml(part: MusicXmlPartInput): string {
  const pref = part.spelling ?? 'sharps';
  const mLen = measureTicks(part.meter);
  const divisions = TICKS_PER_QUARTER;

  // Split each note at barlines (raw ticks — performance data).
  const perMeasure = new Map<number, XmlSeg[]>();
  let maxIndex = -1;
  for (const n of part.notes) {
    if (n.absStartTicks < 0n || n.durationTicks <= 0n) continue;
    const pitched = spellPitch(n.pitch, pref);
    let cursor = n.absStartTicks;
    let remaining = n.durationTicks;
    let first = true;
    while (remaining > 0n) {
      const mi = cursor / mLen;
      const within = cursor - mi * mLen;
      const seg = remaining < mLen - within ? remaining : mLen - within;
      const arr = perMeasure.get(Number(mi));
      const el: XmlSeg = {
        noteId: n.noteId,
        measureIndex: Number(mi),
        offset: within,
        dur: seg,
        pitch: pitched,
        velocity: n.velocity,
        tieStart: seg < remaining,
        tieStop: !first,
      };
      if (arr) arr.push(el);
      else perMeasure.set(Number(mi), [el]);
      if (Number(mi) > maxIndex) maxIndex = Number(mi);
      cursor += seg;
      remaining -= seg;
      first = false;
    }
  }

  const measuresXml: string[] = [];
  for (let i = 0; i <= maxIndex; i++) {
    const segs = (perMeasure.get(i) ?? []).slice().sort(
      (a, b) => Number(a.offset - b.offset) || b.pitch.octave - a.pitch.octave,
    );
    let body = '';
    if (i === 0) {
      body +=
        '<attributes>' +
        `<divisions>${divisions.toString(10)}</divisions>` +
        '<key><fifths>0</fifths></key>' +
        `<time><beats>${part.meter.beatsPerBar}</beats>` +
        `<beat-type>${part.meter.beatUnit}</beat-type></time>` +
        '<staves>1</staves>' +
        '<clef number="1"><sign>G</sign><line>2</line></clef>' +
        '</attributes>';
    }
    if (segs.length === 0) {
      body +=
        '<note><rest measure="yes"/>' +
        `<duration>${mLen.toString(10)}</duration>` +
        '<voice>1</voice><staff>1</staff></note>';
    } else {
      // Voice assignment: group by onset; within an onset, notes sharing
      // a duration form a chord cluster in one voice, while same-onset /
      // different-duration notes are separate voices (engraving rule —
      // chord mates must share rhythmic values). A subgroup takes the
      // first voice whose cursor has already reached the onset.
      const voiceCursor: bigint[] = [];
      const voices = new Map<number, { seg: XmlSeg; chord: boolean }[]>();
      const byOnset = new Map<string, XmlSeg[]>();
      for (const s of segs) {
        const k = s.offset.toString(10);
        const g = byOnset.get(k);
        if (g) g.push(s);
        else byOnset.set(k, [s]);
      }
      const onsetGroups = [...byOnset.values()].sort(
        (a, b) => Number(a[0].offset - b[0].offset),
      );
      for (const group of onsetGroups) {
        const byDur = new Map<string, XmlSeg[]>();
        for (const s of group) {
          const k = s.dur.toString(10);
          const g = byDur.get(k);
          if (g) g.push(s);
          else byDur.set(k, [s]);
        }
        const clusters = [...byDur.values()].sort((a, b) =>
          Number(b[0].dur - a[0].dur),
        );
        for (const cluster of clusters) {
          const onset = cluster[0].offset;
          let v = 0;
          while (v < voiceCursor.length && onset < voiceCursor[v]) v++;
          if (v === voiceCursor.length) voiceCursor.push(0n);
          voiceCursor[v] = onset + cluster[0].dur;
          const arr = voices.get(v + 1) ?? [];
          cluster.forEach((s, i) => arr.push({ seg: s, chord: i > 0 }));
          voices.set(v + 1, arr);
        }
      }
      const voiceNums = [...voices.keys()].sort((a, b) => a - b);
      voiceNums.forEach((v, vi) => {
        if (vi > 0) {
          body += `<backup><duration>${mLen.toString(10)}</duration></backup>`;
        }
        const items = voices.get(v)!;
        // If the voice's last note ends before the barline, pad with a rest
        // so the voice measures exactly one bar.
        let cursor = 0n;
        for (const it of items) {
          const gap = it.seg.offset - cursor;
          if (gap > 0n) {
            body +=
              '<note><rest/>' +
              `<duration>${gap.toString(10)}</duration>` +
              `<voice>${v}</voice><staff>1</staff></note>`;
          }
          body += noteXml(it.seg, it.chord, v);
          cursor = it.seg.offset + it.seg.dur;
        }
        const tail = mLen - cursor;
        if (tail > 0n) {
          body +=
            '<note><rest/>' +
            `<duration>${tail.toString(10)}</duration>` +
            `<voice>${v}</voice><staff>1</staff></note>`;
        }
      });
    }
    measuresXml.push(`<measure number="${i + 1}">${body}</measure>`);
  }

  const partName = xmlEscape(part.name || 'Part');
  return (
    '<?xml version="1.0" encoding="UTF-8"?>\n' +
    '<!DOCTYPE score-partwise PUBLIC ' +
    '"-//Recordare//DTD MusicXML 4.0 Partwise//EN" ' +
    '"http://www.musicxml.org/dtds/partwise.dtd">\n' +
    '<score-partwise version="4.0">' +
    '<part-list><score-part id="P1">' +
    `<part-name>${partName}</part-name>` +
    '</score-part></part-list>' +
    `<part id="P1">${measuresXml.join('')}</part>` +
    '</score-partwise>'
  );
}
