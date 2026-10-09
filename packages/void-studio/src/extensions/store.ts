// Extensions view store (W28) — the extensions panel's UI state:
// catalog rows as the coordinator reported them, the selected module,
// row filters, and the SDK gate ledger. View-state only — the live
// registry, instances, and module state live in void-wasm behind the
// coordinator; this store never fabricates them.

import { createStore, StoreApi } from 'zustand/vanilla';
import {
  parseCatalog,
  parseGateRecord,
  type GateRecordDto,
  type ModuleCapability,
  type ModuleRowDto,
  type ModuleRowStatus,
} from './types';

export function rowKey(moduleId: string, version: string): string {
  return `${moduleId}@${version}`;
}

export interface ExtensionsViewState {
  /** Catalog rows keyed `${module_id}@${version}` — coordinator view. */
  rows: Record<string, ModuleRowDto>;
  order: string[];
  /** Selected row for the detail panel. */
  selected: string | null;
  statusFilter: 'all' | ModuleRowStatus;
  capabilityFilter: ModuleCapability | null;
  /** SDK/rights gate ledger (ara, plugin_export). */
  gates: Record<string, GateRecordDto>;
  busy: boolean;
}

export interface ExtensionsActions {
  ingestCatalog(payload: unknown): number;
  ingestGates(payload: unknown): number;
  select(key: string | null): void;
  setStatusFilter(f: 'all' | ModuleRowStatus): void;
  setCapabilityFilter(c: ModuleCapability | null): void;
  setBusy(b: boolean): void;
}

export type ExtensionsStore = StoreApi<
  ExtensionsViewState & { actions: ExtensionsActions }
>;

export function createExtensionsStore(): ExtensionsStore {
  return createStore<ExtensionsViewState & { actions: ExtensionsActions }>()(
    (set, get) => ({
      rows: {},
      order: [],
      selected: null,
      statusFilter: 'all',
      capabilityFilter: null,
      gates: {},
      busy: false,
      actions: {
        ingestCatalog(payload) {
          const cat = parseCatalog(payload);
          const rows: Record<string, ModuleRowDto> = {};
          const order: string[] = [];
          for (const m of cat.modules) {
            const k = rowKey(m.module_id, m.version);
            rows[k] = m;
            order.push(k);
          }
          // Keep the selection only if it still exists in the new
          // catalog — a revoked-then-purged row can't stay open.
          const sel = get().selected;
          set({
            rows,
            order,
            selected: sel && rows[sel] ? sel : null,
          });
          return order.length;
        },
        ingestGates(payload) {
          const list = payload as unknown[];
          if (!Array.isArray(list)) throw new Error('gate payload must be a list');
          const gates: Record<string, GateRecordDto> = {};
          for (const g of list) {
            const rec = parseGateRecord(g);
            gates[rec.integration] = rec;
          }
          set({ gates });
          return list.length;
        },
        select(key) {
          if (key !== null && !get().rows[key]) {
            throw new Error(`unknown module row ${key}`);
          }
          set({ selected: key });
        },
        setStatusFilter(f) {
          set({ statusFilter: f });
        },
        setCapabilityFilter(c) {
          set({ capabilityFilter: c });
        },
        setBusy(b) {
          set({ busy: b });
        },
      },
    }),
  );
}

/** Rows visible under the active filters — pure view projection. */
export function visibleRows(s: ExtensionsViewState): ModuleRowDto[] {
  return s.order
    .map((k) => s.rows[k])
    .filter(
      (r) =>
        (s.statusFilter === 'all' || r.status === s.statusFilter) &&
        (!s.capabilityFilter || r.capabilities.includes(s.capabilityFilter)),
    );
}
