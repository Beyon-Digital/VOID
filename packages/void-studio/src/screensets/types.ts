// Screenset (layout preset) model (W21 T80-side; DOC-04).
//
// View-state only: a screenset captures WHICH panels are open and WHERE
// (normalized geometry), never project data. Switch/lock semantics:
//   * switching replaces the active layout wholesale;
//   * a locked screenset rejects live layout mutation until unlocked —
//     the lock is a real gate in the store, not a UI hint.

/** Panels the studio shell can host. New panels append — the preset
 * format is additive so older files still load. */
export type ScreensetPanelId =
  | 'arrangement'
  | 'pianoRoll'
  | 'mixer'
  | 'browser'
  | 'inspector'
  | 'meter'
  | 'transport'
  | 'jobQueue'
  | 'proposals'
  | 'metering';

export const PANEL_IDS: readonly ScreensetPanelId[] = Object.freeze([
  'arrangement',
  'pianoRoll',
  'mixer',
  'browser',
  'inspector',
  'meter',
  'transport',
  'jobQueue',
  'proposals',
  'metering',
] as const);

/** Normalized panel geometry — fractions of the shell viewport,
 * 0..1 each. `z` stacks siblings (0 = bottom). */
export interface PanelGeometry {
  x: number;
  y: number;
  w: number;
  h: number;
  z: number;
}

export interface PanelState {
  panelId: ScreensetPanelId;
  visible: boolean;
  geometry: PanelGeometry;
}

export interface Screenset {
  id: string;
  name: string;
  panels: PanelState[];
  locked: boolean;
  /** Builtin presets may not be renamed/deleted. */
  builtin: boolean;
}

const DEFAULT_GEOM: PanelGeometry = { x: 0, y: 0, w: 1, h: 1, z: 0 };

export function panelState(
  panelId: ScreensetPanelId,
  visible: boolean,
  geometry: Partial<PanelGeometry> = {},
): PanelState {
  return { panelId, visible, geometry: { ...DEFAULT_GEOM, ...geometry } };
}

function isGeom(g: unknown): g is PanelGeometry {
  const o = g as PanelGeometry;
  return (
    typeof o === 'object' &&
    o !== null &&
    [o.x, o.y, o.w, o.h].every(
      (n) => typeof n === 'number' && Number.isFinite(n) && n >= 0 && n <= 1,
    ) &&
    typeof o.z === 'number' &&
    Number.isFinite(o.z) &&
    o.z >= 0
  );
}

/** Defensive parse for imported/screenshot layout JSON. */
export function parseScreenset(raw: unknown): Screenset | null {
  const o = raw as Screenset;
  if (
    typeof o !== 'object' ||
    o === null ||
    typeof o.id !== 'string' ||
    typeof o.name !== 'string' ||
    !Array.isArray(o.panels)
  ) {
    return null;
  }
  const panels: PanelState[] = [];
  for (const p of o.panels) {
    if (
      typeof p !== 'object' ||
      p === null ||
      !PANEL_IDS.includes((p as PanelState).panelId) ||
      !isGeom((p as PanelState).geometry)
    ) {
      return null;
    }
    panels.push({
      panelId: (p as PanelState).panelId,
      visible: !!(p as PanelState).visible,
      geometry: { ...(p as PanelState).geometry },
    });
  }
  return {
    id: o.id,
    name: o.name,
    panels,
    locked: !!o.locked,
    builtin: !!o.builtin,
  };
}
