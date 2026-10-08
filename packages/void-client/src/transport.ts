// Transport abstraction over the Tauri IPC surface.
//
// `window.__TAURI__.core.invoke` / `window.__TAURI__.event` are the raw
// injection points; when the global is absent (globalTauri not enabled in
// tauri.conf.json) we fall back to the @tauri-apps/api npm package, which
// talks to the same injected internals. Either way the client only sees
// VoidTransport, so tests can inject a fake invoke and run fully headless.

export type UnlistenFn = () => void;

export interface VoidTransport {
  /** invoke a #[tauri::command] handler. */
  invoke<T = unknown>(cmd: string, args?: Record<string, unknown>): Promise<T>;
  /** subscribe to an emitted event; resolves to an unlisten function. */
  listen<T = unknown>(
    event: string,
    handler: (payload: T) => void,
  ): Promise<UnlistenFn>;
}

interface TauriCoreLike {
  invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T>;
}
interface TauriEventLike {
  listen<T>(
    event: string,
    handler: (evt: { payload: T }) => void,
  ): Promise<UnlistenFn>;
}
interface TauriGlobalLike {
  core?: TauriCoreLike;
  event?: TauriEventLike;
}

function tauriGlobal(): TauriGlobalLike | undefined {
  const g = globalThis as { __TAURI__?: TauriGlobalLike };
  return g.__TAURI__;
}

async function npmInvoke<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T> {
  const mod = await import('@tauri-apps/api/core');
  return mod.invoke<T>(cmd as never, args);
}

async function npmListen<T>(
  event: string,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  const mod = await import('@tauri-apps/api/event');
  return mod.listen<T>(event, (e) => handler(e.payload));
}

/** Transport bound to the real Tauri runtime (global or npm API). */
export function createTauriTransport(): VoidTransport {
  return {
    invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
      const t = tauriGlobal();
      if (t?.core?.invoke) return t.core.invoke<T>(cmd, args);
      return npmInvoke<T>(cmd, args);
    },
    listen<T>(
      event: string,
      handler: (payload: T) => void,
    ): Promise<UnlistenFn> {
      const t = tauriGlobal();
      if (t?.event?.listen) {
        return t.event.listen<T>(event, (e) => handler(e.payload));
      }
      return npmListen<T>(event, handler);
    },
  };
}

/**
 * Headless transport for tests: records every invoke, lets the test decide
 * each reply, and can emit synthetic bus events. No Tauri runtime needed.
 */
export class FakeTransport implements VoidTransport {
  readonly calls: { cmd: string; args?: Record<string, unknown> }[] = [];
  private responders = new Map<
    string,
    (args?: Record<string, unknown>) => unknown | Promise<unknown>
  >();
  private listeners = new Map<string, Set<(payload: unknown) => void>>();

  /** Queue a responder for a command name; return value is the invoke result. */
  respond(
    cmd: string,
    fn: (args?: Record<string, unknown>) => unknown | Promise<unknown>,
  ): void {
    this.responders.set(cmd, fn);
  }

  invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
    this.calls.push({ cmd, args });
    const r = this.responders.get(cmd);
    if (!r) return Promise.reject(new Error(`FakeTransport: no responder for ${cmd}`));
    return Promise.resolve(r(args) as T);
  }

  listen<T>(event: string, handler: (payload: T) => void): Promise<UnlistenFn> {
    let set = this.listeners.get(event);
    if (!set) {
      set = new Set();
      this.listeners.set(event, set);
    }
    const fn = handler as (payload: unknown) => void;
    set.add(fn);
    return Promise.resolve(() => set.delete(fn));
  }

  /** Deliver a synthetic event to all subscribers of `event`. */
  emit(event: string, payload: unknown): void {
    for (const fn of this.listeners.get(event) ?? []) fn(payload);
  }
}
