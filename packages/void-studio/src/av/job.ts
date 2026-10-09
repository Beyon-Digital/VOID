// AV export job envelope (W24) — mirrors crates/void-av::job_spec_for.
// Rides the same void-jobs state machine and the same (not-yet-in-.fbs)
// JobEvent telemetry shape as the audio export lane — parser reused
// from ../export/job rather than duplicated.

import { u64str } from 'void-client';
import type { AvExportSpecDto } from './spec';

export {
  applyJobEvent,
  initialProgress,
  isTerminal,
  parseJobEvent,
} from '../export/job';
export type { ExportJobStatus, ExportProgress, JobEventDto } from '../export/job';

/** serde shape of void_jobs::JobSpec for the void-av runtime. */
export interface AvJobEnvelope {
  jobId: string;
  projectId: string;
  sourceRevision: string;
  /** sha256 of the verified checkpoint manifest — pins input bytes. */
  contextSha256: string;
  kind: 'av_export';
  runtimeId: 'void-av';
  runtimeSha256: string;
  inputs: string[];
  /** Whole AvExportSpecDto — the runner re-validates it verbatim. */
  parameters: AvExportSpecDto;
  reservations: { ramBytes: string; vramBytes: string; cpuThreads: number };
  deadlineMonotonicNs: string;
  outputScopeToken: string;
}

/**
 * Wrap a validated spec. `contextSha256` is the checkpoint manifest
 * hash the runner's verify_checkpoint produced — read from the
 * checkpoint read path, never hashed by the UI.
 */
export function avExportJobEnvelope(
  spec: AvExportSpecDto,
  contextSha256: string,
  runtimeSha256: string,
): AvJobEnvelope {
  return {
    jobId: spec.jobId,
    projectId: spec.projectId,
    sourceRevision: u64str(spec.sourceRevision),
    contextSha256,
    kind: 'av_export',
    runtimeId: 'void-av',
    runtimeSha256,
    inputs: spec.inputs
      .filter((i) => i.kind !== 'lavfi_test')
      .map((i) => (i.kind === 'asset_blob' ? i.sha256 : `${i.rel_path}:${i.sha256}`)),
    parameters: spec,
    reservations: { ramBytes: '536870912', vramBytes: '0', cpuThreads: 1 },
    deadlineMonotonicNs: spec.deadlineMonotonicNs ?? '0',
    outputScopeToken: `av:${spec.jobId}`,
  };
}
