// Extension module wire types (W28; T97–T98) — TS twins of
// crates/void-wasm serde shapes: snake_case fields, limit fields as
// decimal strings (string-int64 at the boundary, CONTRACTS §1).
// Modules are declarative: manifest + wasm, sha256-bound. Ambient
// powers are never grantable, so no ambient field is modelled here.

import { u64str } from 'void-client';

export type ModuleCapability =
  | 'params'
  | 'state'
  | 'render-off'
  | 'midi-transform'
  | 'no-fx'
  | 'generator';

/** Runtime placement — deliberately has no audio-path value. The
 * schema cannot express rt-audio residency; that is the guard. */
export type ModulePlacement =
  | 'param_surface'
  | 'offline_render'
  | 'midi_transform';

export type ModuleRowStatus = 'installed' | 'live' | 'revoked';

/** void-wasm DeclaredLimits twin — every field a decimal string. */
export interface ModuleLimitsDto {
  max_memory_bytes: string;
  max_fuel: string;
  deadline_ms: string;
  max_state_bytes: string;
  max_out_bytes: string;
  max_params: string;
}

export interface ModuleRowDto {
  module_id: string;
  name: string;
  version: string;
  /** Derived by the runtime: live / revoked / installed. */
  status: ModuleRowStatus;
  capabilities: ModuleCapability[];
  placement: ModulePlacement;
  wasm_sha256: string;
  manifest_sha256: string;
  limits: ModuleLimitsDto;
  /** Present when the row is live. */
  live_version: string | null;
  /** Present when revoked. */
  revoked_reason: string | null;
}

export interface ModuleCatalogDto {
  schema: 'void-wasm/catalog@1';
  modules: ModuleRowDto[];
}

export type GateIntegration = 'ara' | 'plugin_export';

/** crates/void-wasm GateStatus twin. */
export interface GateStatusDto {
  status: 'gated' | 'qualified';
  /** gated: the blocking item (SDK/rights unresolved). */
  reason?: string;
  /** qualified: recorded evidence. */
  sdk?: string;
  rights_ref?: string;
  commit?: string;
}

export interface GateRecordDto {
  integration: GateIntegration;
  status: GateStatusDto;
}

const ID_RE = /^[A-Za-z0-9._-]{1,128}$/;
const SHA_RE = /^[0-9a-f]{64}$/;
const VER_RE = /^\d+\.\d+\.\d+(-[A-Za-z0-9.-]+)?$/;

const CAPS: ReadonlySet<string> = new Set([
  'params',
  'state',
  'render-off',
  'midi-transform',
  'no-fx',
  'generator',
]);
const PLACEMENTS: ReadonlySet<string> = new Set([
  'param_surface',
  'offline_render',
  'midi_transform',
]);
const STATUSES: ReadonlySet<string> = new Set(['installed', 'live', 'revoked']);

export function validateModuleRow(r: ModuleRowDto): string | null {
  if (!ID_RE.test(r.module_id)) return 'bad module_id';
  if (!VER_RE.test(r.version)) return 'bad version';
  if (!STATUSES.has(r.status)) return 'bad status';
  if (!SHA_RE.test(r.wasm_sha256) || !SHA_RE.test(r.manifest_sha256))
    return 'bad sha256';
  if (r.capabilities.length === 0 || !r.capabilities.every((c) => CAPS.has(c)))
    return 'bad capabilities';
  if (!PLACEMENTS.has(r.placement)) return 'bad placement';
  const lim = r.limits as unknown as Record<string, unknown>;
  for (const k of [
    'max_memory_bytes',
    'max_fuel',
    'deadline_ms',
    'max_state_bytes',
    'max_out_bytes',
    'max_params',
  ]) {
    // Boundary contract: decimal STRINGS, never JS numbers.
    if (typeof lim[k] !== 'string') return 'bad limits (string-int64 required)';
    try {
      u64str(lim[k] as string);
    } catch {
      return 'bad limits (string-int64 required)';
    }
  }
  if (r.status === 'live' && r.live_version !== r.version)
    return 'live row must carry its own version';
  if (r.status === 'revoked' && !r.revoked_reason)
    return 'revoked row needs a reason';
  return null;
}

export function parseCatalog(payload: unknown): ModuleCatalogDto {
  const p = payload as ModuleCatalogDto;
  if (!p || p.schema !== 'void-wasm/catalog@1' || !Array.isArray(p.modules)) {
    throw new Error('bad module catalog payload');
  }
  for (const m of p.modules) {
    const err = validateModuleRow(m);
    if (err) throw new Error(`catalog row ${m?.module_id ?? '?'}: ${err}`);
  }
  return p;
}

export function parseGateRecord(payload: unknown): GateRecordDto {
  const g = payload as GateRecordDto;
  if (
    !g ||
    (g.integration !== 'ara' && g.integration !== 'plugin_export') ||
    !g.status ||
    (g.status.status !== 'gated' && g.status.status !== 'qualified')
  ) {
    throw new Error('bad gate record');
  }
  if (g.status.status === 'qualified') {
    if (!g.status.sdk || !g.status.rights_ref || !g.status.commit) {
      throw new Error('qualified gate missing evidence');
    }
  }
  return g;
}
