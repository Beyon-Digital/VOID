// PLUGIN_LIST wire items (protocol major.1, EngineSession.cpp).
//
// The engine emits one item per hosted slot:
//   {"instanceId","name"?,"status":"ACTIVE"|"BYPASSED"|"MISSING"}
// and, for slots whose plugin could not be resolved at all:
//   {"instanceId","status":"MISSING","reason":"..."}
//
// This is the authoritative plugin-slot surface on the wire: the slot,
// its load state, and the engine's reason string. There is no separate
// registry or preserved-state view — a MISSING row itself is the proof
// that the engine kept the slot (and its saved state/routing) instead
// of substituting or dropping it.

import type { ReadItem } from 'void-client';

export type PluginSlotStatus = 'ACTIVE' | 'BYPASSED' | 'MISSING' | (string & {});

export interface PluginSlotItem {
  instanceId: string;
  name?: string;
  status: PluginSlotStatus;
  /** Engine-supplied reason for MISSING rows; never a raw path. */
  reason?: string;
}

/** Defensive projection of one PLUGIN_LIST item. Malformed rows → null. */
export function parsePluginSlotItem(item: ReadItem): PluginSlotItem | null {
  let v: unknown;
  try {
    v = JSON.parse(item.summary_json);
  } catch {
    return null;
  }
  if (typeof v !== 'object' || v === null) return null;
  const o = v as Record<string, unknown>;
  if (typeof o.instanceId !== 'string' || o.instanceId.length === 0) return null;
  if (typeof o.status !== 'string' || o.status.length === 0) return null;
  return {
    instanceId: o.instanceId,
    name: typeof o.name === 'string' && o.name.length > 0 ? o.name : undefined,
    status: o.status,
    reason: typeof o.reason === 'string' && o.reason.length > 0 ? o.reason : undefined,
  };
}

export function parsePluginSlots(items: ReadItem[]): PluginSlotItem[] {
  const out: PluginSlotItem[] = [];
  for (const it of items) {
    const p = parsePluginSlotItem(it);
    if (p) out.push(p);
  }
  return out;
}

export function isRecoverable(slot: PluginSlotItem): boolean {
  return slot.status === 'MISSING' || slot.status === 'BYPASSED';
}
