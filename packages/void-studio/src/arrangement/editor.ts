// Arrangement commit paths (W17): plan -> send under one
// transactionId via sendWithStaleRetry, same discipline as
// gestures/commit.ts. Stops on the first non-ok receipt; the
// thrown error carries every receipt so the caller can surface
// which op failed and let undo take the partial gesture back.

import type { CommandReceipt, PersistentOp, VoidClient } from 'void-client';
import { receiptFailed, sendWithStaleRetry } from '../workspaces/editSession';

export class ArrangementCommitError extends Error {
  readonly receipts: CommandReceipt[];
  constructor(message: string, receipts: CommandReceipt[]) {
    super(message);
    this.name = 'ArrangementCommitError';
    this.receipts = receipts;
  }
}

/**
 * Send a plan's ops under its own transactionId. `refresh` re-reads
 * the affected view pages after a STALE_REVISION receipt (same
 * contract as editSession).
 */
export async function sendPlan(
  client: VoidClient,
  plan: { transactionId: string; ops: PersistentOp[] },
  opts: { refresh?: () => Promise<void> } = {},
): Promise<CommandReceipt[]> {
  const receipts: CommandReceipt[] = [];
  for (const op of plan.ops) {
    const out = await sendWithStaleRetry(client, op, {
      transactionId: plan.transactionId,
      ...(opts.refresh !== undefined ? { refresh: opts.refresh } : {}),
    });
    receipts.push(out.receipt);
    if (receiptFailed(out.receipt)) {
      throw new ArrangementCommitError(
        `op failed in transaction ${plan.transactionId}: ${out.receipt.status}/${out.receipt.error}`,
        receipts,
      );
    }
  }
  return receipts;
}

/** Undo the transaction a plan was applied under. */
export function undoPlan(client: VoidClient, transactionId: string): Promise<CommandReceipt> {
  return client.undo(transactionId);
}
