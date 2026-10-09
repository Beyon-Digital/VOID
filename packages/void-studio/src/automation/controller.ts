// Mixed-transaction sends and one-step undo (W18; MIX-08, T70).
//
// CONTRACTS §5: one user gesture == one transaction_id; the engine
// rewinds a whole transaction on a single UndoOp. A mixer+arrangement
// gesture (e.g. move a region AND ride a fader, or flip an A/B state)
// sends every op under the SAME transactionId, so `undoTransaction`
// rewinds the mixed gesture in one step — no client-side undo stack.

import type { CommandReceipt, PersistentOp, VoidClient } from 'void-client';
import { sendWithStaleRetry, type EditOutcome } from '../workspaces/editSession';

export interface MixedTransaction {
  /** One logical gesture id shared by every op it emits. */
  transactionId: string;
  ops: PersistentOp[];
}

export interface TransactionOutcome {
  receipts: EditOutcome[];
  /** true when every receipt APPLIED or dedup'd as DUPLICATE. */
  settled: boolean;
}

/**
 * Issue a mixed mixer/arrangement/automation gesture as ONE
 * transaction. Sends are sequential — the coordinator serializes
 * mutations anyway, and a serial caller observes each receipt.
 */
export async function sendAsOneTransaction(
  client: VoidClient,
  tx: MixedTransaction,
  refresh?: () => Promise<unknown> | unknown,
): Promise<TransactionOutcome> {
  const receipts: EditOutcome[] = [];
  for (const op of tx.ops) {
    receipts.push(
      await sendWithStaleRetry(client, op, {
        transactionId: tx.transactionId,
        refresh,
      }),
    );
  }
  const settled = receipts.every(
    (r) => r.receipt.status === 'APPLIED' || r.receipt.status === 'DUPLICATE',
  );
  return { receipts, settled };
}

/**
 * Rewind an entire transaction in ONE step — one UndoOp carrying the
 * transaction id. Mixer, arrangement and automation ops issued under
 * that id roll back together (T70 "one coherent history").
 */
export function undoTransaction(
  client: VoidClient,
  transactionId: string,
): Promise<CommandReceipt> {
  return client.undo(transactionId);
}
