// Read-view parsers for project info + document notes (W11, DOC-05).
//
// The engine owns document notes and project metadata; the WebView only
// ever *parses* what a view reports. These parsers are deliberately
// defensive: a field absent from summary_json stays absent from the
// result — no invented metadata. When the rev2 note fields land, the
// same surfaces light up without a parse change.

import type { ReadItem } from 'void-client';

/** Project-information fields a PROJECT_SUMMARY item may carry. */
export interface ProjectInfo {
  projectId?: string;
  name?: string;
  sampleRate?: number;
  bpm?: number;
  /** Musical key label, e.g. "A minor" — reported verbatim. */
  key?: string;
  /** e.g. "4/4". */
  timeSignature?: string;
  trackCount?: number;
  assetCount?: number;
  /** Decimal-string byte total (CONTRACTS §1 wire rule). */
  storageBytes?: string;
  /** The document's project note when the engine reports one. */
  note?: string;
}

function body(item: ReadItem | unknown): Record<string, unknown> | null {
  try {
    const parsed =
      typeof item === 'object' && item !== null && 'summary_json' in (item as object)
        ? JSON.parse((item as ReadItem).summary_json)
        : item;
    if (typeof parsed !== 'object' || parsed === null) return null;
    const p = parsed as Record<string, unknown>;
    // Accept flat summaries and one level of {project|summary} nesting.
    for (const nest of ['project', 'summary'] as const) {
      const inner = p[nest];
      if (typeof inner === 'object' && inner !== null) {
        return inner as Record<string, unknown>;
      }
    }
    return p;
  } catch {
    return null;
  }
}

function str(v: unknown): string | undefined {
  return typeof v === 'string' && v.length > 0 ? v : undefined;
}

function num(v: unknown): number | undefined {
  if (typeof v === 'number' && Number.isFinite(v)) return v;
  if (typeof v === 'string' && v !== '' && Number.isFinite(Number(v))) return Number(v);
  return undefined;
}

/** Parse a PROJECT_SUMMARY ReadItem (or its decoded summary). */
export function parseProjectInfo(item: ReadItem | unknown): ProjectInfo | null {
  const p = body(item);
  if (!p) return null;
  const sig = p.time_signature ?? p.timeSignature;
  const info: ProjectInfo = {
    projectId: str(p.project_id) ?? str(p.projectId) ?? str(p.id),
    name: str(p.name),
    sampleRate: num(p.sample_rate ?? p.sampleRate),
    bpm: num(p.bpm ?? p.initial_bpm ?? p.tempo),
    key: str(p.key),
    timeSignature:
      typeof sig === 'string' && sig
        ? sig
        : typeof sig === 'object' && sig !== null
          ? `${num((sig as Record<string, unknown>).numerator) ?? '?'}/${num(
              (sig as Record<string, unknown>).denominator,
            ) ?? '?'}`
          : undefined,
    trackCount: num(p.track_count ?? p.trackCount),
    assetCount: num(p.asset_count ?? p.assetCount),
    storageBytes:
      str(p.storage_bytes) ??
      str(p.storageBytes) ??
      (num(p.storage_bytes) !== undefined ? String(p.storage_bytes) : undefined),
    note: str(p.note) ?? str(p.notes),
  };
  // An item with no usable fields is not project info — stay honest.
  return Object.values(info).some((v) => v !== undefined) ? info : null;
}

/**
 * Parse the document note out of a view row (TRACK_LIST track rows,
 * PROJECT_SUMMARY, …). Returns the note string or undefined when the row
 * does not report one — distinct from an empty note deliberately set.
 */
export function parseNoteField(item: ReadItem | unknown): string | undefined {
  const p = body(item);
  if (!p) return undefined;
  const v = p.note ?? p.notes;
  return typeof v === 'string' ? v : undefined;
}
