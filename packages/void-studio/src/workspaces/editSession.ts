// Shared persistent-edit session for the studio editors.
//
// CONTRACTS.md §3: every PersistentCommand carries expected_revision; one
// user gesture == one transaction_id; conflicts come back as REJECTED
// receipts carrying the CURRENT revision — never silently retry with a
// guessed revision, and never retry STALE_EPOCH (the engine restart means
// the session must re-attach, not re-issue).
//
// `sendWithStaleRetry` implements the W09 rule: on STALE_REVISION, re-read
// the affected view (the caller's `refresh`) and retry ONCE against the
// revision the receipt reported. Any other failure surfaces verbatim.

import type {
  CommandReceipt,
  PersistentOp,
  VoidClient,
} from 'void-client';

export interface EditIssue {
  /** One logical gesture id shared by every op it emits. */
  transactionId?: string;
  /** Re-read the view(s) this edit touched; runs before the stale retry. */
  refresh?: () => Promise<unknown> | unknown;
}

export interface EditOutcome {
  receipt: CommandReceipt;
  /** true when a STALE_REVISION conflict caused a re-read + second send. */
  retried: boolean;
}

export function receiptFailed(r: CommandReceipt): boolean {
  return r.status === 'REJECTED' || r.status === 'OUTCOME_UNKNOWN';
}

/**
 * Issue one persistent op. Returns the last receipt; `retried` records
 * whether the single sanctioned retry ran. Callers render optimistic
 * previews and must revert them when the outcome is a failure receipt.
 */
export async function sendWithStaleRetry(
  client: VoidClient,
  op: PersistentOp,
  issue: EditIssue = {},
): Promise<EditOutcome> {
  const first = await client.sendCommand(op, {
    transactionId: issue.transactionId,
  });
  const stale =
    first.status === 'REJECTED' && first.error === 'STALE_REVISION';
  if (!stale) return { receipt: first, retried: false };

  // The receipt's revision is the authoritative current revision —
  // re-read the touched projection, then retry once against it.
  await issue.refresh?.();
  const second = await client.sendCommand(op, {
    transactionId: issue.transactionId,
    expectedRevision: first.revision,
  });
  return { receipt: second, retried: true };
}

/** Safe user-facing description of a failed receipt (message is sanitized by the coordinator). */
export function describeReceiptError(r: CommandReceipt): string {
  if (r.status === 'REJECTED') {
    return r.message
      ? `${r.error}: ${r.message}`
      : `edit rejected (${r.error})`;
  }
  if (r.status === 'OUTCOME_UNKNOWN') {
    return 'edit outcome unknown — reconcile before retrying';
  }
  return '';
}
