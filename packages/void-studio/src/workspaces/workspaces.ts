// Workspace shells — Compose / Arrange / Mix.
//
// W09: three workspaces share ONE transport bar and the base store's
// track selection. No approved Figma file exists, so these are functional
// region layouts, not a design match: each workspace says which regions
// are primary and which editor owns keyboard focus by default.

import type { PanelId } from '../store';

export type WorkspaceId = 'compose' | 'arrange' | 'mix';

export const WORKSPACE_ORDER: WorkspaceId[] = ['compose', 'arrange', 'mix'];

export interface WorkspaceDef {
  id: WorkspaceId;
  title: string;
  /** Accessible label for the workspace's main region. */
  ariaLabel: string;
  /** Primary region id — the default keyboard-focus target. */
  primaryRegion: 'piano-roll' | 'timeline' | 'mixer';
  /** Panels this workspace keeps open when it becomes active. */
  panels: Partial<Record<PanelId, boolean>>;
  /** Keyboard shortcut (Ctrl/Cmd+1..3) — handled by the shell. */
  shortcut: string;
}

export const WORKSPACES: Record<WorkspaceId, WorkspaceDef> = {
  compose: {
    id: 'compose',
    title: 'Compose',
    ariaLabel: 'Compose workspace — piano roll editor for the selected clip',
    primaryRegion: 'piano-roll',
    panels: { browser: true, inspector: true, mixer: false, editor: true, meters: false },
    shortcut: '1',
  },
  arrange: {
    id: 'arrange',
    title: 'Arrange',
    ariaLabel: 'Arrange workspace — timeline of tracks and clips',
    primaryRegion: 'timeline',
    panels: { browser: true, inspector: true, mixer: false, editor: true, meters: false },
    shortcut: '2',
  },
  mix: {
    id: 'mix',
    title: 'Mix',
    ariaLabel: 'Mix workspace — channel controls and meters',
    primaryRegion: 'mixer',
    panels: { browser: false, inspector: true, mixer: true, editor: true, meters: true },
    shortcut: '3',
  },
};

export function isWorkspaceId(v: string): v is WorkspaceId {
  return v === 'compose' || v === 'arrange' || v === 'mix';
}

/** Cycle workspaces with a keyboard shortcut digit (1..3 → order index). */
export function workspaceForShortcut(key: string): WorkspaceId | null {
  const i = WORKSPACE_ORDER.findIndex((w) => WORKSPACES[w].shortcut === key);
  return i >= 0 ? WORKSPACE_ORDER[i] : null;
}
