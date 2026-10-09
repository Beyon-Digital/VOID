import { describe, expect, it } from 'vitest';
import {
  cancelModuleOp,
  createExtensionsStore,
  installModuleOp,
  loadModuleOp,
  parseCatalog,
  parseGateRecord,
  reloadModuleOp,
  revokeModuleOp,
  rowKey,
  unloadModuleOp,
  visibleRows,
  type ModuleCatalogDto,
  type ModuleRowDto,
} from './index';

function row(over: Partial<ModuleRowDto> = {}): ModuleRowDto {
  return {
    module_id: 'saw-gen',
    name: 'Saw Gen',
    version: '1.0.0',
    status: 'installed',
    capabilities: ['params', 'state', 'render-off'],
    placement: 'offline_render',
    wasm_sha256: 'a'.repeat(64),
    manifest_sha256: 'b'.repeat(64),
    limits: {
      max_memory_bytes: '16777216',
      max_fuel: '10000000',
      deadline_ms: '2000',
      max_state_bytes: '65536',
      max_out_bytes: '4194304',
      max_params: '8',
    },
    live_version: null,
    revoked_reason: null,
    ...over,
  };
}

function catalog(...rows: ModuleRowDto[]): ModuleCatalogDto {
  return { schema: 'void-wasm/catalog@1', modules: rows };
}

describe('extension ops', () => {
  it('builds validated op payloads', () => {
    expect(loadModuleOp('saw-gen').version).toBeNull();
    expect(loadModuleOp('saw-gen', '1.0.0').version).toBe('1.0.0');
    expect(reloadModuleOp('saw-gen').to_version).toBeNull();
    const r = revokeModuleOp('saw-gen', 'cve-2026-1', '1.0.0');
    expect(r).toEqual({
      op: 'revoke_module',
      module_id: 'saw-gen',
      version: '1.0.0',
      reason: 'cve-2026-1',
    });
    expect(unloadModuleOp('m')).toEqual({ op: 'unload_module', module_id: 'm' });
    expect(cancelModuleOp('m')).toEqual({ op: 'cancel_module_call', module_id: 'm' });
    const inst = installModuleOp('{"manifest_version":1}', 'blob:abc', 'c'.repeat(64));
    expect(inst.wasm_sha256).toBe('c'.repeat(64));
  });

  it('rejects malformed inputs', () => {
    expect(() => loadModuleOp('bad id')).toThrow(/bad module_id/);
    expect(() => loadModuleOp('m', 'v1')).toThrow(/bad version/);
    expect(() => revokeModuleOp('m', '')).toThrow(/reason/);
    expect(() => installModuleOp('', 'r', 'a'.repeat(64))).toThrow(/manifest/);
    expect(() => installModuleOp('{}', 'r', 'deadbeef')).toThrow(/sha256/);
  });
});

describe('catalog parsing (string-int64 boundary)', () => {
  it('accepts a real catalog', () => {
    const c = parseCatalog(catalog(row(), row({ version: '2.0.0', status: 'live', live_version: '2.0.0' })));
    expect(c.modules).toHaveLength(2);
  });

  it('rejects numeric (non-string) limits — the DTO contract', () => {
    const bad = row();
    (bad.limits as unknown as Record<string, unknown>).max_fuel = 10000000;
    expect(() => parseCatalog(catalog(bad))).toThrow(/string-int64|bad limits/);
  });

  it('rejects inconsistent rows', () => {
    expect(() =>
      parseCatalog(catalog(row({ status: 'live', live_version: null }))),
    ).toThrow(/live row/);
    expect(() =>
      parseCatalog(catalog(row({ status: 'revoked' }))),
    ).toThrow(/revoked row/);
    expect(() => parseCatalog({ schema: 'x', modules: [] })).toThrow(/catalog/);
  });
});

describe('gate records', () => {
  it('parses gated and qualified records', () => {
    const g = parseGateRecord({
      integration: 'ara',
      status: { status: 'gated', reason: 'licence' },
    });
    expect(g.status.status).toBe('gated');
    const q = parseGateRecord({
      integration: 'plugin_export',
      status: { status: 'qualified', sdk: 'vst3-3.7', rights_ref: 'lic/1', commit: 'abc' },
    });
    expect(q.status.status).toBe('qualified');
  });

  it('rejects evidence-free qualification and unknown integrations', () => {
    expect(() =>
      parseGateRecord({ integration: 'ara', status: { status: 'qualified', sdk: 'x' } }),
    ).toThrow(/evidence/);
    expect(() =>
      parseGateRecord({ integration: 'hardware_synth', status: { status: 'gated' } }),
    ).toThrow(/bad gate/);
  });
});

describe('extensions view store (view-state only)', () => {
  it('ingests catalogs, filters, selects; drops stale selection', () => {
    const s = createExtensionsStore();
    const n = s.getState().actions.ingestCatalog(
      catalog(
        row(),
        row({ version: '2.0.0', status: 'live', live_version: '2.0.0' }),
        row({ module_id: 'arp', version: '0.3.0', capabilities: ['midi-transform'], placement: 'midi_transform' }),
      ),
    );
    expect(n).toBe(3);
    s.getState().actions.select(rowKey('saw-gen', '2.0.0'));
    s.getState().actions.setStatusFilter('live');
    let vis = visibleRows(s.getState());
    expect(vis.map((r) => r.version)).toEqual(['2.0.0']);
    s.getState().actions.setStatusFilter('all');
    s.getState().actions.setCapabilityFilter('midi-transform');
    vis = visibleRows(s.getState());
    expect(vis.map((r) => r.module_id)).toEqual(['arp']);

    // re-ingest without the selected row → selection cleared
    s.getState().actions.setCapabilityFilter(null);
    s.getState().actions.ingestCatalog(catalog(row({ module_id: 'arp' })));
    expect(s.getState().selected).toBeNull();
    expect(() => s.getState().actions.select('nope@1.0.0')).toThrow(/unknown/);
  });

  it('holds only view-state keys — no instances, no blobs, no truth', () => {
    const s = createExtensionsStore();
    const state = s.getState();
    const allowed = new Set([
      'rows', 'order', 'selected', 'statusFilter',
      'capabilityFilter', 'gates', 'busy', 'actions',
    ]);
    for (const k of Object.keys(state)) {
      expect(allowed.has(k), `non-view key "${k}"`).toBe(true);
    }
    // Rows are DTOs — the store must never carry wasm bytes/instances.
    expect(JSON.stringify(state.rows)).not.toContain('wasm_bytes');
  });
});
