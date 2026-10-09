// Gesture session — the armed capture state machine (GEST-01/02/05/06).
//
//   idle → armed → capturing → preview → committed ⇢ (discard → idle)
//
// ARMING IS THE CONTRACT (T58): input only produces musical material
// while a session is armed AND capturing. A pointer/touch/keyboard event
// that arrives while idle does nothing — navigation gestures can never
// change music because the gesture layer is deaf until the user arms it.
//
// NO NOTE-ON STATE EXISTS HERE (GEST-05): the preview is a ghost layer
// (view-state-only) — it is never sent to the engine and never produces
// sounding notes, so there is nothing that could be "left on". Commit
// writes ordinary notes atomically; release()/panic() discard pending
// state and disarm, which IS the all-notes-off guarantee for this layer
// (engine-side sounding notes are the transport PANIC path's business —
// learn/panic composes both).

import { createStore, StoreApi } from 'zustand/vanilla';
import type { PersistentOp } from 'void-client';
import { contourToRawNotes, type ContourMapping } from './contour';
import { tapsToRawNotes, type RhythmOptions } from './rhythm';
import { renderGesture } from './transform';
import {
  defaultTransform,
  type GestureKind,
  type GestureNote,
  type GesturePoint,
  type PitchConstraint,
  type RawGestureNote,
  type RhythmTransform,
  type TapEvent,
} from './types';

export type GesturePhase =
  | 'idle'
  | 'armed'
  | 'capturing'
  | 'preview'
  | 'committed';

export interface GestureTarget {
  trackId: string;
  clipId: string;
}

export type GestureReleaseCause =
  | 'pointer-up'
  | 'cancel'
  | 'focus-lost'
  | 'disconnect'
  | 'panic';

export interface GestureSessionState {
  phase: GesturePhase;
  kind: GestureKind | null;
  target: GestureTarget | null;
  /** Captured contour path (contour kind). */
  points: GesturePoint[];
  /** Captured taps (rhythm kind). */
  taps: TapEvent[];
  /** Raw note derivation — rebuilt on endCapture. */
  raw: RawGestureNote[];
  /** Current transform spec (reversible — reapplied to raw). */
  transform: RhythmTransform;
  /** Active pitch constraint hook, or null = chromatic. */
  constraint: PitchConstraint | null;
  /** Ghost preview — transient; NEVER engine data. */
  preview: GestureNote[];
  /** Receipt of the last commit for undo + display. */
  lastCommit: { transactionId: string; noteIds: string[] } | null;
  lastError: string | null;
}

export interface GestureSessionActions {
  /** Arm a capture mode on a clip. Only from idle/armed — required. */
  arm(kind: GestureKind, target: GestureTarget): boolean;
  /** Begin recording input (armed → capturing). */
  beginCapture(): boolean;
  /** Feed a contour point; ignored unless capturing a contour. */
  addPoint(p: GesturePoint): void;
  /** Feed a tap; ignored unless capturing rhythm. */
  addTap(t: TapEvent): void;
  /** Finish capture → derive raw → render ghost preview. */
  endCapture(opts: {
    mapping?: ContourMapping;
    contour?: { cellTicks: string; velocity?: number };
    rhythm?: RhythmOptions;
    raw?: RawGestureNote[]; // keyboard/step-entry supplies raw directly
  }): boolean;
  /** Replace part of the transform and re-render from raw. */
  setTransform(partial: Partial<RhythmTransform>): void;
  /** Set/replace the harmonic pitch constraint; re-renders preview. */
  setConstraint(fn: PitchConstraint | null): void;
  /** Mark a successful commit (called by commit.ts after receipts). */
  markCommitted(transactionId: string, noteIds: string[]): void;
  /** Throw away pending input/preview and disarm. */
  release(cause: GestureReleaseCause): void;
  setError(message: string | null): void;
  reset(): void;
}

export type GestureSessionStore = StoreApi<
  GestureSessionState & { actions: GestureSessionActions }
>;

function initial(): GestureSessionState {
  return {
    phase: 'idle',
    kind: null,
    target: null,
    points: [],
    taps: [],
    raw: [],
    transform: defaultTransform(),
    constraint: null,
    preview: [],
    lastCommit: null,
    lastError: null,
  };
}

export function createGestureSession(
  init?: Partial<GestureSessionState>,
): GestureSessionStore {
  const base = { ...initial(), ...init };
  return createStore<GestureSessionState & { actions: GestureSessionActions }>()(
    (set, get) => ({
      ...base,
      actions: {
        arm(kind, target) {
          const { phase } = get();
          if (phase === 'capturing') return false; // finish/release first
          set({
            phase: 'armed',
            kind,
            target: { ...target },
            points: [],
            taps: [],
            raw: [],
            preview: [],
            lastCommit: null,
            lastError: null,
          });
          return true;
        },
        beginCapture() {
          if (get().phase !== 'armed') return false;
          set({ phase: 'capturing' });
          return true;
        },
        addPoint(p) {
          const s = get();
          if (s.phase !== 'capturing' || s.kind !== 'contour') return;
          set({ points: [...s.points, p] });
        },
        addTap(t) {
          const s = get();
          if (s.phase !== 'capturing' || s.kind !== 'rhythm') return;
          set({ taps: [...s.taps, t] });
        },
        endCapture(opts) {
          const s = get();
          if (s.phase !== 'capturing') return false;
          let raw: RawGestureNote[];
          if (opts.raw) {
            raw = opts.raw.map((r, i) => ({ ...r, index: i }));
          } else if (s.kind === 'contour' && opts.mapping) {
            raw = contourToRawNotes(s.points, opts.mapping, {
              cellTicks: opts.contour?.cellTicks ?? '240000',
              velocity: opts.contour?.velocity,
            });
          } else if (s.kind === 'rhythm') {
            raw = tapsToRawNotes(s.taps, {
              bpm: opts.rhythm?.bpm ?? 120,
              originTicks: opts.rhythm?.originTicks,
              pitch: opts.rhythm?.pitch,
              velocity: opts.rhythm?.velocity,
              length: opts.rhythm?.length,
              gapFallbackTicks: opts.rhythm?.gapFallbackTicks,
            });
          } else {
            set({ lastError: 'capture needs a mapping or raw input' });
            return false;
          }
          set({
            phase: raw.length > 0 ? 'preview' : 'armed',
            raw,
            preview: renderGesture(raw, s.transform, s.constraint ?? undefined),
          });
          return true;
        },
        setTransform(partial) {
          const s = get();
          const transform = { ...s.transform, ...partial };
          set({
            transform,
            // Reversible: always re-rendered from raw (T57).
            preview:
              s.phase === 'preview' || s.phase === 'committed'
                ? renderGesture(s.raw, transform, s.constraint ?? undefined)
                : s.preview,
          });
        },
        setConstraint(fn) {
          const s = get();
          set({
            constraint: fn,
            preview:
              s.phase === 'preview' || s.phase === 'committed'
                ? renderGesture(s.raw, s.transform, fn ?? undefined)
                : s.preview,
          });
        },
        markCommitted(transactionId, noteIds) {
          set({ phase: 'committed', lastCommit: { transactionId, noteIds } });
        },
        release(_cause) {
          const s = get();
          // Release is always total: pending input AND preview die, the
          // pad disarms. A committed phrase stays committed — undo is the
          // engine's path, not a hidden local revert.
          set({
            phase: 'idle',
            kind: null,
            target: s.target, // keep target visible for re-arm UX
            points: [],
            taps: [],
            raw: [],
            preview: [],
            lastError: null,
          });
        },
        setError(message) {
          set({ lastError: message });
        },
        reset() {
          set(() => ({ ...initial() }));
        },
      },
    }),
  );
}

// ---------------------------------------------------------------------------
// view-state invariant (same discipline as the other studio stores)
// ---------------------------------------------------------------------------

const ALLOWED_TOP_KEYS = new Set([
  'phase',
  'kind',
  'target',
  'points',
  'taps',
  'raw',
  'transform',
  'constraint',
  'preview',
  'lastCommit',
  'lastError',
  'actions',
]);

const FORBIDDEN_NAMES =
  /pcm|audioBuffer|waveformData|undoStack|redoStack|noteMap|songDoc|editTree/i;

function scanForbidden(
  value: unknown,
  path: string,
  seen: Set<unknown>,
): string | null {
  if (value === null || typeof value !== 'object') return null;
  if (seen.has(value)) return null;
  seen.add(value);
  if (
    value instanceof ArrayBuffer ||
    ArrayBuffer.isView(value as ArrayBufferView) ||
    typeof (value as { getChannelData?: unknown }).getChannelData === 'function'
  ) {
    return `${path}: binary/PCM-like buffer`;
  }
  if (Array.isArray(value)) {
    for (let i = 0; i < value.length; i++) {
      const r = scanForbidden(value[i], `${path}[${i}]`, seen);
      if (r) return r;
    }
    return null;
  }
  for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
    if (FORBIDDEN_NAMES.test(k)) return `${path}.${k}: forbidden field name`;
    const r = scanForbidden(v, `${path}.${k}`, seen);
    if (r) return r;
  }
  return null;
}

/** Dev/test guard: the session must stay transient input + preview only. */
export function assertGestureViewState(
  state: GestureSessionState & { actions?: unknown },
): void {
  for (const k of Object.keys(state)) {
    if (!ALLOWED_TOP_KEYS.has(k)) {
      throw new Error(`gesture session holds non-view key "${k}"`);
    }
  }
  const hit = scanForbidden(
    { ...state, actions: undefined, constraint: undefined },
    'state',
    new Set(),
  );
  if (hit) throw new Error(`song clone detected: ${hit}`);
}

/** Ops planned by a commit — one transaction, ordinary InsertNoteOps. */
export function opsForPreview(
  clipId: string,
  preview: GestureNote[],
  noteIds: string[],
): PersistentOp[] {
  return preview.map((n, i) => ({
    InsertNoteOp: {
      clip_id: clipId,
      note_id: noteIds[i],
      pitch: n.pitch,
      velocity: n.velocity,
      start_ticks: n.startTicks,
      length_ticks: n.lengthTicks,
    },
  }));
}
