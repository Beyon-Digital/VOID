// A/B toggle and loudness-aware compare (W18; MIX-08, T70).
//
// An A/B snapshot is a set of parameter values. Flipping applies the
// other side as real parameter ops under ONE transaction id — undo
// rewinds the flip in one step via `undoTransaction`.
//
// Loudness-aware compare: the caller supplies a measured gain offset in
// dB (`matchGainDb`) applied to gain-kind snapshots when switching to B.
// The offset is never invented — without a measured value the A/B is a
// plain level-blind compare and `gainMatchApplied` stays false.

import type { PersistentOp } from 'void-client';
import { dbToGain, setTrackGainOp, setTrackMuteOp, setTrackPanOp } from '../mixer/ops';
import type { ParamRef } from './types';

/** One parameter value inside an A/B snapshot. */
export interface ParamSnapshot {
  ref: ParamRef;
  value: number;
}

export interface ABState {
  a: ParamSnapshot[];
  b: ParamSnapshot[];
  current: 'a' | 'b';
}

export function makeAB(a: ParamSnapshot[], b: ParamSnapshot[]): ABState {
  return { a, b, current: 'a' };
}

/** The side a flip would land on. */
export function toggleTarget(ab: ABState): 'a' | 'b' {
  return ab.current === 'a' ? 'b' : 'a';
}

export function snapshotsOf(ab: ABState, side: 'a' | 'b'): ParamSnapshot[] {
  return side === 'a' ? ab.a : ab.b;
}

/** Wire op for one snapshot entry — every ref kind maps to a real op. */
export function opForSnapshot(s: ParamSnapshot): PersistentOp {
  switch (s.ref.kind) {
    case 'trackGain':
      return setTrackGainOp(s.ref.trackId, s.value);
    case 'trackPan':
      return setTrackPanOp(s.ref.trackId, s.value);
    case 'trackMute':
      return setTrackMuteOp(s.ref.trackId, s.value !== 0);
    case 'pluginParam': {
      if (!s.ref.pluginInstanceId || !s.ref.paramId) {
        throw new Error(
          'pluginParam snapshot needs pluginInstanceId and paramId',
        );
      }
      return {
        SetPluginParamOp: {
          plugin_instance_id: s.ref.pluginInstanceId,
          param_id: s.ref.paramId,
          value: s.value,
        },
      };
    }
  }
}

/**
 * Gain-matched copy of a snapshot list: gain-kind refs get their linear
 * gain scaled by `matchDb` (converted through the mixer dB helpers).
 * Other params pass through unchanged.
 */
export function gainMatched(
  snapshots: ParamSnapshot[],
  matchDb: number,
): ParamSnapshot[] {
  const scale = dbToGain(matchDb);
  return snapshots.map((s) =>
    s.ref.kind === 'trackGain' ? { ...s, value: s.value * scale } : s,
  );
}

/**
 * Ops that flip the A/B state to `side`, optionally gain-matched by a
 * caller-measured dB offset (applied to the LOUDER side — for a plain
 * compare pass no matchDb). Returns the ops; sending them under one
 * transactionId makes the flip a single undo step.
 */
export function abFlipOps(
  ab: ABState,
  side: 'a' | 'b',
  opts?: { matchDb?: number },
): PersistentOp[] {
  let snaps = snapshotsOf(ab, side);
  if (opts?.matchDb !== undefined && Number.isFinite(opts.matchDb)) {
    snaps = gainMatched(snaps, opts.matchDb);
  }
  return snaps.map(opForSnapshot);
}
