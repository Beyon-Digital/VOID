// Customizable keymap model (W21 T80-side; PRO-05).
//
// View-state only — the store owns binding→command resolution and
// conflict detection; the app registers its commands by id. String
// key descriptors use a canonical form ("Ctrl+Shift+Z", "F4",
// "Alt+A") produced by normalizeBinding — never stored raw.

/** Canonical binding: `Modifier+Modifier+Key` in fixed order
 * Ctrl<Alt<Shift<Meta, then the key token upper-cased once. */
export interface KeyBinding {
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  meta: boolean;
  /** Single key token, upper-cased ("A", "F4", "Space", "ArrowLeft"). */
  key: string;
}

const MOD_ORDER = ['ctrl', 'alt', 'shift', 'meta'] as const;
type Mod = (typeof MOD_ORDER)[number];

const KEY_ALIASES: Record<string, string> = {
  ' ': 'Space',
  spacebar: 'Space',
  esc: 'Escape',
  del: 'Delete',
  return: 'Enter',
  left: 'ArrowLeft',
  right: 'ArrowRight',
  up: 'ArrowUp',
  down: 'ArrowDown',
  'control': 'Ctrl',
  'cmd': 'Meta',
  'command': 'Meta',
  'option': 'Alt',
};

/** Parse a user/display string ("ctrl+shift+z") into a canonical
 * binding. Throws on empty/garbage input — callers feed UI text in,
 * so parse is strict (inert data, never instructions). */
export function parseBinding(input: string): KeyBinding {
  const mods = new Set<Mod>();
  let key = '';
  for (const raw of input.split('+')) {
    const tok = raw.trim();
    if (!tok) continue;
    const lower = tok.toLowerCase();
    const alias = KEY_ALIASES[lower] ?? lower;
    const modKey = alias.toLowerCase();
    if (modKey === 'ctrl' || modKey === 'alt' || modKey === 'shift' || modKey === 'meta') {
      mods.add(modKey as Mod);
    } else {
      key = alias.length === 1 ? alias.toUpperCase() : alias;
    }
  }
  if (!key) {
    throw new Error(`keybinding "${input}" has no key`);
  }
  return {
    ctrl: mods.has('ctrl'),
    alt: mods.has('alt'),
    shift: mods.has('shift'),
    meta: mods.has('meta'),
    key,
  };
}

/** Canonical display form — same input always ⇒ same string. */
export function formatBinding(b: KeyBinding): string {
  const parts: string[] = [];
  for (const m of MOD_ORDER) {
    if (b[m]) parts.push(m[0].toUpperCase() + m.slice(1));
  }
  parts.push(b.key);
  return parts.join('+');
}

/** Canonical map key — identical bindings hash equal. */
export function bindingKey(b: KeyBinding): string {
  return formatBinding(b);
}
