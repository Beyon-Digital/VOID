// Shared data hooks for the Signal Studio shell — real store/client only.

import * as React from 'react';
import {
  editorStore,
  loadViewPage,
  makeViewKey,
  studioStore,
  useStudio,
} from 'void-studio';
import type { EditorActions, EditorViewState } from 'void-studio';
import type { ReadItem } from 'void-client';
import { getClient } from '../client';

type EditorState = EditorViewState & { actions: EditorActions };

/** Selector over the editor (view-state-only) store. */
export function useEditor<T>(selector: (s: EditorState) => T): T {
  return React.useSyncExternalStore(
    (onChange) => editorStore.subscribe(onChange),
    () => selector(editorStore.getState()),
    () => selector(editorStore.getState()),
  );
}

export interface ProjectSummary {
  name: string | null;
  bpm: number | null;
  numTracks: number | null;
  recording: { phase?: string; isRecording?: boolean } | null;
  loaded: boolean;
}

/**
 * PROJECT_SUMMARY read view → project name + tempo + recording state.
 * Re-reads whenever the project/revision changes; stays '—' (honest) until
 * a page actually lands.
 */
export function useProjectSummary(): ProjectSummary {
  const projectId = useStudio((s) => s.projectId);
  const revision = useStudio((s) => s.revision);
  const attached = useStudio((s) => s.engine.attached);
  const entry = useStudio((s) => s.views[makeViewKey('PROJECT_SUMMARY')]);

  React.useEffect(() => {
    if (!projectId || !attached) return;
    void loadViewPage(studioStore, getClient(), 'PROJECT_SUMMARY').catch(() => undefined);
  }, [projectId, attached, revision]);

  return React.useMemo(() => {
    const item: ReadItem | undefined = entry?.items?.[0];
    if (!item) return { name: null, bpm: null, numTracks: null, recording: null, loaded: false };
    try {
      const v = JSON.parse(item.summary_json) as Record<string, unknown>;
      const rec = (v.recording ?? null) as ProjectSummary['recording'];
      return {
        name: typeof v.name === 'string' ? v.name : null,
        bpm: typeof v.bpm === 'number' ? v.bpm : null,
        numTracks: typeof v.numTracks === 'number' ? v.numTracks : null,
        recording: rec,
        loaded: true,
      };
    } catch {
      return { name: null, bpm: null, numTracks: null, recording: null, loaded: false };
    }
  }, [entry]);
}

/** Compact breakpoint per the Signal responsive policy (≤1280 collapses). */
export function useStudioCompact(): boolean {
  const query = '(max-width: 1280px)';
  const [compact, setCompact] = React.useState(
    () => typeof window !== 'undefined' && window.matchMedia(query).matches,
  );
  React.useEffect(() => {
    const mq = window.matchMedia(query);
    const on = (e: MediaQueryListEvent) => setCompact(e.matches);
    mq.addEventListener('change', on);
    return () => mq.removeEventListener('change', on);
  }, []);
  return compact;
}

const CompactContext = React.createContext(false);
export const StudioCompactProvider = CompactContext.Provider;
export function useStudioCompactContext(): boolean {
  return React.useContext(CompactContext);
}

const VALID_HASH = /^#\/([a-z0-9-/]+)$/;

/** Read the current hash route (`#/arrange`, `#/dev/gallery`…). */
export function readHashRoute(): string | null {
  if (typeof window === 'undefined') return null;
  const m = VALID_HASH.exec(window.location.hash);
  return m ? m[1] : null;
}

/**
 * Hash-synced screen route. `#/<screen-id>` navigates; `#/dev/gallery` is the
 * component gallery. Unknown ids still navigate — the registry renders the
 * honest "not built yet" placeholder for them.
 */
export function useHashRoute(
  fallback: string,
): [string, (id: string) => void] {
  const [route, setRoute] = React.useState(() => readHashRoute() ?? fallback);
  const navigate = React.useCallback((id: string) => {
    setRoute(id);
    try {
      window.location.hash = `/${id}`;
    } catch {
      /* no DOM */
    }
  }, []);
  React.useEffect(() => {
    const onHash = () => {
      const r = readHashRoute();
      if (r) setRoute(r);
    };
    window.addEventListener('hashchange', onHash);
    return () => window.removeEventListener('hashchange', onHash);
  }, []);
  return [route, navigate];
}
