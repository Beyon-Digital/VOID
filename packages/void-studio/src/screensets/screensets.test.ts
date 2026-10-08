import { describe, expect, it } from 'vitest';
import {
  BUILTIN_SCREENSETS,
  createScreensetStore,
  isCompleteSet,
  parseScreenset,
} from './index';

describe('screenset store', () => {
  it('builtins are complete panel sets and switching deep-copies', () => {
    for (const b of BUILTIN_SCREENSETS) {
      expect(isCompleteSet(b)).toBe(true);
    }
    const s = createScreensetStore();
    expect(s.getState().actions.switchTo('mix')).toBe(true);
    expect(s.getState().activeId).toBe('mix');
    // Live mutation doesn't corrupt the stored preset.
    s.getState().actions.setPanelVisible('arrangement', true);
    const preset = s.getState().presets.find((p) => p.id === 'mix')!;
    expect(
      preset.panels.find((p) => p.panelId === 'arrangement')!.visible,
    ).toBe(false);
  });

  it('lock gates live mutation', () => {
    const s = createScreensetStore();
    s.getState().actions.switchTo('arrange');
    expect(s.getState().actions.setLocked(true)).toBe(true);
    expect(s.getState().actions.setPanelVisible('mixer', true)).toBe(false);
    expect(s.getState().actions.movePanel('arrangement', { x: 0.5 })).toBe(false);
    expect(s.getState().actions.setLocked(false)).toBe(true);
    expect(s.getState().actions.setPanelVisible('mixer', true)).toBe(true);
  });

  it('saveAs captures live and updateActive persists', () => {
    const s = createScreensetStore();
    s.getState().actions.switchTo('edit');
    s.getState().actions.setPanelVisible('jobQueue', true);
    const preset = s.getState().actions.saveAs('My Edit')!;
    expect(preset.builtin).toBe(false);
    s.getState().actions.switchTo(preset.id);
    expect(
      s.getState().live.find((p) => p.panelId === 'jobQueue')!.visible,
    ).toBe(true);
    s.getState().actions.setPanelVisible('browser', true);
    expect(s.getState().actions.updateActive()).toBe(true);
    const stored = s.getState().presets.find((p) => p.id === preset.id)!;
    expect(
      stored.panels.find((p) => p.panelId === 'browser')!.visible,
    ).toBe(true);
    // Builtin can't be renamed/deleted; user can.
    expect(s.getState().actions.rename('arrange', 'x')).toBe(false);
    expect(s.getState().actions.delete('arrange')).toBe(false);
    expect(s.getState().actions.rename(preset.id, 'Better')).toBe(true);
    expect(s.getState().actions.delete(preset.id)).toBe(true);
  });

  it('fitToViewport letterboxes on narrow aspect', () => {
    const s = createScreensetStore();
    const fit = s.getState().actions.fitToViewport(800, 900);
    // Taller-than-design viewport → panels shrink vertically, centered.
    const arr = fit.find((p) => p.panelId === 'arrangement')!;
    expect(arr.geometry.h).toBeLessThan(0.62);
    expect(arr.geometry.y).toBeGreaterThan(0.2);
    // Wide viewport → unchanged fractions.
    const wide = s.getState().actions.fitToViewport(3840, 1080);
    const arr2 = wide.find((p) => p.panelId === 'arrangement')!;
    expect(arr2.geometry.h).toBeCloseTo(0.62);
  });

  it('export/import user sets defensively', () => {
    const s = createScreensetStore();
    s.getState().actions.saveAs('S1');
    const json = s.getState().actions.exportAll();
    const s2 = createScreensetStore();
    expect(s2.getState().actions.importAll(json).ok).toBe(true);
    expect(s2.getState().presets.some((p) => p.name === 'S1')).toBe(true);
    expect(s2.getState().actions.importAll(json).ok).toBe(false); // dup name
    expect(
      s2.getState().actions.importAll('{"version":9,"sets":[]}').ok,
    ).toBe(false);
    expect(
      s2.getState().actions.importAll(
        JSON.stringify({ version: 1, sets: [{ id: 'x', builtin: true }] }),
      ).ok,
    ).toBe(false);
  });
});
