// Editor view-state store (lane-owned half of the WebView's state).
//
// Same ownership rule as store.ts: ONLY transient view state — workspace
// id, snap settings, multi-selection, an optimistic drag preview, the last
// surfaced edit error. No document clones, no PCM, no undo stack; the
// invariant below is the same scanner the base store uses.

import { createStore, StoreApi } from 'zustand/vanilla';
import type { WorkspaceId } from './workspaces';

// -- snap --------------------------------------------------------------------

export type SnapDivision = 'bar' | 'beat' | '1/2' | '1/4' | '1/8' | '1/16' | '1/64';

export interface SnapSettings {
  enabled: boolean;
  division: SnapDivision;
  beatsPerBar: number;
  /**
   * tempo_map_revision these settings were derived under (ClockSnapshot).
   * When the engine's tempo map revision moves on, the shell re-derives
   * the grid — a stale revision flags that the grid may not match.
   */
  tempoMapRevision: string;
}

// -- drag preview ------------------------------------------------------------

export type DragMode =
  | 'move'
  | 'trim-start'
  | 'trim-end'
  | 'split'
  | 'duplicate';

/** Optimistic transient preview — reverted when the command fails. */
export interface DragPreview {
  mode: DragMode;
  clipId: string;
  trackId: string;
  /** transaction_id for the whole gesture (one undo step engine-side). */
  transactionId: string;
  startTicks: string;
  lengthTicks: string;
  offsetTicks?: string;
  /** split cursor position for mode 'split'. */
  atTicks?: string;
  /** target track for a cross-track move. */
  targetTrackId?: string;
}

// -- selection ---------------------------------------------------------------

export interface ClipSelection {
  trackId: string | null;
  clipIds: string[];
}

export interface NoteSelection {
  clipId: string | null;
  noteIds: string[];
}

// -- store -------------------------------------------------------------------

export interface EditorViewState {
  workspace: WorkspaceId;
  snap: SnapSettings;
  clipSelection: ClipSelection;
  noteSelection: NoteSelection;
  drag: DragPreview | null;
  /** One pending gesture at a time — its transaction id, or null. */
  pendingGesture: string | null;
  /** Last surfaced command failure (safe coordinator text). */
  lastEditError: string | null;
}

export interface EditorActions {
  setWorkspace(id: WorkspaceId): void;
  setSnap(partial: Partial<SnapSettings>): void;

  selectClips(trackId: string, clipIds: string[]): void;
  toggleClip(trackId: string, clipId: string): void;
  clearClipSelection(): void;

  selectNotes(clipId: string, noteIds: string[]): void;
  toggleNote(clipId: string, noteId: string): void;
  clearNoteSelection(): void;

  setDrag(drag: DragPreview | null): void;
  setPendingGesture(transactionId: string | null): void;
  setEditError(message: string | null): void;

  reset(): void;
}

export type EditorStore = StoreApi<
  EditorViewState & { actions: EditorActions }
>;

export function initialEditorState(): EditorViewState {
  return {
    workspace: 'arrange',
    snap: {
      enabled: true,
      division: 'beat',
      beatsPerBar: 4,
      tempoMapRevision: '',
    },
    clipSelection: { trackId: null, clipIds: [] },
    noteSelection: { clipId: null, noteIds: [] },
    drag: null,
    pendingGesture: null,
    lastEditError: null,
  };
}

export function createEditorStore(
  init?: Partial<EditorViewState>,
): EditorStore {
  const base = { ...initialEditorState(), ...init };
  return createStore<EditorViewState & { actions: EditorActions }>()(
    (set, get) => ({
      ...base,
      actions: {
        setWorkspace: (id) => set({ workspace: id }),

        setSnap: (partial) =>
          set((s) => ({ snap: { ...s.snap, ...partial } })),

        selectClips: (trackId, clipIds) =>
          set({
            clipSelection: { trackId, clipIds: [...new Set(clipIds)] },
          }),
        toggleClip: (trackId, clipId) =>
          set((s) => {
            const sel =
              s.clipSelection.trackId === trackId ? s.clipSelection.clipIds : [];
            const next = sel.includes(clipId)
              ? sel.filter((c) => c !== clipId)
              : [...sel, clipId];
            return { clipSelection: { trackId, clipIds: next } };
          }),
        clearClipSelection: () =>
          set({ clipSelection: { trackId: null, clipIds: [] } }),

        selectNotes: (clipId, noteIds) =>
          set({
            noteSelection: { clipId, noteIds: [...new Set(noteIds)] },
          }),
        toggleNote: (clipId, noteId) =>
          set((s) => {
            const sel =
              s.noteSelection.clipId === clipId ? s.noteSelection.noteIds : [];
            const next = sel.includes(noteId)
              ? sel.filter((n) => n !== noteId)
              : [...sel, noteId];
            return { noteSelection: { clipId, noteIds: next } };
          }),
        clearNoteSelection: () =>
          set({ noteSelection: { clipId: null, noteIds: [] } }),

        setDrag: (drag) => set({ drag }),
        setPendingGesture: (transactionId) =>
          set({ pendingGesture: transactionId }),
        setEditError: (message) => set({ lastEditError: message }),

        reset: () => set(() => ({ ...initialEditorState() })),
      },
    }),
  );
}

/** App-level singleton — tests construct their own via createEditorStore. */
export const editorStore: EditorStore = createEditorStore();

// ---------------------------------------------------------------------------
// view-state invariant (same discipline as assertViewStateOnly)
// ---------------------------------------------------------------------------

const ALLOWED_TOP_KEYS = new Set([
  'workspace',
  'snap',
  'clipSelection',
  'noteSelection',
  'drag',
  'pendingGesture',
  'lastEditError',
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
    typeof (value as { getChannelData?: unknown }).getChannelData ===
      'function'
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

/** Dev/test guard: throws when the editor store holds non-view state. */
export function assertEditorViewState(
  state: EditorViewState & { actions?: unknown },
): void {
  for (const k of Object.keys(state)) {
    if (!ALLOWED_TOP_KEYS.has(k)) {
      throw new Error(`editor state holds non-view key "${k}"`);
    }
  }
  const hit = scanForbidden(
    { ...state, actions: undefined },
    'state',
    new Set(),
  );
  if (hit) throw new Error(`song clone detected: ${hit}`);
}
