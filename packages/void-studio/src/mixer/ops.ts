// Mixer op payload builders — exact PersistentOp shapes (W10, T41).
//
// The schema surface is small and honest: SetTrackGainOp / SetTrackPanOp /
// SetTrackMuteOp / SetTrackSoloOp exist; there is NO send/route op in
// void_control.fbs (bus tracks exist via AddTrackOp{kind:'BUS'} but sends
// cannot be wired yet — see routing.ts, which reports the gap instead of
// faking a routing model).

import type { PersistentOp } from 'void-client';

/** Reject non-finite values before they reach the wire. */
export function finiteOrThrow(what: string, v: number): number {
  if (!Number.isFinite(v)) {
    throw new Error(`${what}: value must be finite, got ${v}`);
  }
  return v;
}

/** Linear gain for SetTrackGainOp: finite and >= 0 (0 = silence). */
export function setTrackGainOp(trackId: string, gainLinear: number): PersistentOp {
  finiteOrThrow('gain_linear', gainLinear);
  if (gainLinear < 0) {
    throw new Error(`gain_linear must be >= 0, got ${gainLinear}`);
  }
  return { SetTrackGainOp: { track_id: trackId, gain_linear: gainLinear } };
}

/** Pan for SetTrackPanOp: finite within [-1, 1]. */
export function setTrackPanOp(trackId: string, pan: number): PersistentOp {
  finiteOrThrow('pan', pan);
  if (pan < -1 || pan > 1) {
    throw new Error(`pan must be in [-1, 1], got ${pan}`);
  }
  return { SetTrackPanOp: { track_id: trackId, pan } };
}

export function setTrackMuteOp(trackId: string, muted: boolean): PersistentOp {
  return { SetTrackMuteOp: { track_id: trackId, muted } };
}

export function setTrackSoloOp(trackId: string, soloed: boolean): PersistentOp {
  return { SetTrackSoloOp: { track_id: trackId, soloed } };
}

/** Insert-slot bypass: removing a plugin is the schema's bypass path
 *  (there is no SetPluginBypassOp variant; keeping it honest — callers
 *  should prefer disabling an insert, and this builder documents the gap
 *  instead of pretending a bypass op exists). */
export function removeInsertOp(pluginInstanceId: string): PersistentOp {
  return { RemovePluginOp: { plugin_instance_id: pluginInstanceId } };
}

// ---------------------------------------------------------------------------
// gain <-> dB helpers (display only — the wire carries linear gain)
// ---------------------------------------------------------------------------

export const MIN_DB = -80;

export function gainToDb(gainLinear: number): number {
  if (!Number.isFinite(gainLinear) || gainLinear < 0) {
    throw new Error(`gainToDb: invalid gain ${gainLinear}`);
  }
  if (gainLinear === 0) return MIN_DB; // -inf displayed at floor
  return Math.max(MIN_DB, 20 * Math.log10(gainLinear));
}

export function dbToGain(db: number): number {
  finiteOrThrow('db', db);
  if (db <= MIN_DB) return 0;
  return 10 ** (db / 20);
}
