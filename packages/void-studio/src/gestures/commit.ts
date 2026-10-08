// Gesture commit bridge (GEST-05, T57): preview → one transaction of
// ordinary InsertNoteOps; undo through the normal UndoOp path.
//
// Same discipline as proposals/bridge.ts: mint all ids HERE, send each
// op with the shared transaction id, stop at the first non-ok receipt.
// The store records the commit via markCommitted() so undo targets this
// gesture exactly — one undo reverses the phrase without touching any
// earlier take (GEST-05 acceptance).

import type { CommandReceipt, PersistentOp, VoidClient } from 'void-client';
import { sendWithStaleRetry, describeReceiptError } from '../workspaces/editSession';
import { opsForPreview, type GestureSessionStore } from './session';
import type { GestureNote } from './types';

export interface GestureCommitPlan {
  transactionId: string;
  clipId: string;
  noteIds: string[];
  ops: PersistentOp[];
}

export class GestureCommitError extends Error {
  readonly receipts: CommandReceipt[];
  constructor(msg: string, receipts: CommandReceipt[]) {
    super(msg);
    this.receipts = receipts;
  }
}

/**
 * Plan a commit from the session's current preview. Pure — no sends.
 * Returns null when there is nothing to commit (not armed/preview).
 */
export function planGestureCommit(
  session: GestureSessionStore,
  mint: () => string,
): GestureCommitPlan | null {
  const s = session.getState();
  if ((s.phase !== 'preview' && s.phase !== 'committed') || !s.target) {
    return null;
  }
  if (s.preview.length === 0) return null;
  const noteIds = s.preview.map(() => mint());
  return {
    transactionId: mint(),
    clipId: s.target.clipId,
    noteIds,
    ops: opsForPreview(s.target.clipId, s.preview, noteIds),
  };
}

/**
 * Send the plan. Every InsertNoteOp shares the plan's transaction id —
 * one undoable gesture (CONTRACTS.md §5). STALE_REVISION gets the
 * standard single refresh+retry; anything else surfaces verbatim.
 */
export async function applyGestureCommit(
  session: GestureSessionStore,
  client: VoidClient,
  plan: GestureCommitPlan,
  refresh?: () => Promise<unknown> | unknown,
): Promise<CommandReceipt[]> {
  const receipts: CommandReceipt[] = [];
  for (const op of plan.ops) {
    const out = await sendWithStaleRetry(client, op, {
      transactionId: plan.transactionId,
      refresh,
    });
    receipts.push(out.receipt);
    if (out.receipt.status !== 'APPLIED' && out.receipt.status !== 'DUPLICATE') {
      session.getState().actions.setError(describeReceiptError(out.receipt));
      throw new GestureCommitError(
        `gesture commit failed: ${describeReceiptError(out.receipt)}`,
        receipts,
      );
    }
  }
  session.getState().actions.markCommitted(plan.transactionId, plan.noteIds);
  return receipts;
}

/**
 * Undo the session's committed phrase through the NORMAL path — one
 * UndoOp addressed at the commit's transaction id. The store keeps the
 * raw gesture so the user can adjust transforms and re-commit without
 * re-capturing.
 */
export function undoGestureCommit(
  session: GestureSessionStore,
  client: VoidClient,
): Promise<CommandReceipt> | null {
  const commit = session.getState().lastCommit;
  if (!commit) return null;
  return client.undo(commit.transactionId);
}

/** Convenience: plan + apply in one call. */
export async function commitGesture(
  session: GestureSessionStore,
  client: VoidClient,
  mint: () => string,
  refresh?: () => Promise<unknown> | unknown,
): Promise<GestureCommitPlan | null> {
  const plan = planGestureCommit(session, mint);
  if (!plan) return null;
  await applyGestureCommit(session, client, plan, refresh);
  return plan;
}
