// Feature-store runtime for the proposal / gesture / jobs screens.
//
// Singletons hold VIEW STATE ONLY (records the coordinator pushed,
// ghost previews, job cards). Document truth stays in the engine;
// every mutation below goes through client.sendCommand with
// commandId/transactionId set.

import * as React from 'react';
import {
  createGenerationStore,
  createGestureSession,
  createJobsStore,
  createProposalsStore,
  editorStore,
  makeViewKey,
  parseClipItem,
  studioStore,
  useStore,
  useStudio,
} from 'void-studio';
import type { ClipView } from 'void-studio';
import type { ReadItem } from 'void-client';
import {
  acceptProposalCandidate,
  dismissProposal,
  ingestControlEvent,
  ingestTelemetryEvent,
  rescanStaleness,
  targetClipExists,
  undoAcceptedTransaction,
} from 'void-studio/src/ingest';
import type { ModelRow } from 'void-studio/src/jobs/models';
import { parseModelList } from 'void-studio/src/jobs/models';
import { getClient } from '../../client';
import { useEditor } from '../useStudioData';

export const proposalsStore = createProposalsStore();
export const jobsStore = createJobsStore();
export const generationStore = createGenerationStore();
export const gestureSession = createGestureSession();

// -- model registry rows (pushed via model_list events; no read view) --------

interface ModelRowsState {
  rows: ModelRow[];
  loaded: boolean;
}
const modelListeners = new Set<() => void>();
let modelState: ModelRowsState = { rows: [], loaded: false };
function setModelRows(rows: ModelRow[]): void {
  modelState = { rows, loaded: true };
  modelListeners.forEach((l) => l());
}
export function useModelRows(): ModelRowsState {
  return React.useSyncExternalStore(
    (on) => {
      modelListeners.add(on);
      return () => modelListeners.delete(on);
    },
    () => modelState,
    () => modelState,
  );
}

// -- store hooks --------------------------------------------------------------

export const useProposals = <T,>(
  sel: (s: ReturnType<typeof proposalsStore.getState>) => T,
): T => useStore(proposalsStore, sel);
export const useJobs = <T,>(
  sel: (s: ReturnType<typeof jobsStore.getState>) => T,
): T => useStore(jobsStore, sel);
export const useGeneration = <T,>(
  sel: (s: ReturnType<typeof generationStore.getState>) => T,
): T => useStore(generationStore, sel);
export const useGestureSession = <T,>(
  sel: (s: ReturnType<typeof gestureSession.getState>) => T,
): T => useStore(gestureSession, sel);

// -- client event binding -----------------------------------------------------

let bound = false;

/**
 * Bind the feature stores to the client event stream. Idempotent —
 * every screen that needs the stores calls this in an effect.
 * Control events carrying proposal/model payloads and JobEvent
 * telemetry fold into the view stores; unknown kinds drop silently.
 */
export function bindFeatureStores(): () => void {
  const client = getClient();
  const unControl = client.onControl((ev) => {
    const e = ev as unknown as Record<string, unknown>;
    ingestControlEvent(e, proposalsStore);
    // Model registry snapshots arrive on the control channel under
    // kind 'model_list' once the coordinator emits them.
    if (e.kind === 'model_list') {
      setModelRows(parseModelList(e.models ?? e.rows ?? []));
    }
  });
  const unTelemetry = client.onTelemetry((ev) => {
    ingestTelemetryEvent(
      ev as unknown as Record<string, unknown>,
      jobsStore,
      generationStore,
    );
  });
  const unLost = client.onEngineLost(() => {
    // Session ended — every live proposal is stale (UI-T15).
    proposalsStore.getState().actions.markStale('session_ended');
  });
  // Revision drift → staleness rescan (NEEDS.md §14: no push event yet).
  const unRevision = studioStore.subscribe(() => {
    rescanStaleness(proposalsStore, studioStore.getState().revision);
  });
  bound = true;
  return () => {
    unControl();
    unTelemetry();
    unLost();
    unRevision();
    bound = false;
  };
}

export function useFeatureStores(): void {
  React.useEffect(() => {
    if (bound) return undefined;
    return bindFeatureStores();
  }, []);
}

// -- selection → facts ---------------------------------------------------------

/** Parsed clips for the currently selected track (CLIP_LIST view). */
export function useSelectedTrackClips(): ClipView[] {
  const trackId = useEditor((s) => s.clipSelection.trackId);
  const views = useStudio((s) => s.views);
  return React.useMemo(() => {
    if (!trackId) return [];
    const entry = views[makeViewKey('CLIP_LIST', trackId)];
    const out: ClipView[] = [];
    for (const it of entry?.items ?? ([] as ReadItem[])) {
      const c = parseClipItem(it);
      if (c) out.push(c);
    }
    return out;
  }, [trackId, views]);
}

/** Facts needed for accept-time revalidation, resolved live. */
function proposalFacts(proposalClipId: string): {
  projectId: string;
  targetClipExists: boolean;
} {
  const s = studioStore.getState();
  const trackId = editorStore.getState().clipSelection.trackId;
  let exists = false;
  if (trackId) {
    const entry = s.views[makeViewKey('CLIP_LIST', trackId)];
    const clips: ClipView[] = [];
    for (const it of entry?.items ?? []) {
      const c = parseClipItem(it);
      if (c) clips.push(c);
    }
    exists = targetClipExists(clips, proposalClipId);
  }
  return { projectId: s.projectId ?? '', targetClipExists: exists };
}

// -- actions the screens call --------------------------------------------------

export async function acceptProposal(proposalId: string): Promise<{
  ok: boolean;
  reason?: string;
}> {
  const rec = proposalsStore.getState().records[proposalId];
  if (!rec) return { ok: false, reason: 'suggestion no longer exists' };
  return acceptProposalCandidate(
    getClient(),
    proposalsStore,
    proposalId,
    proposalFacts(rec.context?.clipId ?? ''),
  );
}

export function dismiss(proposalId: string): void {
  dismissProposal(proposalsStore, proposalId);
}

export async function undoAccept(proposalId: string): Promise<{
  ok: boolean;
  reason?: string;
}> {
  return undoAcceptedTransaction(getClient(), proposalsStore, proposalId);
}

export const go = (id: string): void => {
  window.location.hash = `#/${id}`;
};
