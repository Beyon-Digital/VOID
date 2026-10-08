// Take/comp view store (W17/REC-04, REC-05, REC-06; T66).
//
// View-state only: folders, comp specs, flashback policy state,
// selection inside the comp editor and the last applied transaction
// (for undo targeting). No PCM, no second song document — the applied
// comp lives in the engine as ordinary clips.

import { createStore, StoreApi } from 'zustand/vanilla';
import { cycleTakeAt, validateComp } from './comp';
import { takeAt } from './takes';
import type { CompSpec, TakeFolder, TakeRecord } from './types';
import type { FlashbackBuffer } from './capture';
import { disableBuffer, enableBuffer, pushChunk, type CaptureChunk } from './capture';

export interface TakeStudioState {
  /** Take folders keyed by folderId (recorded stack per region). */
  folders: Record<string, TakeFolder>;
  /** Comp specs keyed by compId. */
  comps: Record<string, CompSpec>;
  /** Comp currently open in the editor (id), or null. */
  openCompId: string | null;
  /** Flashback buffers keyed by bufferId. */
  buffers: Record<string, FlashbackBuffer>;
  /** Selected comp segment under the cursor. */
  selection: { compId: string; segmentId: string } | null;
  /** Audition focus: the take currently shown as "playing" in a lane
   *  (display state — real audition needs the engine preview layer,
   *  NEEDS §12). */
  auditionTakeId: string | null;
  /** Last applied comp transaction — for undo targeting + tests. */
  lastApply: { compId: string; transactionId: string } | null;
  lastError: string | null;
}

export interface TakeStudioActions {
  upsertFolder(folder: TakeFolder): void;
  removeFolder(folderId: string): void;
  upsertComp(spec: CompSpec): void;
  openComp(compId: string | null): void;
  selectSegment(compId: string, segmentId: string | null): void;
  /** Cycle the take at a timeline position inside the open comp. */
  cycleAt(compId: string, ticks: string, direction: 1 | -1): void;
  audition(takeId: string | null): void;
  markApplied(compId: string, transactionId: string): void;
  setError(msg: string | null): void;
  /** Flashback policy actions. */
  upsertBuffer(b: FlashbackBuffer): void;
  enableBuffer(bufferId: string): void;
  disableBuffer(bufferId: string): void;
  pushChunk(bufferId: string, chunk: CaptureChunk): void;
  reset(): void;
}

export type TakeStudioStore = StoreApi<
  TakeStudioState & { actions: TakeStudioActions }
>;

function initial(): TakeStudioState {
  return {
    folders: {},
    comps: {},
    openCompId: null,
    buffers: {},
    selection: null,
    auditionTakeId: null,
    lastApply: null,
    lastError: null,
  };
}

export function createTakeStudioStore(
  init?: Partial<TakeStudioState>,
): TakeStudioStore {
  const base = { ...initial(), ...init };
  return createStore<TakeStudioState & { actions: TakeStudioActions }>()(
    (set, get) => ({
      ...base,
      actions: {
        upsertFolder(folder) {
          set((s) => ({
            folders: { ...s.folders, [folder.folderId]: folder },
          }));
        },
        removeFolder(folderId) {
          set((s) => {
            const folders = { ...s.folders };
            delete folders[folderId];
            return { folders };
          });
        },
        upsertComp(spec) {
          set((s) => ({ comps: { ...s.comps, [spec.compId]: spec } }));
        },
        openComp(compId) {
          set({ openCompId: compId, selection: null });
        },
        selectSegment(compId, segmentId) {
          set({
            selection: segmentId ? { compId, segmentId } : null,
          });
        },
        cycleAt(compId, ticks, direction) {
          const s = get();
          const spec = s.comps[compId];
          if (!spec) return;
          // The comp's folder is the one containing its segment takes —
          // folders are keyed by folderId, so resolve by membership.
          const folder = Object.values(s.folders).find((f) =>
            spec.segments.every((seg) =>
              f.takes.some((t) => t.takeId === seg.takeId),
            ),
          );
          if (!folder) return;
          const next = cycleTakeAt(spec, folder, ticks, direction);
          if (next) {
            const errors = validateComp(next, folder);
            if (errors.length === 0) {
              set({ comps: { ...s.comps, [compId]: next } });
            }
          }
        },
        audition(takeId) {
          set({ auditionTakeId: takeId });
        },
        markApplied(compId, transactionId) {
          set({ lastApply: { compId, transactionId } });
        },
        setError(msg) {
          set({ lastError: msg });
        },
        upsertBuffer(b) {
          set((s) => ({ buffers: { ...s.buffers, [b.bufferId]: b } }));
        },
        enableBuffer(bufferId) {
          const s = get();
          const b = s.buffers[bufferId];
          if (!b) return;
          try {
            set({ buffers: { ...s.buffers, [bufferId]: enableBuffer(b) } });
          } catch (e) {
            set({ lastError: (e as Error).message });
          }
        },
        disableBuffer(bufferId) {
          const s = get();
          const b = s.buffers[bufferId];
          if (!b) return;
          set({ buffers: { ...s.buffers, [bufferId]: disableBuffer(b) } });
        },
        pushChunk(bufferId, chunk) {
          const s = get();
          const b = s.buffers[bufferId];
          if (!b) return;
          set({ buffers: { ...s.buffers, [bufferId]: pushChunk(b, chunk) } });
        },
        reset() {
          set(() => ({ ...initial() }));
        },
      },
    }),
  );
}

/** Take under a position in a folder (UI convenience). */
export function takeAtPosition(
  store: TakeStudioStore,
  folderId: string,
  ticks: string,
): TakeRecord | null {
  const folder = store.getState().folders[folderId];
  return folder ? takeAt(folder, ticks) : null;
}
