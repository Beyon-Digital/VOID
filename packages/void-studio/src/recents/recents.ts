// Recent projects (W11 item 2, DOC-01 "recent-project picker").
//
// OWNERSHIP (honest): CONTRACTS.md §4 assigns the recent-project index to
// the app-private SQLite owned by the coordinator. No wire op or view
// exposes that index today — recorded in docs/engine/NEEDS.md §rev2 —
// and crates/src-tauri are outside this lane. Until the coordinator path
// lands, this store persists a *cache of real opens from this install*
// through the injected KeyValueStore (WebView localStorage in the app,
// memory in tests). It is a cache, not the authority: entries are only
// ever written after an APPLIED create/open receipt, and a stale entry
// surfaces as a failed open — never edited to look successful.
//
// The list holds reopen tokens only (container dir + project id + display
// name + timestamp). No document content, no revision, no song data.

import { createStore, StoreApi } from 'zustand/vanilla';
import { memoryStore, type KeyValueStore } from './persistence';

export const RECENTS_STORAGE_KEY = 'void.recents.v1';
export const RECENTS_MAX = 20;

export interface RecentProjectEntry {
  /** Absolute .void container directory — the reopen token for OpenProjectOp. */
  containerDir: string;
  /** Project id used to address the project when it was opened. */
  projectId: string;
  /** Display name captured at open/create time. */
  name: string;
  /** What this install last did with it. */
  kind: 'created' | 'opened';
  /** ISO-8601 timestamp of the last successful open/create. */
  lastOpenedAt: string;
}

export interface RecentsState {
  entries: RecentProjectEntry[];
  /** True once the backing store has been read (or failed open). */
  loaded: boolean;
  /** Last persistence failure — surfaced, never swallowed. */
  persistError?: string;
}

export interface RecentsActions {
  /** Record a real open/create; dedups by containerDir, most recent first. */
  recordOpen(entry: Omit<RecentProjectEntry, 'lastOpenedAt'> & { lastOpenedAt?: string }): void;
  remove(containerDir: string): void;
  clear(): void;
  /** Re-read the backing store (e.g. after another install wrote it). */
  reload(): void;
}

export type RecentsStore = StoreApi<RecentsState & { actions: RecentsActions }>;

function isEntry(v: unknown): v is RecentProjectEntry {
  if (typeof v !== 'object' || v === null) return false;
  const e = v as Record<string, unknown>;
  return (
    typeof e.containerDir === 'string' &&
    e.containerDir.length > 0 &&
    typeof e.projectId === 'string' &&
    e.projectId.length > 0 &&
    typeof e.name === 'string' &&
    typeof e.lastOpenedAt === 'string' &&
    (e.kind === 'created' || e.kind === 'opened')
  );
}

/** Defensive load: malformed JSON or bad rows degrade to fewer entries. */
export function entriesFromJson(raw: string | null): RecentProjectEntry[] {
  if (!raw) return [];
  try {
    const doc = JSON.parse(raw) as { entries?: unknown };
    const rows = Array.isArray(doc) ? doc : Array.isArray(doc.entries) ? doc.entries : [];
    return (rows as unknown[]).filter(isEntry).slice(0, RECENTS_MAX);
  } catch {
    return [];
  }
}

export function createRecentsStore(kv?: KeyValueStore, storageKey = RECENTS_STORAGE_KEY): RecentsStore {
  const backing = kv ?? memoryStore();

  const persist = (entries: RecentProjectEntry[]): string | undefined => {
    try {
      backing.setItem(storageKey, JSON.stringify({ v: 1, entries }));
      return undefined;
    } catch (e) {
      return String(e);
    }
  };

  return createStore<RecentsState & { actions: RecentsActions }>()((set, get) => ({
    entries: entriesFromJson(
      (() => {
        try {
          return backing.getItem(storageKey);
        } catch {
          return null;
        }
      })(),
    ),
    loaded: true,
    persistError: undefined,
    actions: {
      recordOpen: (entry) =>
        set(() => {
          const next: RecentProjectEntry = {
            ...entry,
            lastOpenedAt: entry.lastOpenedAt ?? new Date().toISOString(),
          };
          const entries = [
            next,
            ...get().entries.filter((e) => e.containerDir !== entry.containerDir),
          ].slice(0, RECENTS_MAX);
          return { entries, persistError: persist(entries) };
        }),
      remove: (containerDir) =>
        set(() => {
          const entries = get().entries.filter((e) => e.containerDir !== containerDir);
          return { entries, persistError: persist(entries) };
        }),
      clear: () =>
        set(() => {
          const entries: RecentProjectEntry[] = [];
          return { entries, persistError: persist(entries) };
        }),
      reload: () =>
        set(() => {
          try {
            return { entries: entriesFromJson(backing.getItem(storageKey)), loaded: true };
          } catch (e) {
            return { loaded: true, persistError: String(e) };
          }
        }),
    },
  }));
}
