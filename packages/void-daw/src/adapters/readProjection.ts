// Adapter: derive the ClipMatrix render shape from read_view pages.
//
// The UI never holds a song clone: SessionView renders a projection built
// from CLIP_LIST (and scene rows from PROJECT_SUMMARY) items delivered by
// the engine's read_view command. Items are the wire shape
// {object_id, summary_json} where summary_json is a STRING of bounded JSON
// — this adapter parses it defensively and skips anything it does not
// recognise rather than guessing (projection field names are the engine's
// contract, not the renderer's).

import type { ClipMatrix, Scene, SessionClip } from 'void-core';
import type { ReadItem } from 'void-client';

export type { ReadItem };

const isObj = (v: unknown): v is Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v);

const str = (v: unknown): string | undefined =>
  typeof v === 'string' && v.length > 0 ? v : undefined;

const num = (v: unknown): number | undefined =>
  typeof v === 'number' && Number.isFinite(v) ? v : undefined;

function parseSummary(raw: string): Record<string, unknown> | null {
  try {
    const v = JSON.parse(raw) as unknown;
    return isObj(v) ? v : null;
  } catch {
    return null;
  }
}

/** true when the row looks like a scene (not a clip). */
function isSceneLike(v: Record<string, unknown>, objectId: string): boolean {
  if (objectId.startsWith('clip:')) return false;
  if (objectId.startsWith('scene:')) return true;
  const kind = str(v.kind) ?? str(v.type);
  if (kind === 'scene') return true;
  if (kind === 'clip') return false;
  return v.scene_id === undefined && v.track_id === undefined && v.sceneId === undefined;
}

function toScene(v: Record<string, unknown>, objectId: string): Scene | null {
  const id = str(v.scene_id) ?? str(v.id) ?? (objectId.startsWith('scene:') ? objectId.slice(6) : undefined);
  if (!id) return null;
  return {
    id,
    name: str(v.name) ?? id,
    tempo: num(v.tempo),
    timeSignature: str(v.time_signature) ?? str(v.timeSignature),
  };
}

function toClip(v: Record<string, unknown>, objectId: string): SessionClip | null {
  const id = str(v.clip_id) ?? str(v.id) ?? (objectId.startsWith('clip:') ? objectId.slice(5) : undefined);
  const sceneId = str(v.scene_id) ?? str(v.sceneId);
  const trackId = str(v.track_id) ?? str(v.trackId);
  if (!id || !sceneId || !trackId) return null;
  const kind = str(v.clip_type) ?? str(v.type);
  return {
    id,
    name: str(v.name) ?? id,
    sceneId,
    trackId,
    startBeat: num(v.start_beat) ?? num(v.startBeat) ?? 0,
    length: num(v.length_beats) ?? num(v.length) ?? 0,
    loop: v.loop === true,
    color: str(v.color),
    type: kind === 'midi' ? 'midi' : 'audio',
  };
}

/**
 * Build a bounded ClipMatrix from read_view items. Malformed items are
 * skipped (counted in `dropped`); clip membership stays on the clip's own
 * scene_id — no prefix inference anywhere.
 */
export function clipMatrixFromReadItems(
  items: ReadonlyArray<ReadItem | { object_id?: string; summary_json?: string }>,
  opts: { maxItems?: number } = {},
): { matrix: ClipMatrix; dropped: number; truncated: boolean } {
  const max = opts.maxItems ?? 2000;
  const scenes = new Map<string, Scene>();
  const clips = new Map<string, SessionClip>();
  let dropped = 0;
  let truncated = false;

  for (const item of items) {
    if (scenes.size + clips.size >= max) {
      truncated = true;
      break;
    }
    const objectId = str(item?.object_id) ?? '';
    const raw = item?.summary_json;
    const v = typeof raw === 'string' ? parseSummary(raw) : null;
    if (v === null) {
      dropped += 1;
      continue;
    }
    if (isSceneLike(v, objectId)) {
      const s = toScene(v, objectId);
      if (s) scenes.set(s.id, s);
      else dropped += 1;
    } else {
      const c = toClip(v, objectId);
      if (c) clips.set(c.id, c);
      else dropped += 1;
    }
  }

  return { matrix: { clips: [...clips.values()], scenes: [...scenes.values()] }, dropped, truncated };
}
