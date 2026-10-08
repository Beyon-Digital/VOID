// Timeline geometry — clip model, px projection and hit-testing.
//
// Clips arrive as CLIP_LIST ReadItems: {object_id, summary_json} where
// summary_json is a bounded JSON string whose field names are the
// engine's contract — parse defensively, skip what we don't recognise.
// All tick math is bigint-exact; px is display-only (see viewport.ts).

import type { ReadItem } from 'void-client';
import { parseI64 } from 'void-client';
import { TickViewport, Zoom, ticksToPx } from '../viewport';

/** One clip in the arrangement, projected from a read item. */
export interface ClipView {
  clipId: string;
  trackId: string;
  kind?: 'AUDIO' | 'MIDI' | string;
  name?: string;
  color?: string;
  assetId?: string;
  startTicks: string;
  lengthTicks: string;
  offsetTicks: string;
}

const isObj = (v: unknown): v is Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v);

const str = (v: unknown): string | undefined =>
  typeof v === 'string' && v.length > 0 ? v : undefined;

const dec = (v: unknown): string | undefined => {
  // Wire rule: int64 fields are decimal strings. Accept a decimal-looking
  // string or a safe integer; reject everything else (NaN, floats, junk).
  if (typeof v === 'string') {
    const s = v.trim();
    if (!/^-?\d+$/.test(s)) return undefined;
    return parseI64(s).toString(10);
  }
  if (typeof v === 'number' && Number.isSafeInteger(v)) return String(v);
  if (typeof v === 'bigint') return v.toString(10);
  return undefined;
};

/**
 * Defensive projection of one CLIP_LIST item into a ClipView.
 * Returns null when identity or range fields are missing/malformed —
 * a clip we can't place is skipped, never guessed.
 */
export function parseClipItem(item: ReadItem): ClipView | null {
  let raw: unknown;
  try {
    raw = JSON.parse(item.summary_json);
  } catch {
    return null;
  }
  if (!isObj(raw)) return null;
  const clipId =
    str(raw.clip_id) ??
    str(raw.clipId) ??
    str(raw.id) ??
    (item.object_id.startsWith('clip:') ? item.object_id.slice(5) : undefined);
  const trackId = str(raw.track_id) ?? str(raw.trackId);
  const startTicks = dec(raw.start_ticks ?? raw.startTicks);
  const lengthTicks = dec(raw.length_ticks ?? raw.lengthTicks);
  if (!clipId || !trackId || startTicks === undefined || lengthTicks === undefined) {
    return null;
  }
  let length: bigint;
  try {
    length = parseI64(lengthTicks);
    parseI64(startTicks);
  } catch {
    return null;
  }
  if (length <= 0n) return null; // malformed clip never renders
  const kindRaw = str(raw.kind) ?? str(raw.clip_type) ?? str(raw.type);
  return {
    clipId,
    trackId,
    kind:
      kindRaw === 'audio' || kindRaw === 'AUDIO'
        ? 'AUDIO'
        : kindRaw === 'midi' || kindRaw === 'MIDI'
          ? 'MIDI'
          : kindRaw,
    name: str(raw.name),
    color: str(raw.color),
    assetId: str(raw.asset_id) ?? str(raw.assetId),
    startTicks,
    lengthTicks,
    offsetTicks: dec(raw.offset_ticks ?? raw.offsetTicks) ?? '0',
  };
}

export function clipEndTicks(c: ClipView): string {
  return (parseI64(c.startTicks) + parseI64(c.lengthTicks)).toString(10);
}

/** Px rect of a clip inside the viewport. `w` may extend past the view. */
export function clipRect(
  c: ClipView,
  v: TickViewport,
  z: Zoom,
): { x: number; w: number } {
  const x = ticksToPx(c.startTicks, v, z);
  const w = Number(parseI64(c.lengthTicks)) / z.ticksPerPixel;
  return { x, w };
}

export type ClipHitZone = 'body' | 'trim-start' | 'trim-end' | null;

/**
 * Hit-test a px position against a clip rect. Edge zones of `edgePx`
 * (min 2px when the clip is narrower) hit the trim handles.
 */
export function hitTestClip(
  c: ClipView,
  px: number,
  v: TickViewport,
  z: Zoom,
  edgePx = 6,
): ClipHitZone {
  const { x, w } = clipRect(c, v, z);
  if (px < x || px > x + w || w <= 0) return null;
  const edge = Math.min(edgePx, w / 2);
  if (px - x <= edge) return 'trim-start';
  if (x + w - px <= edge) return 'trim-end';
  return 'body';
}

/**
 * Find the topmost clip at a px position, honoring a paint order that
 * puts later items on top. Returns the clip + hit zone.
 */
export function hitTestClips(
  clips: ClipView[],
  px: number,
  v: TickViewport,
  z: Zoom,
  edgePx = 6,
): { clip: ClipView; zone: Exclude<ClipHitZone, null> } | null {
  for (let i = clips.length - 1; i >= 0; i--) {
    const zone = hitTestClip(clips[i], px, v, z, edgePx);
    if (zone) return { clip: clips[i], zone };
  }
  return null;
}

/** Clips visible in the viewport, projected to px rects (bounded work). */
export function layoutClips(
  clips: ClipView[],
  v: TickViewport,
  z: Zoom,
): Array<{ clip: ClipView; x: number; w: number }> {
  const start = parseI64(v.startTicks);
  const end = parseI64(v.endTicks);
  const out: Array<{ clip: ClipView; x: number; w: number }> = [];
  for (const c of clips) {
    const cStart = parseI64(c.startTicks);
    const cEnd = cStart + parseI64(c.lengthTicks);
    if (cEnd <= start || cStart >= end) continue;
    const { x, w } = clipRect(c, v, z);
    out.push({ clip: c, x, w });
  }
  return out;
}
