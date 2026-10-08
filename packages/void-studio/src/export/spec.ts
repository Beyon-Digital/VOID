// Export spec DTO — the TS twin of crates/void-export `ExportSpec`
// (W10, T42). Every field name and shape must match its serde JSON
// exactly: camelCase keys, signed/unsigned 64-bit fields as DECIMAL
// STRINGS, enums snake_case, the tail as an internally-tagged union
// ({mode:"none"|"milliseconds"|"ticks"}).
//
// PROTOCOL GAP (noted for the integrator): void_control.fbs has no
// RenderRequest op — the export request is not a PersistentCommand. The
// integrator's shell surfaces this payload to the export runner; the
// studio package builds and validates it exactly as the runner expects.

import { i64str, u64str } from 'void-client';

export type ExportFormat = 'wav' | 'midi';
export type ExportChannels = 'mono' | 'stereo';
export type ExportBitDepth = 'pcm16' | 'pcm24' | 'float32';

export type ExportTailPolicy =
  | { mode: 'none' }
  | { mode: 'milliseconds'; ms: number }
  | { mode: 'ticks'; ticks: string };

export interface ExportTempoSegment {
  atTicks: string; // i64 decimal string
  bpm: number;
}

/** serde shape of crates/void-export::ExportSpec — do not rename fields. */
export interface ExportSpecDto {
  jobId: string;
  projectId: string;
  checkpointId: string;
  sourceRevision: string;
  assetHashes: string[];
  rangeStartTicks: string;
  rangeEndTicks: string;
  format: ExportFormat;
  channels?: ExportChannels;
  bitDepth?: ExportBitDepth;
  sampleRate?: number;
  tail: ExportTailPolicy;
  tempoMap: ExportTempoSegment[];
  outputName: string;
  deadlineMonotonicNs?: string;
}

/** serde shape of crates/void-export::FramePlan. */
export interface ExportFramePlanDto {
  rangeFrames: string;
  tailFrames: string;
  totalFrames: string;
}

// Rules mirrored from crates/void-export/src/spec.rs — keep in lockstep.
// (tick quantum lives in viewport.ts as the bigint TICKS_PER_QUARTER.)
export const APPROVED_SAMPLE_RATES: readonly number[] = [44_100, 48_000, 88_200, 96_000];
export const MAX_RENDER_FRAMES = 96_000 * 6 * 60 * 60;
export const TAIL_MS_MAX = 60_000;
export const OUTPUT_NAME_MAX = 120;

const UUID_RE =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;
const SHA256_RE = /^[0-9a-f]{64}$/;
const NAME_RE = /^[A-Za-z0-9._-]+$/;

export class ExportSpecError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'ExportSpecError';
  }
}

function bad(m: string): never {
  throw new ExportSpecError(m);
}

/** Mirror of sanitize_output_name: [A-Za-z0-9._-], no leading dot, <=120. */
export function sanitizeOutputName(name: string): string {
  const n = name.trim();
  if (n.length === 0 || n.length > OUTPUT_NAME_MAX) {
    bad(`output name empty or oversized (${n.length})`);
  }
  if (n.startsWith('.') || !NAME_RE.test(n)) {
    bad(`output name ${JSON.stringify(n)} is not a scoped filename`);
  }
  return n;
}

/** Validate an ExportSpecDto with the same rules void-export enforces. */
export function validateExportSpec(s: ExportSpecDto): void {
  if (!UUID_RE.test(s.jobId)) bad('jobId not a UUID');
  if (!UUID_RE.test(s.projectId)) bad('projectId not a UUID');
  if (!UUID_RE.test(s.checkpointId)) bad('checkpointId not a UUID');
  if (!/^-?\d+$/.test(s.sourceRevision)) bad('sourceRevision not a decimal u64');
  if (s.assetHashes.length > 128) bad('assetHashes exceeds 128 entries');
  for (const h of s.assetHashes) {
    if (!SHA256_RE.test(h)) bad('assetHashes[] not sha256 hex');
  }
  const start = i64str(s.rangeStartTicks);
  const end = i64str(s.rangeEndTicks);
  if (BigInt(end) <= BigInt(start)) bad('rangeEndTicks must be > rangeStartTicks');

  switch (s.format) {
    case 'wav':
      if (s.channels === undefined) bad('wav export requires channels');
      if (s.bitDepth === undefined) bad('wav export requires bitDepth');
      if (s.sampleRate === undefined) bad('wav export requires sampleRate');
      if (!APPROVED_SAMPLE_RATES.includes(s.sampleRate)) {
        bad('sampleRate not in approved set');
      }
      break;
    case 'midi':
      if (s.channels !== undefined || s.bitDepth !== undefined || s.sampleRate !== undefined) {
        bad('midi export must not carry channels/bitDepth/sampleRate');
      }
      break;
    default:
      bad('format must be wav|midi');
  }

  if (s.tail.mode === 'ticks') {
    if (BigInt(i64str(s.tail.ticks)) < 0n) bad('tail ticks must be >= 0');
  }
  if (s.tail.mode === 'milliseconds') {
    if (!Number.isFinite(s.tail.ms) || s.tail.ms < 0 || s.tail.ms > TAIL_MS_MAX) {
      bad(`tail ms must be within [0, ${TAIL_MS_MAX}]`);
    }
  }
  if (s.deadlineMonotonicNs !== undefined) u64str(s.deadlineMonotonicNs);
  sanitizeOutputName(s.outputName);
  if (s.tempoMap.length === 0) bad('tempo_map is empty');
  for (const seg of s.tempoMap) {
    i64str(seg.atTicks);
    if (!Number.isFinite(seg.bpm) || seg.bpm <= 0) bad('tempo bpm must be finite and > 0');
  }
}

export interface ExportContext {
  jobId: string;
  projectId: string;
  checkpointId: string;
  sourceRevision: string;
  assetHashes: string[];
  /** Effective piecewise tempo map the frame plan should be computed over. */
  tempoMap: ExportTempoSegment[];
  deadlineMonotonicNs?: string;
}

/** Dialog-owned fields the user picks. */
export interface ExportDialogSelection {
  outputName: string;
  format: ExportFormat;
  rangeStartTicks: string;
  rangeEndTicks: string;
  channels?: ExportChannels;
  bitDepth?: ExportBitDepth;
  sampleRate?: number;
  tail: ExportTailPolicy;
}

/** Merge dialog selection + immutable context into the wire DTO. */
export function buildExportSpec(
  sel: ExportDialogSelection,
  ctx: ExportContext,
): ExportSpecDto {
  const spec: ExportSpecDto = {
    jobId: ctx.jobId,
    projectId: ctx.projectId,
    checkpointId: ctx.checkpointId,
    sourceRevision: u64str(ctx.sourceRevision),
    assetHashes: [...ctx.assetHashes],
    rangeStartTicks: i64str(sel.rangeStartTicks),
    rangeEndTicks: i64str(sel.rangeEndTicks),
    format: sel.format,
    tail:
      sel.tail.mode === 'ticks'
        ? { mode: 'ticks', ticks: i64str(sel.tail.ticks) }
        : sel.tail.mode === 'milliseconds'
          ? { mode: 'milliseconds', ms: sel.tail.ms }
          : { mode: 'none' },
    tempoMap: ctx.tempoMap.map((t) => ({ atTicks: i64str(t.atTicks), bpm: t.bpm })),
    outputName: sel.outputName,
    ...(ctx.deadlineMonotonicNs !== undefined
      ? { deadlineMonotonicNs: u64str(ctx.deadlineMonotonicNs) }
      : {}),
  };
  // Optional fields stay ABSENT (not null/''): serde treats absence as
  // Option::None; '' would fail validation on the Rust side anyway.
  if (sel.format === 'wav') {
    spec.channels = sel.channels ?? bad('wav export requires channels');
    spec.bitDepth = sel.bitDepth ?? bad('wav export requires bitDepth');
    spec.sampleRate = sel.sampleRate ?? bad('wav export requires sampleRate');
  }
  validateExportSpec(spec);
  return spec;
}
