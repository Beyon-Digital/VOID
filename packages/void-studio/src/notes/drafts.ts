// Unsent note drafts (W11, DOC-05).
//
// With no note ops in protocol major.1, a note the user types cannot be
// committed to the document. This store keeps that *unsent intent* as an
// explicit draft — CONTRACTS §3's "recovered unsaved intent" pattern at
// app scope — persisted app-locally via the injected KeyValueStore.
//
// A draft is NEVER a document value: views parse the real note through
// parse.ts and the UI labels drafts "unsent". Once SetProjectNoteOp /
// SetTrackNoteOp land, applying a draft is one sendCommand + clearDraft.

import { createStore, StoreApi } from 'zustand/vanilla';
import { memoryStore, type KeyValueStore } from '../recents/persistence';

export const NOTE_DRAFTS_STORAGE_KEY = 'void.note-drafts.v1';

/** Draft key for a project-level note. */
export function projectNoteKey(projectId: string): string {
  return `proj:${projectId}`;
}
/** Draft key for a track-level note. */
export function trackNoteKey(projectId: string, trackId: string): string {
  return `trk:${projectId}:${trackId}`;
}

export interface NoteDraft {
  /** Draft key — `proj:<projectId>` or `trk:<projectId>:<trackId>`. */
  key: string;
  text: string;
  /** ISO-8601 — set when the draft was last edited. */
  updatedAt: string;
}

export interface NoteDraftsState {
  drafts: Record<string, NoteDraft>;
  loaded: boolean;
  persistError?: string;
}

export interface NoteDraftsActions {
  setDraft(key: string, text: string): void;
  clearDraft(key: string): void;
  reload(): void;
}

export type NoteDraftsStore = StoreApi<NoteDraftsState & { actions: NoteDraftsActions }>;

function isDraft(v: unknown): v is NoteDraft {
  if (typeof v !== 'object' || v === null) return false;
  const d = v as Record<string, unknown>;
  return (
    typeof d.key === 'string' &&
    d.key.length > 0 &&
    typeof d.text === 'string' &&
    typeof d.updatedAt === 'string'
  );
}

export function draftsFromJson(raw: string | null): Record<string, NoteDraft> {
  if (!raw) return {};
  try {
    const doc = JSON.parse(raw) as { drafts?: unknown };
    const rows = Array.isArray(doc.drafts) ? doc.drafts : [];
    const out: Record<string, NoteDraft> = {};
    for (const r of rows as unknown[]) if (isDraft(r)) out[r.key] = r;
    return out;
  } catch {
    return {};
  }
}

export function createNoteDraftsStore(
  kv?: KeyValueStore,
  storageKey = NOTE_DRAFTS_STORAGE_KEY,
): NoteDraftsStore {
  const backing = kv ?? memoryStore();

  const persist = (drafts: Record<string, NoteDraft>): string | undefined => {
    try {
      backing.setItem(storageKey, JSON.stringify({ v: 1, drafts: Object.values(drafts) }));
      return undefined;
    } catch (e) {
      return String(e);
    }
  };

  const load = (): Record<string, NoteDraft> => {
    try {
      return draftsFromJson(backing.getItem(storageKey));
    } catch {
      return {};
    }
  };

  return createStore<NoteDraftsState & { actions: NoteDraftsActions }>()((set, get) => ({
    drafts: load(),
    loaded: true,
    persistError: undefined,
    actions: {
      setDraft: (key, text) =>
        set(() => {
          const drafts = {
            ...get().drafts,
            [key]: { key, text, updatedAt: new Date().toISOString() },
          };
          return { drafts, persistError: persist(drafts) };
        }),
      clearDraft: (key) =>
        set(() => {
          const drafts = { ...get().drafts };
          delete drafts[key];
          return { drafts, persistError: persist(drafts) };
        }),
      reload: () =>
        set(() => {
          try {
            return { drafts: load(), loaded: true };
          } catch (e) {
            return { loaded: true, persistError: String(e) };
          }
        }),
    },
  }));
}
