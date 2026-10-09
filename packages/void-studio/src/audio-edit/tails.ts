// Render-tail policy descriptors (W19 / EDIT-01..03 render semantics;
// T74 side). A render tail decides how much audio past `rangeEnd` is
// emitted so releases/reverb rings out — it is explicit and bounded,
// never renderer-implied (void-export TailPolicy contract).
//
// The wire shape mirrors `crates/void-export/src/spec.rs::TailPolicy`:
//   { mode: 'none' } | { mode: 'milliseconds', ms } | { mode: 'ticks', ticks }
// plus a tail-fade spec (fade applied across the tail region). The fade
// fields are a NEEDS entry — spec.rs has no tail fade today, so the
// descriptor carries them as proposed params the render surface consumes
// when the field lands (same honesty pattern as fades/notes).

import { i64str, parseI64 } from 'void-client';
import type { FadeShape } from '../takes/types';

/** Tail policy mirroring void-export's serde shape. */
export type RenderTailPolicy =
  | { mode: 'none' }
  | { mode: 'milliseconds'; ms: number }
  | { mode: 'ticks'; ticks: string };

/** Tail bound — 60 s, matching spec.rs's "tail ms exceeds 60 s bound". */
export const TAIL_MAX_MS = 60_000;

/**
 * Render-tail descriptor: policy + the fade applied across the emitted
 * tail. `fadeTicks` must not exceed the tail length.
 */
export interface RenderTailSpec {
  tail: RenderTailPolicy;
  /** Optional fade across the tail end. Undefined = no tail fade. */
  fadeOut?: { shape: FadeShape; lengthTicks: string };
}

export function renderTail(spec: RenderTailSpec): RenderTailSpec {
  if (spec.tail.mode === 'milliseconds') {
    if (!Number.isFinite(spec.tail.ms) || spec.tail.ms < 0 || spec.tail.ms > TAIL_MAX_MS) {
      throw new Error(`tail ms out of bound [0, ${TAIL_MAX_MS}]`);
    }
  }
  if (spec.tail.mode === 'ticks') {
    if (parseI64(spec.tail.ticks) < 0n) {
      throw new Error('tail ticks must be >= 0');
    }
  }
  if (spec.fadeOut) {
    if (spec.tail.mode === 'none') {
      throw new Error('tail fade needs a non-none tail');
    }
    const fl = parseI64(spec.fadeOut.lengthTicks);
    if (fl <= 0n) {
      throw new Error('tail fade length must be > 0');
    }
    if (spec.tail.mode === 'ticks' && fl > parseI64(spec.tail.ticks)) {
      throw new Error('tail fade exceeds tail length');
    }
  }
  return spec;
}

/**
 * Flatten to the parameters a render op carries. `tailTicks` is the
 * musical-length form when the policy is tick-based (ms policies are
 * carried as ms — the engine converts at the export tempo map, same as
 * void-export FramePlan does).
 */
export interface RenderTailParams {
  tailPolicy: RenderTailPolicy;
  /** Proposed wire fields (NEEDS): fade shape + length for the tail end. */
  tailFadeShape?: FadeShape;
  tailFadeTicks?: string;
}

export function renderTailParams(spec: RenderTailSpec): RenderTailParams {
  const s = renderTail(spec);
  return {
    tailPolicy: s.tail,
    tailFadeShape: s.fadeOut?.shape,
    tailFadeTicks: s.fadeOut ? i64str(parseI64(s.fadeOut.lengthTicks)) : undefined,
  };
}
