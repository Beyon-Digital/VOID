// Step-pattern model + deterministic renderer (PAT-01, T59).
//
// A StepPattern is UI-side authoring data — the same ownership class as
// a gesture raw capture. Rendering produces RawGestureNote events, so
// materializing a pattern into a clip rides the exact same commit path
// as a drawn gesture (ordinary InsertNoteOps, one transaction, normal
// undo). No pattern ever mutates the engine directly.
//
// DETERMINISM (PAT-01 acceptance): probability rolls are keyed on
// (pattern.seed, rowId, globalStepIndex) — never on row position — so
// reloading a seeded pattern reproduces its output exactly, and editing
// or reordering rows cannot shift another row's output.

import { parseI64 } from 'void-client';
import type { PersistentOp } from 'void-client';
import type { RawGestureNote } from '../gestures/types';
import { clampGestureVelocity } from '../gestures/types';
import { clampGesturePitch } from '../gestures/types';
import { mulDivRound } from '../gestures/transform';

export const PATTERN_PPM = 1_000_000;

/** One grid cell. All fields have honest defaults — a bare `on` step
 *  plays a full-velocity 90%-gate hit that always fires. */
export interface PatternStep {
  on: boolean;
  /** 1..127 */
  velocity: number;
  /** Note length as a fraction of the (sub-)step, 0..1e6. */
  gatePpm: number;
  /** Ratchet: the step splits into this many equal hits, 1..8. */
  repeats: number;
  /** Fire probability 0..1e6 — deterministic against the seed. */
  probabilityPpm: number;
  /** Extend the previous emitted note in this row instead of a new hit. */
  tie: boolean;
}

/** One sequencer row — an independent-length lane (polyrhythm legal). */
export interface PatternRow {
  rowId: string;
  /** Drum pitch / note the row triggers. */
  pitch: number;
  label?: string;
  steps: PatternStep[];
}

export interface StepPattern {
  patternId: string;
  name: string;
  /** Duration of ONE global step in ticks. */
  stepTicks: string;
  /** Deterministic playback seed (string; hashed per roll). */
  seed: string;
  rows: PatternRow[];
}

export function defaultStep(): PatternStep {
  return {
    on: false,
    velocity: 100,
    gatePpm: 900_000,
    repeats: 1,
    probabilityPpm: PATTERN_PPM,
    tie: false,
  };
}

export function makeRow(rowId: string, pitch: number, steps: number, label?: string): PatternRow {
  return {
    rowId,
    pitch: clampGesturePitch(pitch),
    label,
    steps: Array.from({ length: Math.max(0, steps) }, () => defaultStep()),
  };
}

/** Global cycle length: stepTicks × the LONGEST row; shorter rows wrap
 * at their own period (polyrhythm is real, not flattened). */
export function patternCycleTicks(p: StepPattern): bigint {
  const step = parseI64(p.stepTicks);
  const len = p.rows.reduce((m, r) => Math.max(m, r.steps.length), 0);
  return step * BigInt(len);
}

// -- deterministic roll -------------------------------------------------------

function fnv1a(s: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < s.length; i++) {
    h ^= s.charCodeAt(i);
    h = Math.imul(h, 0x01000193);
  }
  return h >>> 0;
}

/** mulberry32 keyed on (seed, rowId, globalStep). Result in [0,1e6). */
export function stepRoll(seed: string, rowId: string, globalStep: number): number {
  let a =
    (fnv1a(seed) ^ Math.imul(fnv1a(rowId) + 1, 0x9e3779b9) ^
      Math.imul(globalStep + 1, 0x85ebca6b)) >>>
    0;
  a = (a + 0x6d2b79f5) | 0;
  let t = a;
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  return Math.floor((((t ^ (t >>> 14)) >>> 0) / 4294967296) * PATTERN_PPM);
}

// -- editing (pure, row-scoped) ------------------------------------------------

export function setStep(
  p: StepPattern,
  rowId: string,
  index: number,
  patch: Partial<PatternStep>,
): StepPattern {
  return {
    ...p,
    rows: p.rows.map((r) => {
      if (r.rowId !== rowId || index < 0 || index >= r.steps.length) return r;
      const steps = r.steps.slice();
      const prev = steps[index];
      const next = { ...prev, ...patch };
      next.velocity = clampGestureVelocity(next.velocity);
      next.gatePpm = Math.min(PATTERN_PPM, Math.max(0, Math.round(next.gatePpm)));
      next.repeats = Math.min(8, Math.max(1, Math.round(next.repeats)));
      next.probabilityPpm = Math.min(
        PATTERN_PPM,
        Math.max(0, Math.round(next.probabilityPpm)),
      );
      steps[index] = next;
      return { ...r, steps };
    }),
  };
}

export function toggleStep(p: StepPattern, rowId: string, index: number): StepPattern {
  const row = p.rows.find((r) => r.rowId === rowId);
  const cur = row?.steps[index];
  if (!cur) return p;
  return setStep(p, rowId, index, { on: !cur.on });
}

export function setRowLength(p: StepPattern, rowId: string, steps: number): StepPattern {
  const n = Math.max(0, Math.round(steps));
  return {
    ...p,
    rows: p.rows.map((r) => {
      if (r.rowId !== rowId) return r;
      if (n === r.steps.length) return r;
      const next =
        n < r.steps.length
          ? r.steps.slice(0, n)
          : r.steps.concat(
              Array.from({ length: n - r.steps.length }, () => defaultStep()),
            );
      return { ...r, steps: next };
    }),
  };
}

// -- rendering ----------------------------------------------------------------

export interface RenderOptions {
  /** Whole-pattern cycles to render (≥1). */
  cycles?: number;
  /** Onset offset for global step 0. */
  originTicks?: string;
}

/**
 * Pattern → raw note events. Deterministic: identical (pattern, cycles,
 * origin) inputs produce identical output — the seed is part of the
 * pattern, not ambient state.
 */
export function renderPattern(
  p: StepPattern,
  opts: RenderOptions = {},
): RawGestureNote[] {
  const step = parseI64(p.stepTicks);
  if (step <= 0n) return [];
  const cycles = Math.max(1, Math.round(opts.cycles ?? 1));
  const origin = parseI64(opts.originTicks ?? '0');
  const maxLen = p.rows.reduce((m, r) => Math.max(m, r.steps.length), 0);
  const notes: RawGestureNote[] = [];
  // Per-row "last note" for tie handling (ties extend within one render).
  const lastInRow = new Map<string, RawGestureNote>();
  for (const row of p.rows) {
    if (row.steps.length === 0) continue;
    const pitch = clampGesturePitch(row.pitch);
    for (let g = 0; g < cycles * maxLen; g++) {
      const st = row.steps[g % row.steps.length];
      if (!st.on) continue;
      const prob = Math.min(PATTERN_PPM, Math.max(0, st.probabilityPpm));
      if (prob < PATTERN_PPM && stepRoll(p.seed, row.rowId, g) >= prob) continue;
      const stepStart = origin + BigInt(g) * step;
      if (st.tie && lastInRow.has(row.rowId)) {
        // Extend the previous emitted note by this whole step span.
        const last = lastInRow.get(row.rowId)!;
        const end = stepStart + step;
        last.lengthTicks = (end - parseI64(last.rawStartTicks)).toString(10);
        continue;
      }
      const repeats = Math.min(8, Math.max(1, st.repeats));
      const sub = step / BigInt(repeats);
      const gate = mulDivRound(sub, BigInt(st.gatePpm), BigInt(PATTERN_PPM));
      for (let k = 0; k < repeats; k++) {
        const n: RawGestureNote = {
          index: notes.length,
          lengthTicks: (gate > 0n ? gate : 1n).toString(10),
          rawStartTicks: (stepStart + BigInt(k) * sub).toString(10),
          rawPitch: pitch,
          rawVelocity: clampGestureVelocity(st.velocity),
        };
        notes.push(n);
        lastInRow.set(row.rowId, n);
      }
    }
  }
  // Emit in musical order; index lanes follow onset order.
  return notes
    .sort((a, b) => {
      const d = parseI64(a.rawStartTicks) - parseI64(b.rawStartTicks);
      return d < 0n ? -1 : d > 0n ? 1 : 0;
    })
    .map((n, i) => ({ ...n, index: i }));
}

/**
 * Materialize a pattern into a clip: InsertNoteOps sharing one
 * transaction id — the same commit discipline gestures use (T59).
 */
export function patternToInsertOps(
  clipId: string,
  p: StepPattern,
  mint: () => string,
  opts: RenderOptions = {},
): { transactionId: string; noteIds: string[]; ops: PersistentOp[] } {
  const notes = renderPattern(p, opts);
  const noteIds = notes.map(() => mint());
  return {
    transactionId: mint(),
    noteIds,
    ops: notes.map((n, i) => ({
      InsertNoteOp: {
        clip_id: clipId,
        note_id: noteIds[i],
        pitch: n.rawPitch,
        velocity: n.rawVelocity,
        start_ticks: n.rawStartTicks,
        length_ticks: n.lengthTicks,
      },
    })),
  };
}
