import { describe, expect, it } from 'vitest';
import {
  bindingKey,
  createKeymapStore,
  formatBinding,
  parseBinding,
} from './index';

describe('binding canonicalization', () => {
  it('normalizes modifiers and key case', () => {
    expect(formatBinding(parseBinding('ctrl+shift+z'))).toBe('Ctrl+Shift+Z');
    expect(formatBinding(parseBinding('shift+ctrl+z'))).toBe('Ctrl+Shift+Z');
    expect(formatBinding(parseBinding('  cmd + p '))).toBe('Meta+P');
    expect(formatBinding(parseBinding('esc'))).toBe('Escape');
    expect(bindingKey(parseBinding('ctrl+a'))).toBe(
      bindingKey(parseBinding('Ctrl+A')),
    );
  });
  it('rejects keyless chords', () => {
    expect(() => parseBinding('ctrl+shift')).toThrow();
    expect(() => parseBinding('')).toThrow();
  });
});

describe('keymap store', () => {
  it('defaults resolve and overrides win', () => {
    const s = createKeymapStore();
    expect(s.getState().actions.commandFor('Space')).toBe('transport.play');
    expect(s.getState().actions.commandFor('ctrl+shift+p')).toBe(
      'view.commandPalette',
    );
    s.getState().actions.bind('Ctrl+P', 'view.commandPalette');
    expect(
      s.getState().actions.bindingFor('view.commandPalette')?.key,
    ).toBe('P');
    expect(s.getState().actions.commandFor('Ctrl+P')).toBe(
      'view.commandPalette',
    );
  });

  it('detects conflicts and resolves either way', () => {
    const s = createKeymapStore();
    const conflict = s.getState().actions.bind('Space', 'edit.copy');
    expect(conflict).not.toBeNull();
    expect(conflict!.current).toBe('transport.play');
    expect(conflict!.incoming).toBe('edit.copy');
    // Incoming wins: play becomes unbound (not shared).
    s.getState().actions.resolveConflict(true);
    expect(s.getState().actions.commandFor('Space')).toBe('edit.copy');
    expect(s.getState().actions.bindingFor('transport.play')).toBeNull();
    // Re-conflict and keep incumbent instead.
    const c2 = s.getState().actions.bind('Space', 'edit.paste');
    expect(c2).not.toBeNull();
    s.getState().actions.resolveConflict(false);
    expect(s.getState().actions.commandFor('Space')).toBe('edit.copy');
  });

  it('defaults are immutable and resettable', () => {
    const s = createKeymapStore();
    const def = s.getState().actions.bindingFor('transport.play');
    expect(def).toEqual(parseBinding('Space'));
    s.getState().actions.bind('Ctrl+P', 'transport.play');
    s.getState().actions.resetToDefaults();
    expect(s.getState().actions.commandFor('Space')).toBe('transport.play');
    expect(s.getState().actions.bindingFor('transport.play')).toEqual(def);
  });

  it('export/import round-trips and rejects bad docs', () => {
    const s = createKeymapStore();
    s.getState().actions.bind('Ctrl+P', 'transport.play');
    s.getState().actions.unbind('edit.cut');
    const json = s.getState().actions.exportKeymap();
    const s2 = createKeymapStore();
    expect(s2.getState().actions.importKeymap(json).ok).toBe(true);
    expect(s2.getState().actions.commandFor('Ctrl+P')).toBe('transport.play');
    expect(s2.getState().actions.bindingFor('edit.cut')).toBeNull();
    expect(
      s2.getState().actions.importKeymap('{not json').ok,
    ).toBe(false);
    expect(
      s2.getState().actions.importKeymap('{"version":99}').ok,
    ).toBe(false);
    expect(
      s2.getState().actions.importKeymap(
        JSON.stringify({
          version: 1,
          overrides: { 'nope.cmd': { ctrl: true, key: 'X' } },
        }),
      ).ok,
    ).toBe(false);
  });
});
