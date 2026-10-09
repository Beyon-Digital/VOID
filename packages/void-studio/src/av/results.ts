// AV export results (W24) — parses `AvExportResult` cards and the
// provenance record written by void-av inside each published export dir.
//
// PROTOCOL GAP: like EXPORT_LIST, there is no AV_EXPORT_LIST ViewKind
// in void_control.fbs — the request builder carries the honest view
// name the integrator must add.

import type { ReadItem } from 'void-client';

export const AV_EXPORT_LIST_VIEW = 'AV_EXPORT_LIST';

export interface AvExportListRequest {
  request_id: string;
  project_id: string;
  view: 'AV_EXPORT_LIST';
  cursor?: string;
  limit?: number;
}

export function avExportListRequest(
  projectId: string,
  requestId: string,
  cursor = '',
): AvExportListRequest {
  return {
    request_id: requestId,
    project_id: projectId,
    view: AV_EXPORT_LIST_VIEW,
    cursor,
    limit: 0,
  };
}

/** serde shape of the result card void-av writes (result.json). */
export interface AvExportResultItem {
  kind: 'AvExportResult';
  jobId: string;
  status: 'succeeded' | 'failed';
  file?: string;
  sha256?: string;
  codecId?: string;
  videoFrames?: string;
  audioSamples?: string;
  error?: string;
  /** Bounded expected-vs-measured evidence — never a bit-identical claim. */
  verify?: {
    videoFramesExpected: string;
    videoFramesMeasured: string;
    audioSamplesExpected: string;
    audioSamplesMeasured: string;
    fpsMatchesSpec: boolean;
  };
}

export function parseAvExportResult(payload: unknown): AvExportResultItem | null {
  if (typeof payload !== 'object' || payload === null) return null;
  const p = payload as Record<string, unknown>;
  if (p.kind !== 'AvExportResult') return null;
  if (typeof p.jobId !== 'string' || typeof p.status !== 'string') return null;
  if (p.status !== 'succeeded' && p.status !== 'failed') return null;
  return p as unknown as AvExportResultItem;
}

/** Narrow read items into AV export result cards. */
export function avExportResultsFromItems(items: ReadItem[]): AvExportResultItem[] {
  const out: AvExportResultItem[] = [];
  for (const item of items) {
    const r = parseAvExportResult((item as unknown as { data?: unknown }).data);
    if (r) out.push(r);
  }
  return out;
}

/** Audio-sample drift recorded in a result card — never hidden. */
export function avAudioDriftSamples(r: AvExportResultItem): bigint | null {
  if (!r.verify) return null;
  const exp = BigInt(r.verify.audioSamplesExpected);
  const got = BigInt(r.verify.audioSamplesMeasured);
  return exp >= got ? exp - got : got - exp;
}
