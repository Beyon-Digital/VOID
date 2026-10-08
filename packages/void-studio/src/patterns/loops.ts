// Loop browser (SND-05, T59): built-in deterministic pattern templates.
//
// Templates are authored StepPatterns — real note data, not names on a
// card. "Preview in project time" is honest in the tick domain: musical
// ticks are tempo-agnostic (the engine's tempo map owns wall time), so
// the rendered note list IS the project-time result; `durationMs` gives
// the UI the wall-clock span at the project bpm for display.
//
// Audition never modifies the project (SND-05 acceptance): preview is a
// pure render — nothing is sent anywhere until the caller materializes
// through the shared commit path.

import { parseI64 } from 'void-client';
import type { RawGestureNote } from '../gestures/types';
import { ticksToMs } from '../gestures/rhythm';
import {
  defaultStep,
  makeRow,
  renderPattern,
  patternCycleTicks,
  type PatternStep,
  type StepPattern,
} from './stepPattern';

export interface LoopTemplate {
  id: string;
  name: string;
  /** Authored reference tempo (display + tag filtering only). */
  bpm: number;
  meter: string;
  tags: string[];
  pattern: StepPattern;
}

/** Compact step authoring: 'x' = on, '.' = off; '|' is visual padding. */
function steps(spec: string, patch: Partial<PatternStep> = {}): PatternStep[] {
  return [...spec.replace(/\|/g, '')].map((ch) => ({
    ...defaultStep(),
    ...patch,
    on: ch === 'x' || ch === 'X',
    velocity: ch === 'X' ? 127 : (patch.velocity ?? 100),
  }));
}

function template(
  id: string,
  name: string,
  bpm: number,
  tags: string[],
  stepTicks: string,
  rows: { pitch: number; label: string; spec: string; patch?: Partial<PatternStep> }[],
): LoopTemplate {
  return {
    id,
    name,
    bpm,
    meter: '4/4',
    tags,
    pattern: {
      patternId: `${id}-pattern`,
      name,
      stepTicks,
      seed: `loop:${id}`,
      rows: rows.map((r) => ({
        ...makeRow(`${id}-${r.label.toLowerCase().replace(/\W+/g, '-')}`, r.pitch, 0, r.label),
        steps: steps(r.spec, r.patch),
      })),
    },
  };
}

/** The shipped set — deterministic, offline, no external content. */
export function builtinLoops(): LoopTemplate[] {
  const q = '240000'; // 1/16
  return [
    template('four-floor', 'Four on the floor', 124, ['house', 'kick', 'driving'], q, [
      { pitch: 36, label: 'Kick', spec: 'x...x...x...x...' },
      { pitch: 42, label: 'Hats', spec: '..x...x...x...x.', patch: { velocity: 70 } },
      { pitch: 39, label: 'Clap', spec: '....x.......x...' },
    ]),
    template('backbeat', 'Backbeat', 96, ['rock', 'snare', 'groove'], q, [
      { pitch: 36, label: 'Kick', spec: 'x.....x...x.....' },
      { pitch: 38, label: 'Snare', spec: '....x.......x...' },
      { pitch: 42, label: 'Hats', spec: 'x.x.x.x.x.x.x.x.', patch: { velocity: 64 } },
    ]),
    template('halftime', 'Halftime', 140, ['hip-hop', 'sparse', 'heavy'], q, [
      { pitch: 36, label: 'Kick', spec: 'x.........x.....' },
      { pitch: 38, label: 'Snare', spec: '........x.......' },
      { pitch: 42, label: 'Hats', spec: 'x.xxx.xxx.xx.xx.', patch: { velocity: 58 } },
    ]),
    template('son-clave', 'Son clave', 100, ['latin', 'clave', 'percussion'], q, [
      { pitch: 75, label: 'Clave', spec: 'x..x..x...x.x...' },
      { pitch: 56, label: 'Cowbell', spec: 'x.x.x.x.x.x.x.x.', patch: { velocity: 55 } },
      { pitch: 36, label: 'Kick', spec: 'x.......x...x...' },
    ]),
    template('drive-16', 'Drive 16ths', 128, ['techno', 'hats', 'busy'], q, [
      { pitch: 36, label: 'Kick', spec: 'x...x...x...x...' },
      { pitch: 42, label: 'Hats', spec: 'xxxxxxxxxxxxxxxx', patch: { velocity: 60 } },
      { pitch: 46, label: 'Open hat', spec: '..............x.', patch: { gatePpm: 400_000 } },
    ]),
    template('swing-ride', 'Swing ride', 112, ['jazz', 'ride', 'swing'], q, [
      { pitch: 51, label: 'Ride', spec: 'x..x.xx..x.x.x..' },
      { pitch: 36, label: 'Kick', spec: 'x.....x.........' },
      { pitch: 38, label: 'Snare', spec: '....x.......x...', patch: { velocity: 72 } },
    ]),
  ];
}

export interface LoopQuery {
  /** Substring match on name/tags (case-insensitive). */
  text?: string;
  /** Require ALL of these tags. */
  tags?: string[];
  minBpm?: number;
  maxBpm?: number;
}

export function searchLoops(templates: LoopTemplate[], q: LoopQuery): LoopTemplate[] {
  const text = q.text?.trim().toLowerCase();
  return templates.filter((t) => {
    if (text) {
      const hay = `${t.name} ${t.tags.join(' ')}`.toLowerCase();
      if (!hay.includes(text)) return false;
    }
    if (q.tags?.length && !q.tags.every((tag) => t.tags.includes(tag))) return false;
    if (q.minBpm !== undefined && t.bpm < q.minBpm) return false;
    if (q.maxBpm !== undefined && t.bpm > q.maxBpm) return false;
    return true;
  });
}

export interface LoopPreview {
  /** Deterministic rendered notes (transient preview — never persisted). */
  notes: RawGestureNote[];
  /** One template cycle in ticks. */
  cycleTicks: string;
  /** Rendered span in ticks (cycle × cycles). */
  durationTicks: string;
  /** Wall-clock span at project bpm — display only. */
  durationMs: number;
}

/** Audition a template at the project tempo — pure render, zero sends. */
export function previewLoop(
  t: LoopTemplate,
  opts: { bpm: number; cycles?: number; originTicks?: string },
): LoopPreview {
  const cycles = Math.max(1, Math.round(opts.cycles ?? 1));
  const notes = renderPattern(t.pattern, {
    cycles,
    originTicks: opts.originTicks ?? '0',
  });
  const cycleTicks = patternCycleTicks(t.pattern);
  return {
    notes,
    cycleTicks: cycleTicks.toString(10),
    durationTicks: (cycleTicks * BigInt(cycles)).toString(10),
    durationMs: ticksToMs(cycleTicks * BigInt(cycles), opts.bpm),
  };
}
