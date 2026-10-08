// AI job card model (W12, CONTRACTS.md §6) — list/status/progress state
// for the studio's job surface. Mirrors crates/void-jobs on the wire:
// statuses are snake_case, i64/u64 fields decimal strings.
//
// PROTOCOL GAP (integrator note): void_control.fbs has no SubmitJob/
// CancelJob persistent ops, no JOB_LIST/MODEL_LIST ViewKind and no
// JobEvent telemetry union member — this module defines the honest DTO
// shapes the coordinator will emit and parses them defensively. Until
// the ops land, the view model exposes `cancelJobOp` building the exact
// intended op payload and the store degrades to local status only.

import { u64str } from 'void-client';
import type { JobEventDto } from '../export/job';
import { parseJobEvent } from '../export/job';

export type JobStatus =
  | 'queued'
  | 'running'
  | 'succeeded'
  | 'failed'
  | 'cancelling'
  | 'cancelled';

export type JobKind =
  | 'symbolic'
  | 'transcription'
  | 'separation'
  | 'audio_generation'
  | 'visual_generation'
  | 'analysis'
  | 'av_export';

const JOB_STATUSES: readonly string[] = [
  'queued',
  'running',
  'succeeded',
  'failed',
  'cancelling',
  'cancelled',
];

const JOB_KINDS: readonly string[] = [
  'symbolic',
  'transcription',
  'separation',
  'audio_generation',
  'visual_generation',
  'analysis',
  'av_export',
];

export function isTerminal(s: JobStatus): boolean {
  return s === 'succeeded' || s === 'failed' || s === 'cancelled';
}

/** One artifact committed by a succeeded job (result.json echo). */
export interface JobArtifact {
  name: string;
  sha256: string;
  /** Decimal string per §1. */
  bytes: string;
  /** Container-relative asset path: assets/sha256/<hash>.<ext>. */
  asset: string;
}

/** The UI's per-job card — a read view, never the job's authority. */
export interface JobCard {
  jobId: string;
  projectId: string;
  kind: JobKind;
  status: JobStatus;
  /** null until the job reports a real percent — indeterminate bar. */
  percent: number | null;
  message?: string;
  modelId?: string;
  modelVersion?: string;
  runtimeId: string;
  /** Budget badge facts — decimal strings rendered verbatim. */
  ramBytes: string;
  vramBytes: string;
  cpuThreads: number;
  /** Effective enforced caps when the coordinator reports them. */
  cpuSeconds?: string;
  memoryBytes?: string;
  deadlineMonotonicNs: string;
  artifacts: JobArtifact[];
  /** Quarantined late results are inspectable, never applied. */
  quarantined: boolean;
  createdAt: string;
  updatedAt: string;
}

function str(v: unknown): string | undefined {
  return typeof v === 'string' && v.length > 0 ? v : undefined;
}

/**
 * Narrow an unknown payload to a job card. Accepts both the flat
 * JOB_LIST row shape and a JobRecord-like `{spec, status}` nest —
 * defensive, never fabricates fields it cannot see.
 */
export function parseJobCard(payload: unknown): JobCard | null {
  if (typeof payload !== 'object' || payload === null) return null;
  const p = payload as Record<string, unknown>;
  const spec =
    typeof p.spec === 'object' && p.spec !== null
      ? (p.spec as Record<string, unknown>)
      : p;
  const jobId = str(spec.jobId) ?? str(p.jobId);
  const projectId = str(spec.projectId) ?? str(p.projectId) ?? '';
  const status = str(p.status);
  const kind = str(spec.kind) ?? 'analysis';
  if (!jobId || !status || !JOB_STATUSES.includes(status)) return null;
  const res =
    typeof spec.reservations === 'object' && spec.reservations !== null
      ? (spec.reservations as Record<string, unknown>)
      : {};
  const artifacts = Array.isArray(p.artifacts)
    ? (p.artifacts as unknown[]).flatMap((a) => {
        if (typeof a !== 'object' || a === null) return [];
        const r = a as Record<string, unknown>;
        const name = str(r.name);
        const sha256 = str(r.sha256);
        const asset = str(r.asset);
        if (!name || !sha256 || !asset) return [];
        return [{ name, sha256, bytes: str(r.bytes) ?? '0', asset }];
      })
    : [];
  return {
    jobId,
    projectId,
    kind: (JOB_KINDS.includes(kind) ? kind : 'analysis') as JobKind,
    status: status as JobStatus,
    percent:
      typeof p.percent === 'number' && Number.isFinite(p.percent)
        ? Math.min(100, Math.max(0, p.percent))
        : null,
    message: str(p.message),
    modelId: str(spec.modelId),
    modelVersion: str(spec.modelVersion),
    runtimeId: str(spec.runtimeId) ?? 'unknown',
    ramBytes: str(res.ramBytes) ?? '0',
    vramBytes: str(res.vramBytes) ?? '0',
    cpuThreads: typeof res.cpuThreads === 'number' ? res.cpuThreads : 0,
    cpuSeconds: str(p.cpuSeconds),
    memoryBytes: str(p.memoryBytes),
    deadlineMonotonicNs: u64str(str(spec.deadlineMonotonicNs) ?? '0'),
    artifacts,
    quarantined: p.quarantined === true,
    createdAt: str(p.createdAt) ?? '',
    updatedAt: str(p.updatedAt) ?? '',
  };
}

/** Fold a JobEvent into a card: only the event's own job + real fields. */
export function applyJobEventToCard(card: JobCard, ev: JobEventDto): JobCard {
  if (ev.job_id !== card.jobId) return card;
  if (!JOB_STATUSES.includes(ev.status)) return card;
  return {
    ...card,
    status: ev.status as JobStatus,
    percent:
      ev.percent !== undefined
        ? Math.min(100, Math.max(0, ev.percent))
        : card.percent,
    message: ev.message ?? card.message,
    quarantined: card.quarantined || ev.quarantined === true,
  };
}

export function jobEventOf(payload: unknown): JobEventDto | null {
  return parseJobEvent(payload);
}

// ---------------------------------------------------------------------------
// Ops — intended void_control union members (see docs/engine/NEEDS.md W12)
// ---------------------------------------------------------------------------

/** SubmitJob persistent op payload (job/1.0.0 envelope verbatim). */
export interface SubmitJobOp {
  op: 'submit_job';
  spec: JobSpecEnvelope;
}

/** CancelJob persistent op payload. */
export interface CancelJobOp {
  op: 'cancel_job';
  jobId: string;
}

/** serde shape of void_jobs::JobSpec — camelCase, decimal-string numerics. */
export interface JobSpecEnvelope {
  jobId: string;
  projectId: string;
  sourceRevision: string;
  contextSha256: string;
  kind: JobKind;
  runtimeId: string;
  runtimeSha256: string;
  modelId?: string;
  modelSha256?: string;
  inputs: string[];
  parameters: unknown;
  reservations: { ramBytes: string; vramBytes: string; cpuThreads: number };
  deadlineMonotonicNs: string;
  outputScopeToken: string;
  cloudConsentId?: string;
}

export function cancelJobOp(jobId: string): CancelJobOp {
  return { op: 'cancel_job', jobId };
}

export function submitJobOp(spec: JobSpecEnvelope): SubmitJobOp {
  return { op: 'submit_job', spec };
}

/** JOB_LIST view request — empty page = the whole nonterminal list. */
export function jobListRequest(projectId: string, includeTerminal = false) {
  return { view: 'job_list', projectId, includeTerminal };
}

/** MODEL_LIST view request — the registry's read view. */
export function modelListRequest() {
  return { view: 'model_list' };
}
