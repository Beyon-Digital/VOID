// Comp commit lane (W17/REC-05; T66).
//
// Same discipline as gestures/commit.ts and proposals/bridge.ts:
// build the whole op list first (pure), then send every op under ONE
// transactionId via sendWithStaleRetry, stopping at the first failed
// receipt. Undo is the normal UndoOp path — `client.undo(txId)`
// reverses the entire comp apply.

import type { CommandReceipt, VoidClient } from 'void-client';
import {
  describeReceiptError,
  receiptFailed,
  sendWithStaleRetry,
} from '../workspaces/editSession';
import type { ClipView } from '../timeline/geometry';
import { compToOps, type CompApplyPlan } from './comp';
import type { CompSpec, TakeFolder } from './types';

export class CompApplyError extends Error {
  readonly receipts: CommandReceipt[];
  constructor(msg: string, receipts: CommandReceipt[]) {
    super(msg);
    this.receipts = receipts;
  }
}

/** Build the apply plan — pure, sends nothing. */
export function planCompApply(
  spec: CompSpec,
  folder: TakeFolder,
  target: { clips: ClipView[] },
  mint: () => string,
  opts: { seamShape?: 'equal-power' | 'equal-gain' | 'linear'; maxFadeTicks?: string } = {},
): CompApplyPlan {
  return compToOps(spec, folder, target, mint, opts);
}

/**
 * Send the plan. Every op shares `plan.transactionId` — one undoable
 * gesture (CONTRACTS §5). On failure the caller surfaces
 * describeReceiptError and leaves the take folder untouched.
 */
export async function applyCompPlan(
  client: VoidClient,
  plan: CompApplyPlan,
  refresh?: () => Promise<unknown> | unknown,
): Promise<CommandReceipt[]> {
  const receipts: CommandReceipt[] = [];
  for (const op of plan.ops) {
    const out = await sendWithStaleRetry(client, op, {
      transactionId: plan.transactionId,
      refresh,
    });
    receipts.push(out.receipt);
    if (receiptFailed(out.receipt)) {
      throw new CompApplyError(
        `comp apply failed: ${describeReceiptError(out.receipt)}`,
        receipts,
      );
    }
  }
  return receipts;
}

/** Roll back an applied comp through the normal undo path. */
export function undoCompApply(
  client: VoidClient,
  plan: CompApplyPlan,
): Promise<CommandReceipt> {
  return client.undo(plan.transactionId);
}
