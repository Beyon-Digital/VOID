// Export job envelope + progress tracking (W10, T42/T43).
//
// The job rides the void-jobs state machine: queued → running →
// succeeded | failed | cancelling → cancelled. `exportJobEnvelope`
// mirrors crates/void-export::job_spec_for — `parameters` carries the
// whole spec echo so the runner re-validates it; `contextSha256` binds
// the job to the immutable checkpoint's manifest hash.
//
// PROTOCOL GAP (integrator note): void_control.fbs / TelemetryEvent has
// no job-event kind and ViewKindName has no EXPORT_LIST. Progress events
// and the export list arrive once the coordinator surfaces them — this
// module parses the expected shapes defensively and never fabricates
// progress or results.

import { u64str } from 'void-client';
import type { ExportSpecDto } from './spec';

export type ExportJobStatus =
  | 'queued'
  | 'running'
  | 'succeeded'
  | 'failed'
  | 'cancelling'
  | 'cancelled';

export function isTerminal(status: ExportJobStatus): boolean {
  return status === 'succeeded' || status === 'failed' || status === 'cancelled';
}

/** serde shape of void_jobs::JobSpec (job/1.0.0), subset the UI sends. */
export interface ExportJobEnvelope {
  jobId: string;
  projectId: string;
  sourceRevision: string;
  /** sha256 of the verified checkpoint manifest — pins the input bytes. */
  contextSha256: string;
  kind: 'av_export';
  runtimeId: 'void-export';
  runtimeSha256: string;
  inputs: string[];
  /** Whole ExportSpecDto — the runner re-validates it verbatim. */
  parameters: ExportSpecDto;
  reservations: { ramBytes: string; vramBytes: string; cpuThreads: number };
  deadlineMonotonicNs: string;
  outputScopeToken: string;
}

/**
 * Wrap a validated spec in the job envelope. `contextSha256` must be the
 * checkpoint manifest hash the runner's verify_checkpoint produced —
 * the caller reads it from the checkpoint read path; a UI must never
 * hash a checkpoint it cannot see.
 */
export function exportJobEnvelope(
  spec: ExportSpecDto,
  contextSha256: string,
  runtimeSha256: string,
): ExportJobEnvelope {
  return {
    jobId: spec.jobId,
    projectId: spec.projectId,
    sourceRevision: u64str(spec.sourceRevision),
    contextSha256,
    kind: 'av_export',
    runtimeId: 'void-export',
    runtimeSha256,
    inputs: [...spec.assetHashes],
    parameters: spec,
    reservations: { ramBytes: '536870912', vramBytes: '0', cpuThreads: 1 },
    deadlineMonotonicNs: spec.deadlineMonotonicNs ?? '0',
    outputScopeToken: `export:${spec.jobId}`,
  };
}

// ---------------------------------------------------------------------------
// Job events (expected telemetry shape — not yet in the protocol union)
// ---------------------------------------------------------------------------

/** Expected `void://telemetry` job event once the coordinator emits it. */
export interface JobEventDto {
  kind: 'JobEvent';
  project_id: string;
  job_id: string;
  status: string;
  /** Optional real progress percent (0..100) — only if the runner emits it. */
  percent?: number;
  message?: string;
  /** Present when a late/foreign result was quarantined. */
  quarantined?: boolean;
}

/** Narrow an unknown payload to a job event by shape. */
export function parseJobEvent(payload: unknown): JobEventDto | null {
  if (typeof payload !== 'object' || payload === null) return null;
  const p = payload as Record<string, unknown>;
  if (p.kind !== 'JobEvent') return null;
  if (typeof p.job_id !== 'string' || typeof p.status !== 'string') return null;
  return {
    kind: 'JobEvent',
    project_id: typeof p.project_id === 'string' ? p.project_id : '',
    job_id: p.job_id,
    status: p.status,
    percent:
      typeof p.percent === 'number' && Number.isFinite(p.percent) ? p.percent : undefined,
    message: typeof p.message === 'string' ? p.message : undefined,
    quarantined: p.quarantined === true,
  };
}

/** UI-side progress state for one export job — no fabricated percent. */
export interface ExportProgress {
  jobId: string;
  status: ExportJobStatus;
  /** null until the job actually reports one — an indeterminate bar. */
  percent: number | null;
  message?: string;
  quarantined: boolean;
}

export function initialProgress(jobId: string): ExportProgress {
  return { jobId, status: 'queued', percent: null, quarantined: false };
}

const STATUSES: readonly string[] = [
  'queued',
  'running',
  'succeeded',
  'failed',
  'cancelling',
  'cancelled',
];

/** Fold a job event into progress. Unknown statuses are ignored. */
export function applyJobEvent(prev: ExportProgress, ev: JobEventDto): ExportProgress {
  if (ev.job_id !== prev.jobId) return prev;
  if (!STATUSES.includes(ev.status)) return prev;
  return {
    jobId: prev.jobId,
    status: ev.status as ExportJobStatus,
    percent: ev.percent !== undefined ? Math.min(100, Math.max(0, ev.percent)) : prev.percent,
    message: ev.message ?? prev.message,
    quarantined: prev.quarantined || ev.quarantined === true,
  };
}
