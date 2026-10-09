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
import { cancelJobOp, submitJobOp } from 'void-studio/src/jobs/job';
import type { JobSpecEnvelope } from 'void-studio/src/jobs/job';
import type { CommandReceipt, PreviewNoteDto } from 'void-client';
import type { StaleCause } from 'void-studio/src/proposals/types';
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
    const e = ev as unknown as Record<string, unknown>;
    ingestTelemetryEvent(e, jobsStore, generationStore);
    // rev-2 (NEEDS §14): pushed staleness — folds alongside the local
    // revision/context rescan; the store dedupes by proposal id.
    if (e.kind === 'ProposalStaleEvent') {
      const cause =
        typeof e.cause === 'string' ? (e.cause as StaleCause) : 'context_changed';
      const pid = typeof e.proposal_id === 'string' ? e.proposal_id : undefined;
      proposalsStore.getState().actions.markStale(cause, pid);
    }
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
  void refreshFeatureViews();
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

// -- rev-2 bounded view reads (JOB_LIST / MODEL_LIST / PROPOSAL_LIST) ---------

/** Read every page of a rev-2 view and return decoded summary rows.
 * Malformed rows drop; a coordinator that rejects the view name
 * surfaces an empty list — never a fabricated one. */
async function readViewRows(
  view:
    | 'JOB_LIST'
    | 'MODEL_LIST'
    | 'PROPOSAL_LIST'
    | 'TAKE_LIST'
    | 'INPUT_DEVICE_LIST'
    | 'SCENE_LIST',
  opts: { includeTerminal?: boolean } = {},
): Promise<unknown[]> {
  const rows: unknown[] = [];
  const client = getClient();
  for await (const page of client.readViewPages({
    view,
    include_terminal: opts.includeTerminal ?? false,
  })) {
    for (const item of page.items ?? []) {
      try {
        rows.push(JSON.parse(item.summary_json));
      } catch {
        /* malformed row — drop, never fake */
      }
    }
  }
  return rows;
}

/**
 * Pull the feature views the coordinator now serves (rev-2). Called
 * from bindFeatureStores on attach and by screens on focus; each read
 * is independent so one absent view never blanks the others.
 */
export async function refreshFeatureViews(): Promise<void> {
  const [jobs, models, proposals] = await Promise.all([
    readViewRows('JOB_LIST', { includeTerminal: true }).catch(() => null),
    readViewRows('MODEL_LIST').catch(() => null),
    readViewRows('PROPOSAL_LIST').catch(() => null),
  ]);
  if (jobs) jobsStore.getState().actions.ingestList(jobs);
  if (models) setModelRows(parseModelList(models));
  if (proposals) proposalsStore.getState().actions.ingestList(proposals);
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
  // rev-2: ResolveProposalOp{accept:false} is the coordinator-side
  // bookkeeping; the local ghost preview drops either way.
  const sel = proposalsStore.getState().selection;
  const rank =
    sel && sel.proposalId === proposalId ? sel.candidateRank : undefined;
  void getClient()
    .sendCommand({
      ResolveProposalOp: {
        proposal_id: proposalId,
        accept: false,
        candidate_rank: rank,
      },
    })
    .catch(() => undefined);
  dismissProposal(proposalsStore, proposalId);
}

// -- rev-2 proposal actions (NEEDS §12–14) -------------------------------------

/** sha256-hex of a request context — the coordinator treats it as an
 * opaque digest naming what the suggestions should continue. */
async function digestContext(context: Record<string, unknown>): Promise<string> {
  const bytes = new TextEncoder().encode(JSON.stringify(context));
  const hash = await crypto.subtle.digest('SHA-256', bytes);
  return Array.from(new Uint8Array(hash))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

/** RequestProposalOp — ask the engine for continuations of a context. */
export async function requestProposal(
  context: Record<string, unknown>,
  opts: { maxProposals?: number; seed?: string } = {},
): Promise<CommandReceipt> {
  return getClient().sendCommand({
    RequestProposalOp: {
      context_digest: await digestContext(context),
      seed: opts.seed,
      max_proposals: opts.maxProposals ?? 1,
    },
  });
}

/** RequestProposalOp over the current clip selection — the digest names
 * project + track + clip + revision so the coordinator can reject a
 * request made against stale context. */
export async function requestSuggestion(
  maxProposals = 3,
): Promise<CommandReceipt> {
  const s = studioStore.getState();
  const sel = editorStore.getState().clipSelection;
  return requestProposal(
    {
      project_id: s.projectId ?? '',
      track_id: sel.trackId ?? '',
      clip_id: sel.clipIds[0] ?? '',
      revision: s.revision,
    },
    { maxProposals },
  );
}

/** PreviewLayerOp — native audition of a candidate's ghost notes. */
export async function auditionProposal(
  proposalId: string,
  enable = true,
): Promise<CommandReceipt | null> {
  const state = proposalsStore.getState();
  const rec = state.records[proposalId];
  if (!rec) return null;
  const clipId = rec.context?.clipId ?? '';
  if (!clipId) return null;
  const rank =
    state.selection?.proposalId === proposalId
      ? state.selection.candidateRank
      : (rec.candidates[0]?.rank ?? 1);
  const cand = rec.candidates.find((c) => c.rank === rank) ?? rec.candidates[0];
  const notes: PreviewNoteDto[] = (cand?.notes ?? []).map((n) => ({
    note_id: '',
    pitch: n.pitch,
    velocity: n.velocity,
    start_ticks: n.onsetTicks,
    length_ticks: n.lengthTicks,
  }));
  return getClient().sendCommand({
    PreviewLayerOp: {
      proposal_id: proposalId,
      clip_id: clipId,
      notes,
      enable,
    },
  });
}

// -- rev-2 job actions (NEEDS §9–11) --------------------------------------------

/** SubmitJobOp — the pinned flat DTO form the codec accepts. */
export function submitJobSpec(spec: JobSpecEnvelope): Promise<CommandReceipt> {
  return getClient().sendCommand(submitJobOp(spec));
}

export function cancelJob(jobId: string): Promise<CommandReceipt> {
  return getClient().sendCommand(cancelJobOp(jobId));
}

/** PauseJobOp — the coordinator may reject; the receipt says so. */
export function pauseJob(jobId: string): Promise<CommandReceipt> {
  return getClient().sendCommand({ op: 'pause_job', jobId });
}

/** InstallModelOp for a registry row (MODEL_LIST). */
export function installModel(row: ModelRow): Promise<CommandReceipt> {
  return getClient().sendCommand({
    InstallModelOp: {
      model_id: row.modelId,
      model_version: row.version,
      source_uri: '',
    },
  });
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
