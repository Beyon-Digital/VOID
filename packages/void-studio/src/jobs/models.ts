// Model registry read view (W12, T51) — the MODEL_LIST payload shape.
// Mirrors crates/void-models::ModelDescriptor: the index is
// reconstructable, never the authority — the UI renders exactly what
// the read view reports and labels unavailable/degraded models so a
// user cannot pick one the runner would refuse.

import { u64str } from 'void-client';

export type ModelStatus = 'available' | 'degraded' | 'missing' | 'rejected';
export type ModelKind = 'symbolic' | 'audio' | 'visual';
export type RuntimeKind = 'argv' | 'internal';

export interface ModelRow {
  modelId: string;
  version: string;
  name: string;
  kind: ModelKind;
  runtime: RuntimeKind;
  status: ModelStatus;
  /** Budget defaults — decimal strings, rendered verbatim as badges. */
  cpuSeconds: string;
  memoryBytes: string;
  wallNs: string;
  cpuThreads: number;
  /** Artifact refs into the content-addressed store. */
  artifactCount: number;
  /** sha256 of the manifest the descriptor was verified against. */
  manifestSha256: string;
  license: string;
  /** Free-text source note ("bundled", "user install") — never a URL. */
  source: string;
  missingArtifacts: number;
  optionalArtifacts: number;
}

const MODEL_STATUSES: readonly string[] = ['available', 'degraded', 'missing', 'rejected'];
const MODEL_KINDS: readonly string[] = ['symbolic', 'audio', 'visual'];
const RUNTIME_KINDS: readonly string[] = ['argv', 'internal'];

function str(v: unknown): string | undefined {
  return typeof v === 'string' && v.length > 0 ? v : undefined;
}

/** Narrow a MODEL_LIST row — defensive; unknown fields are dropped. */
export function parseModelRow(payload: unknown): ModelRow | null {
  if (typeof payload !== 'object' || payload === null) return null;
  const p = payload as Record<string, unknown>;
  const modelId = str(p.modelId);
  const version = str(p.version) ?? '0';
  const status = str(p.status);
  if (!modelId || !status || !MODEL_STATUSES.includes(status)) return null;
  const budgets =
    typeof p.budgets === 'object' && p.budgets !== null
      ? (p.budgets as Record<string, unknown>)
      : {};
  const artifacts = Array.isArray(p.artifacts) ? p.artifacts : [];
  const missing = artifacts.filter(
    (a) =>
      typeof a === 'object' &&
      a !== null &&
      (a as Record<string, unknown>).present === false,
  );
  return {
    modelId,
    version,
    name: str(p.name) ?? modelId,
    kind: (MODEL_KINDS.includes(str(p.kind) ?? '') ? str(p.kind) : 'audio') as ModelKind,
    runtime: (RUNTIME_KINDS.includes(str(p.runtime) ?? '')
      ? str(p.runtime)
      : 'argv') as RuntimeKind,
    status: status as ModelStatus,
    cpuSeconds: u64str(str(budgets.cpuSeconds) ?? '0'),
    memoryBytes: u64str(str(budgets.memoryBytes) ?? '0'),
    wallNs: u64str(str(budgets.wallNs) ?? '0'),
    cpuThreads: typeof budgets.cpuThreads === 'number' ? budgets.cpuThreads : 1,
    artifactCount: artifacts.length,
    manifestSha256: str(p.manifestSha256) ?? '',
    license: str(p.license) ?? '',
    source: str(p.source) ?? 'bundled',
    missingArtifacts: missing.length,
    optionalArtifacts: artifacts.filter(
      (a) =>
        typeof a === 'object' &&
        a !== null &&
        (a as Record<string, unknown>).required === false,
    ).length,
  };
}

/** Parse a whole MODEL_LIST payload (`{models:[...]}` or bare array). */
export function parseModelList(payload: unknown): ModelRow[] {
  const rows = Array.isArray(payload)
    ? payload
    : typeof payload === 'object' && payload !== null
      ? ((payload as Record<string, unknown>).models ?? [])
      : [];
  if (!Array.isArray(rows)) return [];
  return rows.flatMap((r) => {
    const m = parseModelRow(r);
    return m ? [m] : [];
  });
}

/** Whether the model may be picked for a new job (T51 honest state). */
export function modelRunnable(m: ModelRow): boolean {
  return m.status === 'available' || m.status === 'degraded';
}
