// S24 score screen — pure model helpers (no React, node-testable).
// Same split as other studio lanes: every function here is deterministic
// and covered by model.test.ts / void-studio's derive.test.ts.

import type { ReadItem } from 'void-client';
import {
  clipStartMap,
  layoutScore,
  notesToAbsolute,
  type ClipView,
  type DisplayQuantize,
  type NoteView,
  type ScoreLayout,
  type ScoreMeter,
  type ScoreNote,
  type SpellingPref,
} from 'void-studio';

/** One row in the PARTS rail — a TRACK_LIST item. */
export interface ScorePart {
  trackId: string;
  name: string;
  kind?: string;
  muted: boolean;
}

export function partsFromTrackItems(items: readonly ReadItem[]): ScorePart[] {
  return items.map((it) => {
    try {
      const v = JSON.parse(it.summary_json) as Record<string, unknown>;
      return {
        trackId: String(v.track_id ?? v.id ?? it.object_id),
        name: String(v.name ?? v.track_id ?? it.object_id),
        kind: typeof v.kind === 'string' ? v.kind : undefined,
        muted: v.muted === true,
      };
    } catch {
      return {
        trackId: it.object_id,
        name: it.object_id,
        muted: false,
      };
    }
  });
}

/** Only MIDI clips carry notes a score can show. */
export function isMidiClip(c: ClipView): boolean {
  return c.kind === 'MIDI' || c.kind === 'midi';
}

/** Engine notes → absolute score notes for one part. */
export function deriveScoreNotes(
  notes: readonly NoteView[],
  clips: readonly ClipView[],
): { notes: ScoreNote[]; dropped: number } {
  return notesToAbsolute(notes, clipStartMap(clips));
}

export function deriveLayout(
  scoreNotes: readonly ScoreNote[],
  meter: ScoreMeter,
  quantize: DisplayQuantize,
  spelling: SpellingPref,
): ScoreLayout {
  return layoutScore(scoreNotes, { meter, quantize, spelling });
}

// ---------------------------------------------------------------------------
// Staff geometry — treble staff, step units of half a line space.
// staffStep 0 = bottom line (E4); odd steps sit in spaces.
// ---------------------------------------------------------------------------

export const STAFF_STEP_PX = 5;
export const STAFF_LINE_COUNT = 5;
export const STAFF_HEIGHT = (STAFF_LINE_COUNT - 1) * 2 * STAFF_STEP_PX; // 40
export const SYSTEM_HEIGHT = 112;
export const MEASURE_WIDTH = 216;
export const MEASURES_PER_SYSTEM = 4;
export const CLEF_W = 56;
export const MEASURE_NUM_H = 14;
/** Ledger lines trigger beyond the outer lines (step <0 or >8). */
export const STAFF_TOP_STEP = 8;

/** y of a notehead center within a staff block of `staffTop` px origin. */
export function noteHeadY(staffStep: number): number {
  // step 0 (E4) is the BOTTOM line: y = STAFF_HEIGHT; each step up is
  // STAFF_STEP_PX higher on the page (lower y).
  return STAFF_HEIGHT - staffStep * STAFF_STEP_PX;
}

/** y of staff line i (0 = bottom). */
export function staffLineY(i: number): number {
  return STAFF_HEIGHT - i * 2 * STAFF_STEP_PX;
}

/** Ledger-line positions (step values) a note needs drawn. */
export function ledgerSteps(staffStep: number): number[] {
  const out: number[] = [];
  if (staffStep < 0) {
    for (let s = -2; s >= staffStep + (staffStep % 2 === 0 ? 0 : 1); s -= 2) {
      out.push(s);
    }
  } else if (staffStep > STAFF_TOP_STEP) {
    for (
      let s = STAFF_TOP_STEP + 2;
      s <= staffStep - (staffStep % 2 === 0 ? 0 : 1);
      s += 2
    ) {
      out.push(s);
    }
  }
  return out;
}

/** Staff step → MIDI pitch for insert (diatonic staff position, sharps). */
export function midiForStaffStep(staffStep: number): number {
  const PC = [0, 2, 4, 5, 7, 9, 11];
  const idx = staffStep + (4 * 7 + 2); // diatonic index of E4 base
  const octave = Math.floor(idx / 7);
  const stepIdx = idx - octave * 7;
  return octave * 12 + PC[stepIdx] + 12;
}

/** x position inside a measure for a display offset. */
export function offsetX(
  offsetTicks: bigint,
  measureLenTicks: bigint,
): number {
  if (measureLenTicks <= 0n) return 0;
  return CLEF_W + (Number(offsetTicks) / Number(measureLenTicks)) * (MEASURE_WIDTH - CLEF_W - 8);
}

// ---------------------------------------------------------------------------
// Inspector summary — the "selected passage" rows.
// ---------------------------------------------------------------------------

export interface PassageInfo {
  /** e.g. "1" | "2–5" | null when nothing is selected. */
  bars: string | null;
  count: number;
}

export function passageInfo(
  selectedNoteIds: readonly string[],
  layout: ScoreLayout,
): PassageInfo {
  if (selectedNoteIds.length === 0) return { bars: null, count: 0 };
  const ids = new Set(selectedNoteIds);
  let min = Number.POSITIVE_INFINITY;
  let max = -1;
  let count = 0;
  for (const m of layout.measures) {
    for (const s of m.segments) {
      if (ids.has(s.noteId)) {
        count++;
        if (m.index < min) min = m.index;
        if (m.index > max) max = m.index;
      }
    }
  }
  if (count === 0) return { bars: null, count: 0 };
  return {
    bars: min === max ? `${min + 1}` : `${min + 1}–${max + 1}`,
    count,
  };
}

/** Download filename for the MusicXML export. */
export function exportFileName(partName: string): string {
  const slug = partName
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '');
  return `${slug || 'part'}.musicxml`;
}

// ---------------------------------------------------------------------------
// Honest capability copy (no invented flags — literals come from
// void-studio/src/notation/capabilities.ts; strings here only describe).
// ---------------------------------------------------------------------------

export const SCORE_GAPS = {
  engraving:
    'Verovio engraving is not wired — layout uses the internal renderer (NEEDS-notation-01).',
  scoreOps:
    'Score-op wire is absent — edits use the shipped note ops; crate plans stay unstaged (NEEDS-notation-02).',
  exportView:
    'Exports the derived view: real ticks + engine note ids; score-model anchors/voices beyond it are out of scope for this wire.',
  playback:
    'Notation playback is not wired (NEEDS-notation-05).',
} as const;
