// Missing-plugin badges (W20, T76).
//
// Derives per-slot badge state from PLUGIN_LIST items + registry records.
// Honest semantics only:
//   - a slot whose (format, pluginUid) has no registry record is
//     "missing" when a preserved-state record exists for it, else
//     "unverified" — never assumed compatible;
//   - a quarantined record badges "quarantined" regardless of presence;
//   - AAX and AU-off-platform badge "unavailable" (FormatNotHostable);
//   - arch mismatch badges "unavailable".
// Badge text carries no paths — details are user-readable reason strings
// taken from the registry/preserved records.

import {
  CompatibilityReportDto,
  PluginBadge,
  PluginDescriptorDto,
  PluginListItemLike,
  PreservedPluginDto,
  RegistryEntryDto,
} from './types';

export interface BadgeInputs {
  /** Items from a PLUGIN_LIST view (the slots a project wants). */
  slots: PluginListItemLike[];
  /** Registry records keyed `format/pluginUid` (lowercase). */
  registry: RegistryEntryDto[];
  /** Preserved-state records (missing-plugin store rows). */
  preserved: PreservedPluginDto[];
  /** Host platform id, e.g. "linux-x86_64". */
  platform: string;
}

export function pluginKey(format: string, uid: string): string {
  return `${format.toLowerCase()}/${uid}`;
}

function compatOf(
  entry: RegistryEntryDto | undefined,
  platform: string,
): CompatibilityReportDto | null {
  if (!entry) return null;
  const d = entry.descriptor;
  const reasons: string[] = [];
  if (d.format === 'aax') {
    return { status: 'formatNotHostable', reasons: ['AAX is Avid-only — VOID cannot host it'] };
  }
  if (d.format === 'au' && !platform.startsWith('macos')) {
    return {
      status: 'formatNotHostable',
      reasons: [`Audio Unit requires macOS (host: ${platform})`],
    };
  }
  if (entry.quarantined) {
    return {
      status: 'quarantined',
      reasons: [entry.quarantineReason ?? 'plugin quarantined'],
    };
  }
  if (d.arch.length > 0) {
    const hostArch = platform.split('-').pop() ?? '';
    if (!d.arch.includes(hostArch) && !d.arch.includes('universal')) {
      return {
        status: 'archMismatch',
        reasons: [`no ${hostArch} build (has: ${d.arch.join(', ') || 'unknown'})`],
      };
    }
  }
  if (entry.hostedOn.includes(platform)) {
    return { status: 'compatible', reasons };
  }
  return {
    status: 'notObservedOnPlatform',
    reasons: [`recorded on: ${entry.hostedOn.join(', ') || 'none'}`],
  };
}

/** One badge per slot, keyed by instance id. */
export function deriveBadges(inputs: BadgeInputs): PluginBadge[] {
  const regByKey = new Map(inputs.registry.map((r) => [
    pluginKey(r.descriptor.format, r.descriptor.pluginUid),
    r,
  ]));
  const preservedByInstance = new Map(
    inputs.preserved.filter((p) => !p.resolved).map((p) => [p.instanceId, p]),
  );
  const badges: PluginBadge[] = [];
  for (const slot of inputs.slots) {
    const key = pluginKey(slot.format, slot.plugin_uid);
    const entry = regByKey.get(key);
    const preserved = preservedByInstance.get(slot.plugin_instance_id);
    const compat = compatOf(entry, inputs.platform);

    // Format-level unhostability needs no registry record — AAX is
    // Avid-only and AU is macOS-only regardless of what was scanned.
    const fmt = slot.format.toLowerCase();
    if (fmt === 'aax') {
      badges.push({
        instanceId: slot.plugin_instance_id,
        label: 'unavailable',
        detail: 'AAX is Avid-only — VOID cannot host it',
      });
      continue;
    }
    if (fmt === 'au' && !inputs.platform.startsWith('macos')) {
      badges.push({
        instanceId: slot.plugin_instance_id,
        label: 'unavailable',
        detail: `Audio Unit requires macOS (host: ${inputs.platform})`,
      });
      continue;
    }

    if (preserved && !entry) {
      badges.push({
        instanceId: slot.plugin_instance_id,
        label: 'missing',
        detail: `plugin not installed — state preserved (${slot.format}/${slot.plugin_uid})`,
      });
      continue;
    }
    if (!compat) {
      badges.push({
        instanceId: slot.plugin_instance_id,
        label: 'unverified',
        detail: `no registry record for ${slot.format}/${slot.plugin_uid}`,
      });
      continue;
    }
    switch (compat.status) {
      case 'quarantined':
        badges.push({
          instanceId: slot.plugin_instance_id,
          label: 'quarantined',
          detail: compat.reasons[0] ?? 'plugin quarantined',
        });
        break;
      case 'formatNotHostable':
      case 'archMismatch':
        badges.push({
          instanceId: slot.plugin_instance_id,
          label: 'unavailable',
          detail: compat.reasons[0] ?? 'not hostable on this platform',
        });
        break;
      case 'notObservedOnPlatform':
        if (preserved) {
          badges.push({
            instanceId: slot.plugin_instance_id,
            label: 'missing',
            detail: `not observed on ${inputs.platform} — state preserved`,
          });
        } else {
          badges.push({
            instanceId: slot.plugin_instance_id,
            label: 'unverified',
            detail: compat.reasons[0] ?? 'not observed on this platform',
          });
        }
        break;
      case 'unknown':
      case 'compatible':
      default:
        break;
    }
  }
  return badges;
}

/** Descriptors → registry entries (what the coordinator writes after a
 *  scan or import). Pure derivation — the coordinator owns persistence. */
export function registryEntriesFromDescriptors(
  descriptors: PluginDescriptorDto[],
  platform: string,
): RegistryEntryDto[] {
  return descriptors.map((descriptor) => ({
    descriptor,
    hostedOn: [platform],
    quarantined: false,
  }));
}
