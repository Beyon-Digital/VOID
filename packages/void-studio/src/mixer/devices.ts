// Device-chain items from the PLUGIN_LIST read view (S04, UI-T35).
//
// PLUGIN_LIST carries the engine's loaded plugin instances. The summary
// shape is engine-defined JSON; this module narrows it defensively and
// surfaces only what was actually reported — a malformed item drops out
// rather than fabricating a device slot.

import type { ReadItem } from 'void-client';

/** One loaded device (plugin instance) in a channel's insert chain. */
export interface DeviceItem {
  /** plugin_instance_id — required; ops (param/editor/remove) target it. */
  instanceId: string;
  trackId: string;
  name: string;
  format: string;
  /** Insert position when the engine reports one. */
  slot?: number;
  /** Powered/enabled state; undefined = not reported (assume powered). */
  powered?: boolean;
  /**
   * Honest degraded state: the engine flagged this instance missing,
   * quarantined or failed — the slot keeps its identity but shows the
   * reason instead of pretending the device renders.
   */
  failed?: string;
}

/**
 * Narrow a PLUGIN_LIST summary_json into a DeviceItem. Field names follow
 * the engine's reported keys with common aliases tolerated; unknown or
 * absent flag fields stay undefined (never assumed unavailable).
 */
export function deviceFromSummary(objectId: string, summaryJson: string): DeviceItem | null {
  try {
    const s = JSON.parse(summaryJson) as Record<string, unknown>;
    const instanceId =
      (typeof s.plugin_instance_id === 'string' && s.plugin_instance_id) ||
      (typeof s.instance_id === 'string' && s.instance_id) ||
      (typeof s.id === 'string' && s.id) ||
      objectId;
    if (!instanceId) return null;
    const trackId =
      (typeof s.track_id === 'string' && s.track_id) ||
      (typeof s.track === 'string' && s.track) ||
      '';
    const name =
      (typeof s.name === 'string' && s.name) ||
      (typeof s.plugin_uid === 'string' && s.plugin_uid) ||
      '(unnamed device)';
    const format = typeof s.format === 'string' && s.format ? s.format : 'plugin';
    const slot =
      typeof s.slot === 'number' && Number.isInteger(s.slot) && s.slot >= 0 ? s.slot : undefined;
    const powered =
      typeof s.enabled === 'boolean'
        ? s.enabled
        : typeof s.powered === 'boolean'
          ? s.powered
          : typeof s.bypassed === 'boolean'
            ? !s.bypassed
            : undefined;
    const failed =
      (typeof s.error === 'string' && s.error) ||
      (typeof s.failure === 'string' && s.failure) ||
      (s.quarantined === true ? 'quarantined' : undefined) ||
      (s.missing === true ? 'missing' : undefined) ||
      (s.unavailable === true ? 'unavailable' : undefined);
    return { instanceId, trackId, name, format, slot, powered, failed };
  } catch {
    return null;
  }
}

/**
 * All device items in a PLUGIN_LIST page for one track, ordered by slot
 * (unslotted devices keep page order at the end).
 */
export function deviceChainFromItems(items: ReadItem[], trackId: string): DeviceItem[] {
  const out: DeviceItem[] = [];
  for (const it of items) {
    const d = deviceFromSummary(it.object_id, it.summary_json);
    if (!d) continue;
    // Items that name a different track are not this chain's; items that
    // name no track stay (the view may already be track-scoped).
    if (d.trackId && d.trackId !== trackId) continue;
    out.push(d);
  }
  return out.sort((a, b) => (a.slot ?? Number.MAX_SAFE_INTEGER) - (b.slot ?? Number.MAX_SAFE_INTEGER));
}

/** Every device item in a page regardless of track (S11 preflight). */
export function devicesFromItems(items: ReadItem[]): DeviceItem[] {
  const out: DeviceItem[] = [];
  for (const it of items) {
    const d = deviceFromSummary(it.object_id, it.summary_json);
    if (d) out.push(d);
  }
  return out;
}
