// Minimal key/value persistence port for app-local caches (W11).
//
// The coordinator owns durable app metadata per CONTRACTS.md §4
// (app-private SQLite). Until a coordinator path exists on the wire —
// recorded in docs/engine/NEEDS.md §rev2 — W11 app-level features persist
// their caches through this port. The shipped adapter is WebView-local
// storage: an honest *cache*, never the authority, and never a separate
// save format for document data.

/** The subset of Web Storage a cache needs — `localStorage` fits. */
export interface KeyValueStore {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
  removeItem(key: string): void;
}

/** In-memory store for tests and for environments without Web Storage. */
export function memoryStore(): KeyValueStore {
  const map = new Map<string, string>();
  return {
    getItem: (k) => (map.has(k) ? map.get(k)! : null),
    setItem: (k, v) => void map.set(k, v),
    removeItem: (k) => void map.delete(k),
  };
}

/**
 * The WebView's own localStorage when present; memory otherwise. The
 * fallback keeps tests and plain-browser previews honest — callers are
 * told which backing they got via `isPersistent`.
 */
export function webStore(): { store: KeyValueStore; persistent: boolean } {
  try {
    const ls = (globalThis as { localStorage?: KeyValueStore }).localStorage;
    if (ls) {
      // Probe once — some contexts expose a throwing localStorage.
      const probe = '__void_probe__';
      ls.setItem(probe, '1');
      ls.removeItem(probe);
      return { store: ls, persistent: true };
    }
  } catch {
    /* fall through to memory */
  }
  return { store: memoryStore(), persistent: false };
}
