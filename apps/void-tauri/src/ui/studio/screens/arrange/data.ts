// Shared view-state + command-surface helpers for the arrange workspace
// (S01/S22/S23). Everything here reads through void-studio stores or the
// bounded read-view cache in studioStore.views — nothing is fabricated.

import * as React from 'react';
import type { ReadItem } from 'void-client';
import {
  ClipEditor,
  NoteEditor,
  loadViewPage,
  createArrangementStore,
  createAutomationStore,
  createInstrumentBrowseStore,
  editorStore,
  makeViewKey,
  parseClipItem,
  studioStore,
  useStudio,
  type ArrangementStore,
  type AutomationStore,
  type ClipView,
  type InstrumentBrowseStore,
} from 'void-studio';
import { getClient } from '../../../client';
import { useEditor } from '../../useStudioData';

const newGesture = () => crypto.randomUUID();

// Lazy singletons — same command-surface pattern as ui/shell.tsx.
let _clipEd: ClipEditor | null = null;
export function clipEd(): ClipEditor {
  if (!_clipEd) _clipEd = new ClipEditor(getClient(), () => crypto.randomUUID());
  return _clipEd;
}
let _noteEd: NoteEditor | null = null;
export function noteEd(): NoteEditor {
  if (!_noteEd) _noteEd = new NoteEditor(getClient(), () => crypto.randomUUID());
  return _noteEd;
}

// Feature stores owned by this workspace (view state only — the engine
// stays the project truth).
let _arrangement: ArrangementStore | null = null;
export function arrangementStore(): ArrangementStore {
  if (!_arrangement) _arrangement = createArrangementStore();
  return _arrangement;
}
let _automation: AutomationStore | null = null;
export function automationStore(): AutomationStore {
  if (!_automation) _automation = createAutomationStore();
  return _automation;
}
let _browse: InstrumentBrowseStore | null = null;
export function instrumentBrowseStore(): InstrumentBrowseStore {
  if (!_browse) _browse = createInstrumentBrowseStore();
  return _browse;
}

export { newGesture };

// ---------------------------------------------------------------------------
// TRACK_LIST projection
// ---------------------------------------------------------------------------

export interface TrackRow {
  objectId: string;
  id: string;
  name: string;
  kind?: string;
  muted: boolean;
  soloed: boolean;
}

export function useTrackRows(): { tracks: TrackRow[]; loaded: boolean } {
  const views = useStudio((s) => s.views);
  const entry = views[makeViewKey('TRACK_LIST')];
  return React.useMemo(() => {
    if (!entry) return { tracks: [], loaded: false };
    const tracks = entry.items.map((it: ReadItem) => {
      try {
        const v = JSON.parse(it.summary_json) as Record<string, unknown>;
        return {
          objectId: it.object_id,
          id: String(v.track_id ?? v.id ?? it.object_id),
          name: String(v.name ?? v.track_id ?? it.object_id),
          kind: typeof v.kind === 'string' ? v.kind : undefined,
          muted: v.muted === true,
          soloed: v.soloed === true,
        };
      } catch {
        return {
          objectId: it.object_id,
          id: it.object_id,
          name: it.object_id,
          muted: false,
          soloed: false,
        };
      }
    });
    return { tracks, loaded: true };
  }, [entry]);
}

// ---------------------------------------------------------------------------
// CLIP_LIST projection — lane-scoped slices (T30): lanes read the bounded
// view slice for the visible viewport range.
// ---------------------------------------------------------------------------

export function useTrackClips(
  trackId: string,
  startTicks: string,
  endTicks: string,
): { clips: ClipView[] | null } {
  const views = useStudio((s) => s.views);
  const attached = useStudio((s) => s.engine.attached);
  const key = makeViewKey('CLIP_LIST', trackId, startTicks, endTicks);
  const entry = views[key];

  // Load the bounded slice once per (track, range) — a load mutates views,
  // so key the effect on the key itself, not on `entry`.
  const loadedRef = React.useRef<string | null>(null);
  React.useEffect(() => {
    if (!attached || loadedRef.current === key) return;
    loadedRef.current = key;
    void clipEd()
      .loadTrackClips(studioStore, trackId, { startTicks, endTicks })
      .catch((e) =>
        editorStore.getState().actions.setEditError(String(e)),
      );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, attached, trackId]);

  const clips = React.useMemo(() => {
    if (!entry) return null;
    const out: ClipView[] = [];
    for (const it of entry.items) {
      const c = parseClipItem(it);
      if (c) out.push(c);
    }
    return out;
  }, [entry]);
  return { clips };
}

// ---------------------------------------------------------------------------
// PLUGIN_LIST projection — one row per loaded device slot on a track.
// ---------------------------------------------------------------------------

export interface PluginSlot {
  slotIndex: number;
  instanceId: string;
  pluginUid: string;
  name?: string;
  bypassed?: boolean;
  paramValues: Record<string, number>;
}

const str = (v: unknown): string | undefined =>
  typeof v === 'string' && v !== '' ? v : undefined;

export function parsePluginItem(item: ReadItem): PluginSlot | null {
  let raw: unknown;
  try {
    raw = JSON.parse(item.summary_json);
  } catch {
    return null;
  }
  if (typeof raw !== 'object' || raw === null) return null;
  const o = raw as Record<string, unknown>;
  const pluginUid = str(o.plugin_uid) ?? str(o.pluginUid) ?? str(o.format);
  if (!pluginUid) return null;
  const params = (o.param_values ?? o.params) as Record<string, unknown> | undefined;
  const paramValues: Record<string, number> = {};
  if (params && typeof params === 'object') {
    for (const [k, v] of Object.entries(params)) {
      const n = typeof v === 'number' ? v : Number(v);
      if (Number.isFinite(n)) paramValues[k] = n;
    }
  }
  return {
    slotIndex: Number(o.slot_index ?? o.slotIndex ?? 0) || 0,
    instanceId: str(o.instance_id) ?? str(o.instanceId) ?? str(o.plugin_instance_id) ?? item.object_id,
    pluginUid,
    name: str(o.name),
    bypassed: o.bypassed === true ? true : undefined,
    paramValues,
  };
}

export function useTrackPlugins(trackId: string | null): {
  slots: PluginSlot[];
  loaded: boolean;
} {
  const views = useStudio((s) => s.views);
  const attached = useStudio((s) => s.engine.attached);
  const entry = trackId ? views[makeViewKey('PLUGIN_LIST', trackId)] : undefined;

  const loadedFor = React.useRef<string | null>(null);
  React.useEffect(() => {
    if (!attached || !trackId || loadedFor.current === trackId) return;
    loadedFor.current = trackId;
    void loadViewPage(studioStore, getClient(), 'PLUGIN_LIST', { trackId }).catch(
      () => undefined,
    );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [attached, trackId]);

  return React.useMemo(() => {
    if (!entry) return { slots: [], loaded: false };
    const slots: PluginSlot[] = [];
    for (const it of entry.items) {
      const s = parsePluginItem(it);
      if (s) slots.push(s);
    }
    slots.sort((a, b) => a.slotIndex - b.slotIndex);
    return { slots, loaded: true };
  }, [entry]);
}

// ---------------------------------------------------------------------------
// The clip currently selected in the editor (for the inspector + dock).
// ---------------------------------------------------------------------------

export function useSelectedClip(): ClipView | null {
  const clipSel = useEditor((s) => s.clipSelection);
  const views = useStudio((s) => s.views);
  return React.useMemo(() => {
    if (!clipSel.trackId || clipSel.clipIds.length === 0) return null;
    const clipId = clipSel.clipIds[0];
    for (const [key, entry] of Object.entries(views)) {
      if (!key.startsWith(`CLIP_LIST|${clipSel.trackId}`)) continue;
      for (const it of entry.items) {
        const c = parseClipItem(it);
        if (c && c.clipId === clipId) return c;
      }
    }
    return null;
  }, [clipSel, views]);
}
