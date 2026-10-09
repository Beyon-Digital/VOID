// Spatial view-state types (W26: T92/T93 display side).
//
// Mirrors crates/void-spatial for the UI: declared output paths and
// their last-known gate verdicts. This store NEVER upgrades a verdict
// — it displays what the gate decided, verbatim, including every
// typed unavailable reason. Stereo-as-proxy is impossible here by
// construction: the only "available" state carries a validator id.

/** Layout ids — mirror void-spatial `SpatialLayout`. */
export type SpatialLayoutId =
  | 'mono'
  | 'stereo'
  | 'quad'
  | 'surround-5-1'
  | 'surround-7-1'
  | 'surround-7-1-2'
  | 'surround-7-1-4'
  | 'surround-9-1-4'
  | 'surround-22-2';

export const SPATIAL_LAYOUT_CHANNELS: Record<SpatialLayoutId, number> = {
  mono: 1,
  stereo: 2,
  quad: 4,
  'surround-5-1': 6,
  'surround-7-1': 8,
  'surround-7-1-2': 10,
  'surround-7-1-4': 12,
  'surround-9-1-4': 14,
  'surround-22-2': 24,
};

/** Output path kinds — mirror `SpatialOutputPath`. */
export type SpatialPathKind =
  | 'channel-bus'
  | 'binaural'
  | 'atmos-adm-bwf'
  | 'atmos-mp4'
  | 'bed-plus-objects';

/** Typed unavailable reasons — mirror `SpatialUnavailable`. */
export type SpatialUnavailableReason =
  | { kind: 'licensed-renderer-required'; detail: string }
  | { kind: 'approved-validator-required'; detail: string }
  | { kind: 'external-validator-required'; detail: string }
  | { kind: 'head-tracking-unavailable'; detail: string }
  | { kind: 'invalid-container'; detail: string }
  | { kind: 'monitor-unreachable'; detail: string }
  | { kind: 'monitoring-invalid'; detail: string }
  | { kind: 'no-monitoring-config' }
  | { kind: 'platform-unsupported'; detail: string };

/** Verdict as reported by the coordinator gate — verbatim display. */
export type GateVerdictView =
  | {
      kind: 'available';
      validatorId: string;
      validatorKind: 'self-check' | 'approved' | 'external';
      monitorRoute: 'direct' | 'fallback';
    }
  | { kind: 'unavailable'; reason: SpatialUnavailableReason };

/** A declared output path + its last-known verdict. */
export interface SpatialOutputView {
  outputId: string;
  pathKind: SpatialPathKind;
  layout: SpatialLayoutId;
  label: string;
  verdict: GateVerdictView | null;
  verdictAtMs: number | null;
}

/** Monitoring config draft — the editable half (validates locally
 *  to the same bounds the crate enforces; the coordinator re-checks). */
export interface MonitoringDraft {
  speakerSet: SpatialLayoutId;
  /** speaker id → trim dB (finite, ±20). */
  levelCalibrationDb: Record<string, number>;
  fallback:
    | { kind: 'declared-downmix'; to: SpatialLayoutId }
    | { kind: 'binaural' }
    | { kind: 'none' };
}

export const MAX_TRIM_DB = 20;

/** Structural validation of a monitoring draft — mirrors the crate's
 *  `MonitoringConfig::validate` bounds (the coordinator still owns the
 *  verdict; a clean draft is necessary, not sufficient). */
export function validateMonitoringDraft(d: MonitoringDraft): string[] {
  const errors: string[] = [];
  const setChannels = SPATIAL_LAYOUT_CHANNELS[d.speakerSet];
  const trimSpeakers = Object.keys(d.levelCalibrationDb);
  if (trimSpeakers.length !== setChannels) {
    errors.push(
      `speaker set ${d.speakerSet} has ${setChannels} speakers but ${trimSpeakers.length} trims declared`,
    );
  }
  for (const [sp, trim] of Object.entries(d.levelCalibrationDb)) {
    if (!Number.isFinite(trim)) errors.push(`trim for ${sp} is not finite`);
    else if (Math.abs(trim) > MAX_TRIM_DB)
      errors.push(`trim for ${sp} is ${trim} dB — bound is ±${MAX_TRIM_DB}`);
  }
  if (d.fallback.kind === 'declared-downmix') {
    if (d.fallback.to === d.speakerSet)
      errors.push('declared-downmix fallback targets the same layout');
    const to = SPATIAL_LAYOUT_CHANNELS[d.fallback.to];
    if (to > setChannels)
      errors.push('declared-downmix fallback targets a WIDER layout — a downmix folds down');
  }
  return errors;
}
