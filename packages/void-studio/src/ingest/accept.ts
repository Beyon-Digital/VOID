// Proposal acceptance orchestration (UI-T12..T16).
//
// Turns a ghost-note selection into real engine edits:
//   selection → planAcceptLocal (revalidation: project, target clip,
//   context bytes) → applyAcceptPlan (InsertNoteOps sharing one
//   transaction id) → record marked accepted with the transaction id.
//
// One accepted transaction = one undo action (client.undo(txId)).
// Originals are never touched before acceptance; locked-range ghosts
// are dropped by the planner, not inserted.

import type { VoidClient } from 'void-client';
import type { ProposalsStore } from '../proposals/store';
import type { ProposalRecord } from '../proposals/types';
import {
  AcceptPlanError,
  applyAcceptPlan,
  planAcceptLocal,
} from '../proposals/bridge';
import { latestContextSha } from './stale';

export interface AcceptOutcome {
  ok: boolean;
  /** PlanError kind or a human-readable refusal reason. */
  reason?: string;
  transactionId?: string;
  noteIds?: string[];
  droppedIndices?: number[];
}

export interface AcceptFacts {
  /** Open project id — must equal record.projectId. */
  projectId: string;
  /** Whether the context clip still exists (from real CLIP_LIST). */
  targetClipExists: boolean;
}

const REASON_LABEL: Record<string, string> = {
  'not-ready': 'suggestion is not in a previewable state',
  'wrong-project': 'a different project is open',
  'target-gone': 'the target clip was removed',
  'context-changed': 'the region changed since generation',
  'bad-selection': 'nothing usable is selected',
  'empty-plan': 'every selected note overlaps a locked range',
};

/**
 * Accept the selected candidate (or an explicit subset of ghost
 * indices) for a proposal record. Resolves the record, revalidates
 * against live facts, sends the InsertNoteOps under one transaction,
 * then marks the record accepted in the view store.
 */
export async function acceptProposalCandidate(
  client: VoidClient,
  proposals: ProposalsStore,
  proposalId: string,
  facts: AcceptFacts,
): Promise<AcceptOutcome> {
  const state = proposals.getState();
  const rec = state.records[proposalId];
  if (!rec) return { ok: false, reason: 'suggestion no longer exists' };

  const sel = state.selection;
  const candidateRank =
    sel && sel.proposalId === proposalId ? sel.candidateRank : undefined;
  const indices =
    sel && sel.proposalId === proposalId ? sel.indices : undefined;
  const rank = candidateRank ?? rec.candidates[0]?.rank ?? 1;

  const currentContextSha256 =
    latestContextSha(state.records, rec.context?.clipId ?? '') ??
    rec.contextSha256;

  let plan;
  try {
    plan = planAcceptLocal(
      rec,
      rank,
      indices && indices.length > 0 ? indices : undefined,
      {
        projectId: facts.projectId,
        currentContextSha256,
        targetClipExists: facts.targetClipExists,
      },
    );
  } catch (e) {
    if (e instanceof AcceptPlanError) {
      // A refused accept marks the record honestly when the cause is
      // a staleness fact the store can keep for the user.
      if (e.kind === 'target-gone')
        state.actions.markStale('target_gone', proposalId);
      else if (e.kind === 'context-changed')
        state.actions.markStale('context_changed', proposalId);
      return { ok: false, reason: REASON_LABEL[e.kind] ?? e.message };
    }
    return { ok: false, reason: String(e) };
  }

  state.actions.setBusy(true);
  try {
    const { noteIds } = await applyAcceptPlan(client, plan);
    markAccepted(
      proposals,
      rec,
      plan.candidateRank,
      plan.transactionId,
      noteIds,
      plan.droppedIndices,
    );
    // rev-2 (NEEDS §14): ResolveProposalOp — server-side bookkeeping
    // for the committed notes. Best-effort: the edits already landed,
    // and the next PROPOSAL_LIST read reconciles status if the
    // coordinator rejects the resolve.
    try {
      await client.sendCommand({
        ResolveProposalOp: {
          proposal_id: rec.proposalId,
          accept: true,
          candidate_rank: plan.candidateRank,
        },
      });
    } catch {
      /* bookkeeping only — PROPOSAL_LIST is the reconciling read */
    }
    return {
      ok: true,
      transactionId: plan.transactionId,
      noteIds,
      droppedIndices: plan.droppedIndices,
    };
  } catch (e) {
    return {
      ok: false,
      reason:
        e instanceof AcceptPlanError
          ? `engine rejected the edit (${e.kind})`
          : String(e),
    };
  } finally {
    proposals.getState().actions.setBusy(false);
  }
}

/** Record the accepted transaction on the record — the audit trail
 * the inspector + undo affordance read. View state only; the document
 * change already committed through the coordinator. */
export function markAccepted(
  proposals: ProposalsStore,
  rec: ProposalRecord,
  candidateRank: number,
  transactionId: string,
  noteIds: string[],
  droppedIndices: number[],
): void {
  proposals.getState().actions.upsert({
    proposalId: rec.proposalId,
    projectId: rec.projectId,
    sourceRevision: rec.sourceRevision,
    contextSha256: rec.contextSha256,
    status: 'accepted',
    context: rec.context,
    candidates: rec.candidates,
    provenance: rec.provenance,
    accepted: {
      transactionId,
      candidateRank,
      noteIds,
      droppedIndices,
      acceptedAt: new Date().toISOString(),
    },
    createdAt: rec.createdAt,
    updatedAt: new Date().toISOString(),
  });
}

/** One accepted transaction = one undo action. Reverts every insert
 * minted under the transaction id, then returns the record to ready
 * so the user can re-audit or dismiss the preview. */
export async function undoAcceptedTransaction(
  client: VoidClient,
  proposals: ProposalsStore,
  proposalId: string,
): Promise<AcceptOutcome> {
  const rec = proposals.getState().records[proposalId];
  if (!rec?.accepted?.transactionId)
    return { ok: false, reason: 'nothing committed to undo' };
  try {
    await client.undo(rec.accepted.transactionId);
  } catch (e) {
    return { ok: false, reason: `undo failed: ${String(e)}` };
  }
  proposals.getState().actions.upsert({
    proposalId: rec.proposalId,
    projectId: rec.projectId,
    sourceRevision: rec.sourceRevision,
    contextSha256: rec.contextSha256,
    status: 'ready',
    context: rec.context,
    candidates: rec.candidates,
    provenance: rec.provenance,
    createdAt: rec.createdAt,
    updatedAt: new Date().toISOString(),
  });
  return { ok: true };
}

/**
 * Dismiss a proposal's ghost preview. Ghost notes are view state, so
 * dropping them never touches committed notes — dismiss-after-partial
 * removes only ghosts (UI-T16). The caller sends ResolveProposalOp
 * {accept:false} for the coordinator-side bookkeeping (rev-2,
 * NEEDS.md §14); the store drops the record from view regardless.
 */
export function dismissProposal(
  proposals: ProposalsStore,
  proposalId: string,
): void {
  const rec = proposals.getState().records[proposalId];
  if (!rec) return;
  proposals.getState().actions.remove(proposalId);
}
