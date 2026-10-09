// Piano-roll note model — NOTE_RANGE projections, geometry, clamps.
//
// Notes arrive as NOTE_RANGE ReadItems scoped by track + tick window and
// carry their own clip_id — the roll shows notes for ONE selected clip
// and filters by clip_id defensively. Validation mirrors the schema:
// pitch 0..127, velocity 1..127, length_ticks > 0. Malformed items are
// dropped, never clamped into fake notes.

import type { ReadItem } from 'void-client';
import { parseI64 } from 'void-client';
import type { TickViewport, Zoom } from '../viewport';
import { ticksToPx } from '../viewport';

export interface NoteView {
  noteId: string;
  clipId: string;
  pitch: number; // 0..127
  velocity: number; // 1..127
  startTicks: string;
  lengthTicks: string;
}

export const PITCH_MIN = 0;
export const PITCH_MAX = 127;
export const VELOCITY_MIN = 1;
export const VELOCITY_MAX = 127;
/** Smallest legal note length (ticks). */
export const MIN_NOTE_LENGTH = 1n;

const isObj = (v: unknown): v is Record<string, unknown> =>
  typeof v === 'object' && v !== null && !Array.isArray(v);

const str = (v: unknown): string | undefined =>
  typeof v === 'string' && v.length > 0 ? v : undefined;

const int = (v: unknown): number | undefined =>
  typeof v === 'number' && Number.isInteger(v) ? v : undefined;

const dec = (v: unknown): string | undefined => {
  if (typeof v === 'string' && /^-?\d+$/.test(v.trim())) return v.trim();
  if (typeof v === 'number' && Number.isSafeInteger(v)) return String(v);
  if (typeof v === 'bigint') return v.toString(10);
  return undefined;
};

export function clampPitch(p: number): number {
  if (!Number.isFinite(p)) return PITCH_MIN;
  return Math.min(PITCH_MAX, Math.max(PITCH_MIN, Math.round(p)));
}

export function clampVelocity(v: number): number {
  if (!Number.isFinite(v)) return VELOCITY_MIN;
  return Math.min(VELOCITY_MAX, Math.max(VELOCITY_MIN, Math.round(v)));
}

/** Defensive projection of one NOTE_RANGE item. */
export function parseNoteItem(item: ReadItem): NoteView | null {
  let raw: unknown;
  try {
    raw = JSON.parse(item.summary_json);
  } catch {
    return null;
  }
  if (!isObj(raw)) return null;
  const noteId =
    str(raw.note_id) ??
    str(raw.noteId) ??
    str(raw.id) ??
    (item.object_id.startsWith('note:') ? item.object_id.slice(5) : undefined);
  const clipId = str(raw.clip_id) ?? str(raw.clipId);
  const pitch = int(raw.pitch);
  const velocity = int(raw.velocity);
  const startTicks = dec(raw.start_ticks ?? raw.startTicks);
  const lengthTicks = dec(raw.length_ticks ?? raw.lengthTicks);
  if (
    !noteId ||
    !clipId ||
    pitch === undefined ||
    velocity === undefined ||
    startTicks === undefined ||
    lengthTicks === undefined
  ) {
    return null;
  }
  if (pitch < PITCH_MIN || pitch > PITCH_MAX) return null;
  if (velocity < VELOCITY_MIN || velocity > VELOCITY_MAX) return null;
  if (parseI64(lengthTicks) < MIN_NOTE_LENGTH) return null;
  return {
    noteId,
    clipId,
    pitch,
    velocity,
    startTicks,
    lengthTicks,
  };
}

/** Piano-roll row for a pitch (row 0 = highest pitch, C4=60 near middle). */
export function pitchRow(pitch: number): number {
  return PITCH_MAX - clampPitch(pitch);
}

/** Note px rect: x from ticks, y from pitch row, h = row height. */
export function noteRect(
  n: NoteView,
  v: TickViewport,
  z: Zoom,
  rowHeightPx: number,
): { x: number; y: number; w: number; h: number } {
  return {
    x: ticksToPx(n.startTicks, v, z),
    y: pitchRow(n.pitch) * rowHeightPx,
    w: Math.max(1, Number(parseI64(n.lengthTicks)) / z.ticksPerPixel),
    h: rowHeightPx,
  };
}

export type NoteHitZone = 'body' | 'resize-end' | null;

export function hitTestNote(
  n: NoteView,
  px: number,
  py: number,
  v: TickViewport,
  z: Zoom,
  rowHeightPx: number,
  edgePx = 5,
): NoteHitZone {
  const r = noteRect(n, v, z, rowHeightPx);
  if (py < r.y || py > r.y + r.h) return null;
  if (px < r.x || px > r.x + r.w) return null;
  const edge = Math.min(edgePx, r.w / 2);
  if (r.x + r.w - px <= edge) return 'resize-end';
  return 'body';
}

export function hitTestNotes(
  notes: NoteView[],
  px: number,
  py: number,
  v: TickViewport,
  z: Zoom,
  rowHeightPx: number,
  edgePx = 5,
): { note: NoteView; zone: Exclude<NoteHitZone, null> } | null {
  for (let i = notes.length - 1; i >= 0; i--) {
    const zone = hitTestNote(notes[i], px, py, v, z, rowHeightPx, edgePx);
    if (zone) return { note: notes[i], zone };
  }
  return null;
}

/** pitch for a row index (clamped). */
export function rowToPitch(row: number): number {
  return clampPitch(PITCH_MAX - Math.floor(row));
}

/** Is this pitch row a "black key" (for lane shading)? */
export function isBlackKey(pitch: number): boolean {
  const p = ((pitch % 12) + 12) % 12;
  return p === 1 || p === 3 || p === 6 || p === 8 || p === 10;
}

export function pitchName(pitch: number): string {
  const names = [
    'C', 'C#', 'D', 'D#', 'E', 'F', 'F#', 'G', 'G#', 'A', 'A#', 'B',
  ];
  const p = ((clampPitch(pitch) % 12) + 12) % 12;
  const octave = Math.floor(clampPitch(pitch) / 12) - 1;
  return `${names[p]}${octave}`;
}
