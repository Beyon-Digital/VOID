// Exchange DTOs (W20) — mirror `crates/void-exchange` serde shapes.
// String-int64 rule applies to every i64/u64 field (ticks, bytes).

export type ExchangeDirection = 'import' | 'export';
export type LossKind = 'dropped' | 'approximated';

export interface LossEntry {
  /** Stable element id or structural path — never a raw fs path. */
  element: string;
  /** Which aspect was lost: "sends", "warps", "note.releaseVelocity". */
  aspect: string;
  kind: LossKind;
  reason: string;
}

export interface LossReport {
  direction?: ExchangeDirection;
  /** Canonically sorted at emit. */
  entries: LossEntry[];
}

export function parseLossReport(value: unknown): LossReport | null {
  if (typeof value !== 'object' || value === null) return null;
  const v = value as { direction?: unknown; entries?: unknown };
  if (!Array.isArray(v.entries)) return null;
  const entries: LossEntry[] = [];
  for (const e of v.entries) {
    if (typeof e !== 'object' || e === null) return null;
    const x = e as Record<string, unknown>;
    if (
      typeof x.element !== 'string' ||
      typeof x.aspect !== 'string' ||
      typeof x.reason !== 'string' ||
      (x.kind !== 'dropped' && x.kind !== 'approximated')
    ) {
      return null;
    }
    entries.push({
      element: x.element,
      aspect: x.aspect,
      kind: x.kind,
      reason: x.reason,
    });
  }
  return {
    direction: v.direction === 'import' || v.direction === 'export' ? v.direction : undefined,
    entries,
  };
}

export function lossSummary(report: LossReport): { dropped: number; approximated: number } {
  let dropped = 0;
  let approximated = 0;
  for (const e of report.entries) {
    if (e.kind === 'dropped') dropped += 1;
    else approximated += 1;
  }
  return { dropped, approximated };
}

export type PluginFormat =
  | 'vst3'
  | 'vst2'
  | 'clap'
  | 'au'
  | 'aax'
  | 'builtin'
  | (string & {});

export interface PluginDescriptorDto {
  format: PluginFormat;
  pluginUid: string;
  name: string;
  vendor?: string;
  version?: string;
  deviceRole?: string;
  arch: string[];
  stateFormatVersion?: string;
}

export type CompatibilityStatus =
  | 'compatible'
  | 'notObservedOnPlatform'
  | 'archMismatch'
  | 'quarantined'
  | 'formatNotHostable'
  | 'unknown';

export interface CompatibilityReportDto {
  status: CompatibilityStatus;
  reasons: string[];
}

/** Registry entry DTO (void-plugin-registry/1). */
export interface RegistryEntryDto {
  descriptor: PluginDescriptorDto;
  hostedOn: string[];
  quarantined: boolean;
  quarantineReason?: string;
  notes?: string;
}

/** PLUGIN_LIST item's exchange-relevant surface (fbs PluginInfo subset). */
export interface PluginListItemLike {
  plugin_instance_id: string;
  format: string;
  plugin_uid: string;
  name: string;
}

/** Preserved-state record for a missing plugin (void-missing-plugins/1). */
export interface PreservedPluginDto {
  instanceId: string;
  trackId: string;
  slot: number;
  descriptor: PluginDescriptorDto;
  stateHex?: string;
  stateSha256?: string;
  stateFormatVersion?: string;
  resolved: boolean;
}

/** What a track row should display for one plugin slot. */
export interface PluginBadge {
  instanceId: string;
  /** Short badge label, e.g. "missing", "quarantined", "unavailable". */
  label: 'missing' | 'quarantined' | 'unavailable' | 'unverified';
  /** User-readable detail (never raw paths). */
  detail: string;
}
