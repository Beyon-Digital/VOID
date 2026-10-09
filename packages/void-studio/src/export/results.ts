// Export result list (W10) — parses `ExportResult` cards from read items.
//
// The result card is written by void-export inside each published export
// dir (result.json) and echoed into the job record's result_json by the
// runner. The UI sees them through read views — PROTOCOL GAP: there is
// no EXPORT_LIST ViewKind in void_control.fbs yet, so the request
// builder below carries the view name the integrator must add. Until
// the coordinator serves it, expect UNSUPPORTED_CAPABILITY — surfaced
// verbatim, never papered over.

import type { I64, ReadItem } from 'void-client';

/** The ViewKind name the coordinator will expose for export cards. */
export const EXPORT_LIST_VIEW = 'EXPORT_LIST';

/**
 * Read request for the export list. Returned as a structurally-typed
 * object — `view` is deliberately NOT the ViewKindName union because
 * EXPORT_LIST does not exist in the .fbs yet (protocol gap; the request
 * is honest about what it asks for).
 */
export interface ExportListRequest {
  request_id: string;
  project_id: string;
  view: 'EXPORT_LIST';
  cursor?: string;
  limit?: number;
  track_id?: string;
  start_ticks?: I64;
  end_ticks?: I64;
}

export function exportListRequest(
  projectId: string,
  requestId: string,
  cursor = '',
): ExportListRequest {
  return {
    request_id: requestId,
    project_id: projectId,
    view: EXPORT_LIST_VIEW,
    cursor,
    limit: 0,
    track_id: '',
    start_ticks: '-1',
    end_ticks: '-1',
  };
}

/** serde shape of the ExportResult card (crates/void-export). */
export interface ExportResultItem {
  kind: 'ExportResult';
  jobId: string;
  status: 'succeeded' | 'failed';
  file?: string;
  sha256?: string;
  bytes?: string;
  frames?: string;
  error?: string;
  warnings?: string[];
}

/** Narrow a read item's summary_json to an export card. */
export function parseExportResult(item: ReadItem): ExportResultItem | null {
  try {
    const s = JSON.parse(item.summary_json) as Record<string, unknown>;
    if (s.kind !== 'ExportResult' || typeof s.jobId !== 'string') return null;
    return {
      kind: 'ExportResult',
      jobId: s.jobId,
      status: s.status === 'succeeded' ? 'succeeded' : 'failed',
      file: typeof s.file === 'string' ? s.file : undefined,
      sha256: typeof s.sha256 === 'string' ? s.sha256 : undefined,
      bytes: typeof s.bytes === 'string' ? s.bytes : undefined,
      frames: typeof s.frames === 'string' ? s.frames : undefined,
      error: typeof s.error === 'string' ? s.error : undefined,
      warnings: Array.isArray(s.warnings)
        ? s.warnings.filter((w): w is string => typeof w === 'string')
        : undefined,
    };
  } catch {
    return null;
  }
}

/** All export cards from a read page. Malformed items drop, never fake. */
export function exportResultsFromItems(items: ReadItem[]): ExportResultItem[] {
  const out: ExportResultItem[] = [];
  for (const it of items) {
    const r = parseExportResult(it);
    if (r) out.push(r);
  }
  return out;
}
