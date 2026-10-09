// Screenset store (W21) — layout presets with switch/lock semantics.
//   * presets are deep-copied on switch — mutating the live layout
//     never corrupts the stored preset;
//   * a LOCKED screenset blocks live geometry/visibility changes until
//     unlocked (the gate is enforced here);
//   * "fit to viewport" rescales stored normalized geometry for a
//     viewport aspect — a real derivation the shell applies;
//   * builtins can't be deleted or renamed; user sets can.

import { createStore, StoreApi } from 'zustand/vanilla';
import {
  PANEL_IDS,
  panelState,
  parseScreenset,
  type PanelGeometry,
  type ScreensetPanelId,
  type PanelState,
  type Screenset,
} from './types';

export const SCREENSET_FORMAT_VERSION = 1;
export const MAX_USER_SCREENSETS = 64;

function builtin(id: string, name: string, panels: PanelState[]): Screenset {
  return { id, name, panels, locked: false, builtin: true };
}

/** Shipped presets — every panel id appears exactly once per preset
 * (a screenset always describes the whole shell). */
export const BUILTIN_SCREENSETS: readonly Screenset[] = Object.freeze([
  builtin('arrange', 'Arrange', [
    panelState('arrangement', true, { x: 0, y: 0.2, w: 1, h: 0.62 }),
    panelState('transport', true, { x: 0, y: 0, w: 1, h: 0.1, z: 1 }),
    panelState('browser', true, { x: 0.82, y: 0.2, w: 0.18, h: 0.62, z: 1 }),
    panelState('mixer', false),
    panelState('pianoRoll', false),
    panelState('inspector', false),
    panelState('meter', true, { x: 0.82, y: 0.82, w: 0.18, h: 0.18, z: 1 }),
    panelState('jobQueue', false),
    panelState('proposals', false),
    panelState('metering', false),
  ]),
  builtin('edit', 'Edit', [
    panelState('arrangement', true, { x: 0, y: 0.1, w: 1, h: 0.45 }),
    panelState('pianoRoll', true, { x: 0, y: 0.55, w: 1, h: 0.45 }),
    panelState('transport', true, { x: 0, y: 0, w: 1, h: 0.1, z: 1 }),
    panelState('browser', false),
    panelState('inspector', true, { x: 0, y: 0.55, w: 0.2, h: 0.45, z: 1 }),
    panelState('mixer', false),
    panelState('meter', false),
    panelState('jobQueue', false),
    panelState('proposals', false),
    panelState('metering', false),
  ]),
  builtin('mix', 'Mix', [
    panelState('mixer', true, { x: 0, y: 0.1, w: 1, h: 0.9 }),
    panelState('transport', true, { x: 0, y: 0, w: 1, h: 0.1, z: 1 }),
    panelState('meter', true, { x: 0.8, y: 0.1, w: 0.2, h: 0.5, z: 1 }),
    panelState('metering', true, { x: 0.8, y: 0.6, w: 0.2, h: 0.4, z: 1 }),
    panelState('arrangement', false),
    panelState('pianoRoll', false),
    panelState('browser', false),
    panelState('inspector', false),
    panelState('jobQueue', false),
    panelState('proposals', false),
  ]),
]);

export interface ScreensetViewState {
  presets: Screenset[];
  activeId: string | null;
  /** Live layout — what the shell actually renders right now. */
  live: PanelState[];
  /** True while a switch is in flight (async apply on the shell). */
  busy: boolean;
}

export interface ScreensetActions {
  /** Switch to preset — live layout becomes a deep copy of it. */
  switchTo(id: string): boolean;
  /** Mutate a live panel. Rejected while the active set is locked. */
  setPanelVisible(panelId: ScreensetPanelId, visible: boolean): boolean;
  movePanel(panelId: ScreensetPanelId, geom: Partial<PanelGeometry>): boolean;
  /** Lock/unlock the ACTIVE screenset (not builtin-locked semantics:
   * any set may be locked to freeze the layout). */
  setLocked(locked: boolean): boolean;
  /** Save live layout as a new user preset. */
  saveAs(name: string): Screenset | null;
  /** Write live back into the active user preset. */
  updateActive(): boolean;
  delete(id: string): boolean;
  rename(id: string, name: string): boolean;
  /** Scale stored normalized geometry for a viewport aspect (w/h px). */
  fitToViewport(viewW: number, viewH: number): PanelState[];
  exportAll(): string;
  importAll(json: string): { ok: boolean; error?: string };
  activeLocked(): boolean;
}

export type ScreensetStore = StoreApi<
  ScreensetViewState & { actions: ScreensetActions }
>;

function clonePanels(ps: PanelState[]): PanelState[] {
  return ps.map((p) => ({ ...p, geometry: { ...p.geometry } }));
}

export function createScreensetStore(): ScreensetStore {
  return createStore<ScreensetViewState & { actions: ScreensetActions }>()(
    (set, get) => ({
      presets: BUILTIN_SCREENSETS.map((s) => ({
        ...s,
        panels: clonePanels(s.panels),
      })),
      activeId: BUILTIN_SCREENSETS[0].id,
      live: clonePanels(BUILTIN_SCREENSETS[0].panels),
      busy: false,
      actions: {
        switchTo(id) {
          const s = get().presets.find((p) => p.id === id);
          if (!s) return false;
          set({ activeId: s.id, live: clonePanels(s.panels) });
          return true;
        },
        setPanelVisible(panelId, visible) {
          const s = get();
          const active = s.presets.find((p) => p.id === s.activeId);
          if (active?.locked) return false;
          set({
            live: s.live.map((p) =>
              p.panelId === panelId ? { ...p, visible } : p,
            ),
          });
          return true;
        },
        movePanel(panelId, geom) {
          const s = get();
          const active = s.presets.find((p) => p.id === s.activeId);
          if (active?.locked) return false;
          set({
            live: s.live.map((p) =>
              p.panelId === panelId
                ? { ...p, geometry: { ...p.geometry, ...geom } }
                : p,
            ),
          });
          return true;
        },
        setLocked(locked) {
          const s = get();
          const idx = s.presets.findIndex((p) => p.id === s.activeId);
          if (idx < 0) return false;
          const presets = s.presets.map((p, i) =>
            i === idx ? { ...p, locked } : p,
          );
          set({ presets });
          return true;
        },
        saveAs(name) {
          const n = name.trim();
          if (!n || n.length > 48) return null;
          const s = get();
          if (
            s.presets.filter((p) => !p.builtin).length >= MAX_USER_SCREENSETS ||
            s.presets.some((p) => p.name === n)
          ) {
            return null;
          }
          const preset: Screenset = {
            id: `user-${Date.now().toString(36)}-${Math.random()
              .toString(36)
              .slice(2, 8)}`,
            name: n,
            panels: clonePanels(s.live),
            locked: false,
            builtin: false,
          };
          set({ presets: [...s.presets, preset] });
          return preset;
        },
        updateActive() {
          const s = get();
          const idx = s.presets.findIndex((p) => p.id === s.activeId);
          if (idx < 0 || s.presets[idx].builtin || s.presets[idx].locked) {
            return false;
          }
          const presets = s.presets.map((p, i) =>
            i === idx ? { ...p, panels: clonePanels(s.live) } : p,
          );
          set({ presets });
          return true;
        },
        delete(id) {
          const s = get();
          const t = s.presets.find((p) => p.id === id);
          if (!t || t.builtin) return false;
          const presets = s.presets.filter((p) => p.id !== id);
          const activeId = s.activeId === id ? presets[0]?.id ?? null : s.activeId;
          const live = s.activeId === id ? clonePanels(presets[0]?.panels ?? []) : s.live;
          set({ presets, activeId, live });
          return true;
        },
        rename(id, name) {
          const n = name.trim();
          const s = get();
          const t = s.presets.find((p) => p.id === id);
          if (!t || t.builtin || !n || s.presets.some((p) => p.name === n)) {
            return false;
          }
          set({
            presets: s.presets.map((p) => (p.id === id ? { ...p, name: n } : p)),
          });
          return true;
        },
        fitToViewport(viewW, viewH) {
          // Keep each panel's fractional box but re-letterbox vertically
          // when the real aspect is narrower/taller than 16:9 design.
          const s = get();
          if (viewW <= 0 || viewH <= 0) return s.live;
          const designAspect = 16 / 9;
          const aspect = viewW / viewH;
          const yScale = aspect >= designAspect ? 1 : aspect / designAspect;
          const yOff = (1 - yScale) / 2;
          return s.live.map((p) => ({
            ...p,
            geometry: {
              ...p.geometry,
              y: p.geometry.y * yScale + yOff,
              h: p.geometry.h * yScale,
            },
          }));
        },
        exportAll() {
          const s = get();
          return JSON.stringify(
            {
              version: SCREENSET_FORMAT_VERSION,
              activeId: s.activeId,
              sets: s.presets.filter((p) => !p.builtin),
            },
            null,
            2,
          );
        },
        importAll(json) {
          let doc: { version?: number; activeId?: string; sets?: unknown[] };
          try {
            doc = JSON.parse(json);
          } catch {
            return { ok: false, error: 'not JSON' };
          }
          if (doc?.version !== SCREENSET_FORMAT_VERSION || !Array.isArray(doc.sets)) {
            return { ok: false, error: 'bad format' };
          }
          const imported: Screenset[] = [];
          for (const raw of doc.sets) {
            const p = parseScreenset(raw);
            if (!p) return { ok: false, error: 'bad screenset' };
            if (p.builtin) return { ok: false, error: 'builtin flag not importable' };
            if (get().presets.some((q) => q.name === p.name)) {
              return { ok: false, error: `name ${p.name} exists` };
            }
            imported.push({ ...p, builtin: false });
          }
          if (imported.length > MAX_USER_SCREENSETS) {
            return { ok: false, error: 'too many' };
          }
          set({ presets: [...get().presets, ...imported] });
          return { ok: true };
        },
        activeLocked() {
          const s = get();
          return !!s.presets.find((p) => p.id === s.activeId)?.locked;
        },
      },
    }),
  );
}

/** Every panel id is present in a well-formed screenset — helper for
 * import validation + tests. */
export function isCompleteSet(s: Screenset): boolean {
  const ids = new Set(s.panels.map((p) => p.panelId));
  return PANEL_IDS.every((id) => ids.has(id));
}
