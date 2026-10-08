// void-studio workspace state.
//
// CONTRACTS.md ownership rule: the WebView holds ONLY transient view state —
// selection, viewport, panels, focus, acknowledged cache. It NEVER holds a
// mutable copy of the musical document, PCM, plugin state, or its own undo
// stack. Project data flows in through bounded read_view pages; edits flow
// out as PersistentCommands; undo is issued to the engine (UndoOp/RedoOp).

import { createStore, StoreApi } from 'zustand/vanilla';
import type {
  ClockSnapshot,
  MeterFrame,
  ReadItem,
  ReadResponse,
  SaveResultEvent,
} from 'void-client';
import { Zoom, TickViewport, makeViewport, panByPx, zoomAtPx } from './viewport';

/** Server-side page bound (CONTRACTS.md §2): <=2000 items or 512KiB. */
export const VIEW_PAGE_ITEM_MAX = 2000;
/** Max pages kept per view key in the view cache. */
export const VIEW_CACHE_PAGE_MAX = 64;

export type PanelId = 'browser' | 'inspector' | 'mixer' | 'editor' | 'meters';

/** One bounded read-view page cache entry (a projection, not the document). */
export interface ReadViewEntry {
  items: ReadItem[];
  revision: string;
  nextCursor: string;
  done: boolean;
  truncated: boolean;
  /** total items received across merged pages (for diagnostics) */
  received: number;
}

export interface Selection {
  trackId: string | null;
  clipId: string | null;
  noteId: string | null;
}

export interface EngineView {
  attached: boolean;
  workerId?: string;
  epoch?: string;
  state?: string;
}

export interface FocusState {
  /** Region currently holding keyboard focus (panel id / component id). */
  region: string;
  /** CSS selector or element key to restore focus to after a modal/overlay. */
  restoreTarget?: string;
}

export interface TelemetryView {
  clock?: ClockSnapshot;
  /** Latest meter frame per track_id (transient, replaced not accumulated). */
  meters: Record<string, MeterFrame>;
  save?: SaveResultEvent;
}

/**
 * The whole studio state. Everything here is view state: IDs, ranges,
 * flags, and bounded projections of engine data. There is deliberately no
 * field for tracks/clips/notes as a document — the store is not a song
 * clone. `views` holds bounded pages keyed by view descriptor.
 */
export interface StudioViewState {
  engine: EngineView;
  /** Project the workspace addresses; '' when none is open. */
  projectId: string;
  /** Last coordinator-acknowledged revision (decimal string). */
  revision: string;
  selection: Selection;
  viewport: TickViewport;
  zoom: Zoom;
  panels: Record<PanelId, boolean>;
  focus: FocusState;
  /** Bounded read projections keyed by makeViewKey(). */
  views: Record<string, ReadViewEntry>;
  telemetry: TelemetryView;
}

export interface StudioActions {
  setEngine(next: EngineView): void;
  setProject(projectId: string, revision?: string): void;
  setRevision(revision: string): void;

  select(sel: Partial<Selection>): void;
  selectTrack(trackId: string | null): void;
  selectClip(trackId: string, clipId: string | null): void;
  selectNote(trackId: string, clipId: string, noteId: string | null): void;

  setViewport(startTicks: string, endTicks: string): void;
  panViewportByPx(pxDelta: number): void;
  zoomViewportAtPx(factor: number, anchorPx: number): void;
  setTicksPerPixel(ticksPerPixel: number): void;

  setPanel(panel: PanelId, open: boolean): void;
  togglePanel(panel: PanelId): void;
  setFocusedRegion(region: string, restoreTarget?: string): void;

  /** Merge one read page into the bounded cache for key. */
  mergeReadPage(key: string, page: ReadResponse): void;
  /** Drop all projections — required on project/epoch change. */
  clearProjections(): void;

  applyTelemetry(ev: ClockSnapshot | MeterFrame | SaveResultEvent): void;
  reset(): void;
}

export type StudioStore = StoreApi<StudioViewState & { actions: StudioActions }>;

export const DEFAULT_PANELS: Record<PanelId, boolean> = {
  browser: false,
  inspector: true,
  mixer: false,
  editor: true,
  meters: true,
};

export function initialState(): StudioViewState {
  return {
    engine: { attached: false },
    projectId: '',
    revision: '0',
    selection: { trackId: null, clipId: null, noteId: null },
    viewport: makeViewport('0', (16n * 3840000n).toString(10)), // 16 bars of 4/4
    zoom: { ticksPerPixel: 48000 }, // ~20px per quarter note
    panels: { ...DEFAULT_PANELS },
    focus: { region: 'timeline' },
    views: {},
    telemetry: { meters: {} },
  };
}

export function createStudioStore(init?: Partial<StudioViewState>): StudioStore {
  const base = { ...initialState(), ...init };
  return createStore<StudioViewState & { actions: StudioActions }>()((set, get) => ({
    ...base,
    actions: {
      setEngine: (next) =>
        set((s) => {
          const epochChanged =
            next.epoch !== undefined && s.engine.epoch !== undefined && next.epoch !== s.engine.epoch;
          const detach = s.engine.attached && !next.attached;
          return {
            engine: next,
            // A new epoch or a detach invalidates every projection.
            views: epochChanged || detach ? {} : s.views,
            revision: epochChanged ? '0' : s.revision,
            telemetry: epochChanged || detach ? { meters: {} } : s.telemetry,
          };
        }),
      setProject: (projectId, revision) =>
        set((s) =>
          projectId === s.projectId
            ? { projectId, revision: revision ?? s.revision }
            : { projectId, revision: revision ?? '0', views: {}, telemetry: { meters: {} } },
        ),
      setRevision: (revision) => set({ revision }),

      select: (sel) =>
        set((s) => ({ selection: { ...s.selection, ...sel } })),
      selectTrack: (trackId) =>
        set({ selection: { trackId, clipId: null, noteId: null } }),
      selectClip: (trackId, clipId) =>
        set({ selection: { trackId, clipId, noteId: null } }),
      selectNote: (trackId, clipId, noteId) =>
        set({ selection: { trackId, clipId, noteId } }),

      setViewport: (startTicks, endTicks) =>
        set({ viewport: makeViewport(startTicks, endTicks) }),
      panViewportByPx: (pxDelta) =>
        set((s) => ({ viewport: panByPx(s.viewport, s.zoom, pxDelta) })),
      zoomViewportAtPx: (factor, anchorPx) =>
        set((s) => {
          const r = zoomAtPx(s.viewport, s.zoom, factor, anchorPx);
          return { viewport: r.viewport, zoom: r.zoom };
        }),
      setTicksPerPixel: (ticksPerPixel) =>
        set((s) => ({ zoom: { ticksPerPixel: Math.min(1e9, Math.max(1e-3, ticksPerPixel)) }, viewport: s.viewport })),

      setPanel: (panel, open) =>
        set((s) => ({ panels: { ...s.panels, [panel]: open } })),
      togglePanel: (panel) =>
        set((s) => ({ panels: { ...s.panels, [panel]: !s.panels[panel] } })),
      setFocusedRegion: (region, restoreTarget) =>
        set({ focus: { region, restoreTarget } }),

      mergeReadPage: (key, page) =>
        set((s) => {
          const prev = s.views[key] ?? {
            items: [],
            revision: '0',
            nextCursor: '',
            done: false,
            truncated: false,
            received: 0,
          };
          const items = prev.items.concat(page.items);
          const truncated =
            prev.truncated || items.length > VIEW_PAGE_ITEM_MAX;
          return {
            views: {
              ...s.views,
              [key]: {
                items: truncated ? items.slice(0, VIEW_PAGE_ITEM_MAX) : items,
                revision: page.revision,
                nextCursor: page.next_cursor,
                done: page.done,
                truncated,
                received: prev.received + page.items.length,
              },
            },
          };
        }),
      clearProjections: () => set({ views: {} }),

      applyTelemetry: (ev) =>
        set((s) => {
          if (ev.kind === 'ClockSnapshot') {
            return { telemetry: { ...s.telemetry, clock: ev } };
          }
          if (ev.kind === 'MeterFrame') {
            return {
              telemetry: {
                ...s.telemetry,
                meters: { ...s.telemetry.meters, [ev.track_id]: ev },
              },
            };
          }
          if (ev.kind === 'SaveResultEvent') {
            return { telemetry: { ...s.telemetry, save: ev } };
          }
          return s;
        }),

      reset: () => set(() => ({ ...initialState() })),
    },
  }));
}

/** Stable key for a bounded view cache entry. */
export function makeViewKey(
  view: string,
  trackId?: string,
  startTicks?: string,
  endTicks?: string,
): string {
  return [view, trackId ?? '', startTicks ?? '', endTicks ?? ''].join('|');
}

// ---------------------------------------------------------------------------
// no-song-clone invariant
// ---------------------------------------------------------------------------

const ALLOWED_TOP_KEYS = new Set([
  'engine',
  'projectId',
  'revision',
  'selection',
  'viewport',
  'zoom',
  'panels',
  'focus',
  'views',
  'telemetry',
  'actions',
]);

/** Patterns that indicate musical-document or raw-media state sneaking in. */
const FORBIDDEN_NAMES = /pcm|audioBuffer|waveformData|undoStack|redoStack|noteMap|songDoc|editTree/i;

function scanForbidden(value: unknown, path: string, seen: Set<unknown>): string | null {
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

/**
 * Development/test guard: throws when the state tree violates the
 * WebView-owns-only-view-state rule. Checks:
 *  - no unexpected top-level keys (e.g. someone adding `song`)
 *  - bounded page caches never exceed VIEW_PAGE_ITEM_MAX
 *  - no PCM/audio buffers/undo stacks anywhere in the tree
 */
export function assertViewStateOnly(
  state: StudioViewState & { actions?: unknown },
): void {
  for (const k of Object.keys(state)) {
    if (!ALLOWED_TOP_KEYS.has(k)) {
      throw new Error(`studio state holds non-view key "${k}"`);
    }
  }
  for (const [key, entry] of Object.entries(state.views)) {
    if (!Array.isArray(entry.items)) {
      throw new Error(`view "${key}" is not a read page`);
    }
    if (entry.items.length > VIEW_PAGE_ITEM_MAX) {
      throw new Error(`view "${key}" exceeds ${VIEW_PAGE_ITEM_MAX} items`);
    }
  }
  const hit = scanForbidden(
    { ...state, actions: undefined },
    'state',
    new Set(),
  );
  if (hit) throw new Error(`song clone detected: ${hit}`);
}
