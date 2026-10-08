// Flashback capture policy (W17/REC-06; T66).
//
// "Capture the intended recent passage; purge disabled buffers and
// respect disk limits." The actual audio/MIDI buffering is
// engine-side; the studio owns the CONSENT + RETENTION policy model
// and the recovery mapping (recent buffer → take record).
//
// Consent is explicit: a buffer only retains input while `enabled &&
// consentGranted`. Disabling purges every retained chunk (honest
// purge — no shadow copies). Capacity is in ticks; a full buffer
// evicts its oldest chunk — the buffer is a ring, never unbounded.

import { i64str, parseI64 } from 'void-client';
import type { TakeRecord } from './types';
import { checkTake } from './takes';

export interface CaptureChunk {
  chunkId: string;
  /** Timeline position of the captured passage. */
  startTicks: string;
  lengthTicks: string;
  /** Asset/data reference the engine assigned the chunk. */
  assetId?: string;
  sourceHash?: string;
  capturedAt?: string;
}

export interface FlashbackBuffer {
  bufferId: string;
  trackId: string;
  kind: 'AUDIO' | 'MIDI';
  /** Hard retention bound in ticks — a ring, not a log. */
  capacityTicks: string;
  enabled: boolean;
  consentGranted: boolean;
  chunks: CaptureChunk[];
}

export class CaptureError extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = 'CaptureError';
  }
}

export function makeBuffer(args: {
  bufferId: string;
  trackId: string;
  kind: 'AUDIO' | 'MIDI';
  capacityTicks: string;
  consentGranted: boolean;
}): FlashbackBuffer {
  if (parseI64(args.capacityTicks) <= 0n) {
    throw new CaptureError('capture capacity must be > 0 ticks');
  }
  return {
    ...args,
    enabled: false,
    chunks: [],
  };
}

function retainedTicks(b: FlashbackBuffer): bigint {
  return b.chunks.reduce((a, c) => a + parseI64(c.lengthTicks), 0n);
}

/** Enable capture — requires consent. Without consent the buffer
 * stays off and empty (no silent capture, REC-06). */
export function enableBuffer(b: FlashbackBuffer): FlashbackBuffer {
  if (!b.consentGranted) {
    throw new CaptureError('flashback capture requires explicit consent');
  }
  return { ...b, enabled: true };
}

/** Disable + purge: disabling ALWAYS empties the ring — retained
 * material is deleted, not parked. */
export function disableBuffer(b: FlashbackBuffer): FlashbackBuffer {
  return { ...b, enabled: false, chunks: [] };
}

/** Push one chunk into the ring; evicts oldest chunks past capacity. */
export function pushChunk(b: FlashbackBuffer, chunk: CaptureChunk): FlashbackBuffer {
  if (!b.enabled) return b; // disabled buffers retain nothing
  const chunks = [...b.chunks, chunk];
  let total = chunks.reduce((a, c) => a + parseI64(c.lengthTicks), 0n);
  const cap = parseI64(b.capacityTicks);
  while (chunks.length > 0 && total > cap) {
    total -= parseI64(chunks[0].lengthTicks);
    chunks.shift();
  }
  return { ...b, chunks };
}

/** The tail of the ring covering the most recent `spanTicks`. */
export function recentWindow(b: FlashbackBuffer, spanTicks: string): CaptureChunk[] {
  const want = parseI64(spanTicks);
  const out: CaptureChunk[] = [];
  let acc = 0n;
  for (let i = b.chunks.length - 1; i >= 0 && acc < want; i--) {
    out.unshift(b.chunks[i]);
    acc += parseI64(b.chunks[i].lengthTicks);
  }
  return out;
}

/**
 * Recover the captured passage into a take record (REC-06 "recovery
 * into takes"). The take references the recovered material as one
 * region — it is then an ordinary immutable take usable for comping.
 */
export function recoverToTake(args: {
  buffer: FlashbackBuffer;
  takeId: string;
  folderId: string;
  laneIndex: number;
  spanTicks: string;
  recordedAt?: string;
}): TakeRecord | null {
  const window = recentWindow(args.buffer, args.spanTicks);
  if (window.length === 0) return null;
  const start = parseI64(window[0].startTicks);
  const end = window.reduce(
    (m, c) => {
      const e = parseI64(c.startTicks) + parseI64(c.lengthTicks);
      return e > m ? e : m;
    },
    start,
  );
  const rec: TakeRecord = {
    takeId: args.takeId,
    folderId: args.folderId,
    trackId: args.buffer.trackId,
    kind: args.buffer.kind,
    laneIndex: args.laneIndex,
    assetId: window.length === 1 ? window[0].assetId : undefined,
    sourceHash: window.length === 1 ? window[0].sourceHash : undefined,
    regionStartTicks: i64str(start),
    regionLengthTicks: i64str(end - start),
    offsetTicks: '0',
    complete: true,
    recordedAt: args.recordedAt,
  };
  if (rec.kind === 'AUDIO' && rec.assetId === undefined) {
    // Multi-chunk audio recovery has no single asset_id — honest gap:
    // the engine concatenates chunks into one asset on recovery; the
    // take cannot be placed on the timeline until it does. Keep the
    // record out (caller gets null) rather than fake an asset.
    return null;
  }
  checkTake(rec);
  return rec;
}
