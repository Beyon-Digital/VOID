// Keymap store (W21) — customizable bindings over an immutable default
// map. Rules enforced here:
//   * DEFAULT_MAP is frozen — "reset to defaults" is a real restore,
//     not a rebuild of hand-edited defaults;
//   * conflict detection: binding → exactly one command. A rebind that
//     collides reports the conflict; `resolveConflict` chooses whether
//     the incumbent or the new command keeps the key (the loser is
//     unbound, never silently shared);
//   * export/import is a versioned JSON document, parse-defensive.

import { createStore, StoreApi } from 'zustand/vanilla';
import {
  bindingKey,
  formatBinding,
  parseBinding,
  type KeyBinding,
} from './types';

export const KEYMAP_FORMAT_VERSION = 1;
export const MAX_BINDINGS = 512;

export interface CommandInfo {
  commandId: string;
  /** Display name for palette/help. */
  title: string;
  /** e.g. 'transport' | 'edit' | 'view' — grouping for the UI. */
  section: string;
}

export interface BindingConflict {
  binding: string;
  /** The two competing command ids (winner is `map[binding]`). */
  current: string;
  incoming: string;
}

interface KeymapData {
  version: number;
  /** commandId → binding; user layer only. */
  overrides: Record<string, KeyBinding>;
  /** commandIds the user deliberately unbound. */
  unbound: string[];
}

export interface KeymapViewState {
  overrides: Record<string, KeyBinding>;
  unbound: ReadonlySet<string>;
  /** Registered commands (the app's vocabulary). */
  commands: Record<string, CommandInfo>;
  /** Last conflict awaiting resolution (UI shows a chooser). */
  pendingConflict: BindingConflict | null;
}

export interface KeymapActions {
  registerCommands(list: CommandInfo[]): void;
  /** Effective binding for a command (override ?? default). */
  bindingFor(commandId: string): KeyBinding | null;
  /** Effective command for a binding keystroke string. */
  commandFor(stroke: string): string | null;
  /** Bind a stroke to a command. Returns the conflict to resolve, or
   * null when the bind applied cleanly. */
  bind(stroke: string, commandId: string): BindingConflict | null;
  /** After a conflict, keep the incoming bind (true) or keep the
   * incumbent (false). */
  resolveConflict(keepIncoming: boolean): void;
  unbind(commandId: string): void;
  resetToDefaults(): void;
  exportKeymap(): string;
  importKeymap(json: string): { ok: boolean; error?: string };
}

export type KeymapStore = StoreApi<
  KeymapViewState & { actions: KeymapActions }
>;

/** The shipped default map — the single source of defaults. Frozen:
 * user edits never touch this object (immutability is structural). */
export const DEFAULT_COMMANDS: readonly CommandInfo[] = Object.freeze([
  { commandId: 'transport.play', title: 'Play', section: 'transport' },
  { commandId: 'transport.stop', title: 'Stop', section: 'transport' },
  { commandId: 'transport.record', title: 'Record', section: 'transport' },
  { commandId: 'transport.loop', title: 'Toggle loop', section: 'transport' },
  { commandId: 'edit.undo', title: 'Undo', section: 'edit' },
  { commandId: 'edit.redo', title: 'Redo', section: 'edit' },
  { commandId: 'edit.cut', title: 'Cut', section: 'edit' },
  { commandId: 'edit.copy', title: 'Copy', section: 'edit' },
  { commandId: 'edit.paste', title: 'Paste', section: 'edit' },
  { commandId: 'edit.selectAll', title: 'Select all', section: 'edit' },
  { commandId: 'edit.delete', title: 'Delete selection', section: 'edit' },
  { commandId: 'view.zoomIn', title: 'Zoom in', section: 'view' },
  { commandId: 'view.zoomOut', title: 'Zoom out', section: 'view' },
  { commandId: 'view.commandPalette', title: 'Command palette', section: 'view' },
  { commandId: 'produce.accompaniment', title: 'Generate accompaniment', section: 'produce' },
  { commandId: 'produce.variation', title: 'Generate variation', section: 'produce' },
] as const);

const DEFAULT_BINDINGS: Readonly<Record<string, string>> = Object.freeze({
  'transport.play': 'Space',
  'transport.stop': 'Ctrl+Space',
  'transport.record': 'R',
  'transport.loop': 'L',
  'edit.undo': 'Ctrl+Z',
  'edit.redo': 'Ctrl+Shift+Z',
  'edit.cut': 'Ctrl+X',
  'edit.copy': 'Ctrl+C',
  'edit.paste': 'Ctrl+V',
  'edit.selectAll': 'Ctrl+A',
  'edit.delete': 'Delete',
  'view.zoomIn': 'Ctrl+=',
  'view.zoomOut': 'Ctrl+-',
  'view.commandPalette': 'Ctrl+Shift+P',
  'produce.accompaniment': 'Ctrl+Shift+A',
  'produce.variation': 'Ctrl+Shift+V',
} as const);

const DEFAULT_MAP: Readonly<Record<string, KeyBinding>> = Object.freeze(
  Object.fromEntries(
    Object.entries(DEFAULT_BINDINGS).map(([cmd, s]) => [cmd, parseBinding(s)]),
  ),
);

function effective(
  commands: Record<string, CommandInfo>,
  overrides: Record<string, KeyBinding>,
  unbound: ReadonlySet<string>,
): Map<string, string> {
  // bindingKey → commandId (defaults first, overrides replace, unbound removes)
  const byCmd = new Map<string, KeyBinding | null>();
  for (const id of Object.keys(commands)) {
    byCmd.set(id, DEFAULT_MAP[id] ?? null);
  }
  for (const [cmd, b] of Object.entries(overrides)) byCmd.set(cmd, b);
  for (const cmd of unbound) byCmd.set(cmd, null);
  const m = new Map<string, string>();
  for (const [cmd, b] of byCmd) {
    if (b) m.set(bindingKey(b), cmd);
  }
  return m;
}

export function createKeymapStore(): KeymapStore {
  const commands: Record<string, CommandInfo> = Object.fromEntries(
    DEFAULT_COMMANDS.map((c) => [c.commandId, c]),
  );
  return createStore<KeymapViewState & { actions: KeymapActions }>()(
    (set, get) => ({
      overrides: {},
      unbound: new Set<string>(),
      commands,
      pendingConflict: null,
      actions: {
        registerCommands(list) {
          set((s) => ({
            commands: {
              ...s.commands,
              ...Object.fromEntries(list.map((c) => [c.commandId, c])),
            },
          }));
        },
        bindingFor(commandId) {
          const s = get();
          if (s.unbound.has(commandId)) return null;
          return s.overrides[commandId] ?? DEFAULT_MAP[commandId] ?? null;
        },
        commandFor(stroke) {
          let b: KeyBinding;
          try {
            b = parseBinding(stroke);
          } catch {
            return null;
          }
          const s = get();
          return effective(s.commands, s.overrides, s.unbound).get(
            bindingKey(b),
          ) ?? null;
        },
        bind(stroke, commandId) {
          const s = get();
          const b = parseBinding(stroke); // throws → caller surfaces
          if (!s.commands[commandId]) {
            throw new Error(`unknown command ${commandId}`);
          }
          const eff = effective(s.commands, s.overrides, s.unbound);
          const incumbent = eff.get(bindingKey(b));
          if (incumbent && incumbent !== commandId) {
            const conflict: BindingConflict = {
              binding: formatBinding(b),
              current: incumbent,
              incoming: commandId,
            };
            set({ pendingConflict: conflict });
            return conflict;
          }
          set((st) => ({
            overrides: { ...st.overrides, [commandId]: b },
            unbound: new Set([...st.unbound].filter((c) => c !== commandId)),
            pendingConflict: null,
          }));
          return null;
        },
        resolveConflict(keepIncoming) {
          const c = get().pendingConflict;
          if (!c) return;
          set((st) => {
            const overrides = { ...st.overrides };
            const unbound = new Set(st.unbound);
            if (keepIncoming) {
              overrides[c.incoming] = parseBinding(c.binding);
              unbound.delete(c.incoming);
              // The incumbent loses the key — unbound, not reassigned.
              delete overrides[c.current];
              unbound.add(c.current);
            }
            return { overrides, unbound, pendingConflict: null };
          });
        },
        unbind(commandId) {
          set((st) => {
            const overrides = { ...st.overrides };
            delete overrides[commandId];
            return {
              overrides,
              unbound: new Set([...st.unbound, commandId]),
            };
          });
        },
        resetToDefaults() {
          set({ overrides: {}, unbound: new Set(), pendingConflict: null });
        },
        exportKeymap() {
          const s = get();
          const doc: KeymapData = {
            version: KEYMAP_FORMAT_VERSION,
            overrides: s.overrides,
            unbound: [...s.unbound],
          };
          return JSON.stringify(doc, null, 2);
        },
        importKeymap(json) {
          let doc: KeymapData;
          try {
            doc = JSON.parse(json) as KeymapData;
          } catch {
            return { ok: false, error: 'not JSON' };
          }
          if (doc?.version !== KEYMAP_FORMAT_VERSION) {
            return { ok: false, error: `unsupported version ${doc?.version}` };
          }
          const overrides: Record<string, KeyBinding> = {};
          const keys = Object.keys(doc.overrides ?? {});
          if (keys.length > MAX_BINDINGS) {
            return { ok: false, error: 'too many bindings' };
          }
          for (const [cmd, b] of Object.entries(doc.overrides ?? {})) {
            if (!get().commands[cmd]) {
              return { ok: false, error: `unknown command ${cmd}` };
            }
            if (typeof b !== 'object' || b === null || typeof b.key !== 'string') {
              return { ok: false, error: 'bad binding shape' };
            }
            overrides[cmd] = {
              ctrl: !!b.ctrl,
              alt: !!b.alt,
              shift: !!b.shift,
              meta: !!b.meta,
              key: b.key,
            };
          }
          // Reject imports that leave two commands on one key.
          const unbound = new Set(
            Array.isArray(doc.unbound) ? doc.unbound : [],
          );
          const eff = effective(get().commands, overrides, unbound);
          if (eff.size < Object.values(overrides).length) {
            return { ok: false, error: 'import contains binding collisions' };
          }
          set({ overrides, unbound, pendingConflict: null });
          return { ok: true };
        },
      },
    }),
  );
}
