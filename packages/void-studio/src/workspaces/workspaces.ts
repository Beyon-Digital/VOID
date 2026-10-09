// Workspace shells — Signal Studio's five canonical workspaces.
//
// W09: workspaces share ONE transport bar and the base store's track
// selection. Signal Studio (UI01) extends the original three to the
// designed set: Arrange / Compose / Mix / Perform / Visuals.

import type { PanelId } from '../store';

export type WorkspaceId = 'arrange' | 'compose' | 'mix' | 'perform' | 'visuals';

export const WORKSPACE_ORDER: WorkspaceId[] = [
  'arrange',
  'compose',
  'mix',
  'perform',
  'visuals',
];

export interface WorkspaceDef {
  id: WorkspaceId;
  title: string;
  /** Accessible label for the workspace's main region. */
  ariaLabel: string;
  /** Primary region id — the default keyboard-focus target. */
  primaryRegion: 'piano-roll' | 'timeline' | 'mixer' | 'scene-grid' | 'visual-output';
  /** Panels this workspace keeps open when it becomes active. */
  panels: Partial<Record<PanelId, boolean>>;
  /** Keyboard shortcut (Ctrl/Cmd+1..5) — handled by the shell. */
  shortcut: string;
}

export const WORKSPACES: Record<WorkspaceId, WorkspaceDef> = {
  arrange: {
    id: 'arrange',
    title: 'Arrange',
    ariaLabel: 'Arrange workspace — timeline of tracks and clips',
    primaryRegion: 'timeline',
    panels: { browser: true, inspector: true, mixer: false, editor: true, meters: false },
    shortcut: '1',
  },
  compose: {
    id: 'compose',
    title: 'Compose',
    ariaLabel: 'Compose workspace — piano roll editor for the selected clip',
    primaryRegion: 'piano-roll',
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
  perform: {
    id: 'perform',
    title: 'Perform',
    ariaLabel: 'Perform workspace — scene launch grid',
    primaryRegion: 'scene-grid',
    panels: { browser: true, inspector: false, mixer: false, editor: true, meters: false },
    shortcut: '4',
  },
  visuals: {
    id: 'visuals',
    title: 'Visuals',
    ariaLabel: 'Visuals workspace — preview and program output',
    primaryRegion: 'visual-output',
    panels: { browser: false, inspector: true, mixer: false, editor: true, meters: false },
    shortcut: '5',
  },
};

export function isWorkspaceId(v: string): v is WorkspaceId {
  return WORKSPACE_ORDER.indexOf(v as WorkspaceId) >= 0;
}

/** Cycle workspaces with a keyboard shortcut digit (1..3 → order index). */
export function workspaceForShortcut(key: string): WorkspaceId | null {
  const i = WORKSPACE_ORDER.findIndex((w) => WORKSPACES[w].shortcut === key);
  return i >= 0 ? WORKSPACE_ORDER[i] : null;
}
