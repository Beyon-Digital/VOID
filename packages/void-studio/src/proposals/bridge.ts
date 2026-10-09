// Accept/reject bridge — turns a validated selection into the existing
// PersistentCommand path (W13: accept → InsertNoteOps, ONE transaction
// id, undo = one UndoOp; reject → record intent, nothing applied).
// Revalidation inputs must be answered live by the caller (crate-side
// plan_accept revalidates again inside accept — this is the TS mirror).

import type { VoidClient } from 'void-client';
import type { CommandReceipt } from 'void-client';
import { ghostNotesOf, type GhostNote } from './ghost';
import type { ProposalRecord, StaleCause } from './types';

export interface RevalidationInput {
  projectId: string;
  /** sha256 of the *current* region context, recomputed now. */
  currentContextSha256: string;
  targetClipExists: boolean;
}

export interface PlannedInsert {
  clipId: string;
  noteId: string;
  pitch: number;
  velocity: number;
  startTicks: string;
  lengthTicks: string;
}

export interface LocalAcceptPlan {
  transactionId: string;
  proposalId: string;
  candidateRank: number;
  inserts: PlannedInsert[];
  droppedIndices: number[];
}

export type PlanError =
  | 'not-ready'
  | 'wrong-project'
  | 'target-gone'
  | 'context-changed'
  | 'bad-selection'
  | 'empty-plan';

export class AcceptPlanError extends Error {
  readonly kind: PlanError;
  constructor(kind: PlanError, msg: string) {
    super(msg);
    this.kind = kind;
  }
}

function uuid(): string {
  const c = (globalThis as { crypto?: { randomUUID?: () => string } })
    .crypto;
  if (c?.randomUUID) return c.randomUUID();
  return 'xxxxxxxx-xxxx-4xxx-yxxx-xxxxxxxxxxxx'.replace(/[xy]/g, (ch) => {
    const r = (Math.random() * 16) | 0;
    return (ch === 'x' ? r : (r & 0x3) | 0x8).toString(16);
  });
}

/** Mirrors crates/void-proposals plan_accept (T55/T54): freshness first,
 * locked-range notes dropped (never inserted), ids minted HERE. */
export function planAcceptLocal(
  rec: ProposalRecord,
  candidateRank: number,
  indices: number[] | undefined,
  reval: RevalidationInput,
  mint: () => string = uuid,
): LocalAcceptPlan {
  if (rec.status !== 'ready')
    throw new AcceptPlanError('not-ready', `status ${rec.status}`);
  if (reval.projectId !== rec.projectId)
    throw new AcceptPlanError('wrong-project', 'different project open');
  if (!reval.targetClipExists)
    throw new AcceptPlanError('target-gone', 'target clip gone');
  if (reval.currentContextSha256 !== rec.contextSha256)
    throw new AcceptPlanError('context-changed', 'region changed');
  const cand = rec.candidates.find((c) => c.rank === candidateRank);
  if (!cand) throw new AcceptPlanError('bad-selection', 'rank missing');
  const want =
    indices && indices.length > 0
      ? [...indices].sort((a, b) => a - b)
      : cand.notes.map((_, i) => i);
  if (want.some((i) => i < 0 || i >= cand.notes.length))
    throw new AcceptPlanError('bad-selection', 'index out of range');
  const ghosts = ghostNotesOf(cand, rec.context?.lockedRanges ?? []);
  const clipId = rec.context?.clipId ?? '';
  const inserts: PlannedInsert[] = [];
  const dropped: number[] = [];
  for (const i of want) {
    const g = ghosts[i];
    if (!g) throw new AcceptPlanError('bad-selection', 'index out of range');
    if (g.locked) {
      dropped.push(i);
      continue;
    }
    const n = cand.notes[i];
    inserts.push({
      clipId,
      noteId: mint(),
      pitch: n.pitch,
      velocity: n.velocity,
      startTicks: n.onsetTicks,
      lengthTicks: n.lengthTicks,
    });
  }
  if (inserts.length === 0)
    throw new AcceptPlanError('empty-plan', 'nothing insertable');
  return {
    transactionId: mint(),
    proposalId: rec.proposalId,
    candidateRank,
    inserts,
    droppedIndices: dropped,
  };
}

/** Apply the plan through sendCommand: every InsertNoteOp shares the
 * plan's transaction id — one undoable gesture (CONTRACTS.md §5). */
export async function applyAcceptPlan(
  client: VoidClient,
  plan: LocalAcceptPlan,
): Promise<{ receipts: CommandReceipt[]; noteIds: string[] }> {
  const receipts: CommandReceipt[] = [];
  for (const ins of plan.inserts) {
    const receipt = await client.sendCommand(
      {
        InsertNoteOp: {
          clip_id: ins.clipId,
          note_id: ins.noteId,
          pitch: ins.pitch,
          velocity: ins.velocity,
          start_ticks: ins.startTicks,
          length_ticks: ins.lengthTicks,
        },
      },
      { transactionId: plan.transactionId },
    );
    receipts.push(receipt);
    if (receipt.status !== 'APPLIED' && receipt.status !== 'DUPLICATE') {
      throw new AcceptPlanError(
        'bad-selection',
        `insert rejected: ${receipt.status}`,
      );
    }
  }
  return { receipts, noteIds: plan.inserts.map((i) => i.noteId) };
}

/** Stale-cause mapping for workspace events (T55): region/clip edits →
 * context_changed; target removed → target_gone; close/epoch →
 * session_ended. */
export function causeForEdit(kind: 'notes' | 'clip-removed' | 'session'): StaleCause {
  switch (kind) {
    case 'notes':
      return 'context_changed';
    case 'clip-removed':
      return 'target_gone';
    case 'session':
      return 'session_ended';
  }
}
