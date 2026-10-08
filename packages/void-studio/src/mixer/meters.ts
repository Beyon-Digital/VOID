// Meter strips fed by MeterFrame telemetry (W10, T41).
//
// Honesty contract: a strip shows what the engine actually reported —
// the last MeterFrame's peaks/RMS and clip flag. When no frame has
// arrived (engine silent, track new, transport stopped, or frames older
// than the freshness window) the strip reports `idle`, NOT a decaying
// fake envelope. Silence == silence; clipping == clipping. T41 requires
// "meters reflect native samples ... no simulated levels".

import type { MeterFrame } from 'void-client';
import { MIN_DB } from './ops';

/** A meter frame older than this is stale — engine is not feeding it. */
export const METER_FRESH_MS = 250;

/** linear 0..1 → dB with floor (mirrors gainToDb convention). */
export function levelToDb(linear: number): number {
  if (!Number.isFinite(linear) || linear < 0) return MIN_DB;
  if (linear === 0) return MIN_DB;
  return Math.max(MIN_DB, 20 * Math.log10(linear));
}

export interface MeterStripState {
  /** 'idle' when no fresh frame exists — render an empty strip, not 0dB. */
  status: 'live' | 'idle';
  peakDbL: number;
  peakDbR: number;
  rmsDbL: number;
  rmsDbR: number;
  clipped: boolean;
  /** Echo of the last frame's sequence — lets callers detect frozen meters. */
  sequence?: string;
}

/** Pure evaluation of a strip's display state from its last frame. */
export function meterStrip(
  frame: MeterFrame | undefined,
  nowMs: number,
  frameAtMs?: number,
): MeterStripState {
  const fresh =
    frame !== undefined &&
    frameAtMs !== undefined &&
    nowMs - frameAtMs <= METER_FRESH_MS;
  if (!frame || !fresh) {
    return {
      status: 'idle',
      peakDbL: MIN_DB,
      peakDbR: MIN_DB,
      rmsDbL: MIN_DB,
      rmsDbR: MIN_DB,
      clipped: false,
      sequence: frame?.sequence,
    };
  }
  return {
    status: 'live',
    peakDbL: levelToDb(frame.peak_l),
    peakDbR: levelToDb(frame.peak_r),
    rmsDbL: levelToDb(frame.rms_l),
    rmsDbR: levelToDb(frame.rms_r),
    clipped: frame.clipped,
    sequence: frame.sequence,
  };
}

/**
 * Arrival tracker: records when each track's frame arrived so
 * `meterStrip` can judge freshness. Pure Map — the studio store's
 * telemetry.meters already holds the last frame per track.
 */
export class MeterClock {
  private lastAt = new Map<string, number>();
  private now: () => number;

  constructor(now: () => number = () => Date.now()) {
    this.now = now;
  }

  /** Record a frame arrival; returns its receive time. */
  note(frame: MeterFrame): number {
    const t = this.now();
    this.lastAt.set(frame.track_id, t);
    return t;
  }

  at(trackId: string): number | undefined {
    return this.lastAt.get(trackId);
  }

  strip(frame: MeterFrame | undefined): MeterStripState {
    return frame
      ? meterStrip(frame, this.now(), this.lastAt.get(frame.track_id))
      : meterStrip(undefined, 0);
  }

  clear(): void {
    this.lastAt.clear();
  }
}
