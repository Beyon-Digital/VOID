// Staleness detection for proposal records (UI-T15).
//
// A proposal is bound to the document revision it was generated
// against (`sourceRevision`) plus the context bytes the generator saw
// (`contextSha256`). Any later edit, clip removal, or session change
// must refuse acceptance — never silently rebase musical edits.
//
// The wire carries ProposalStaleEvent (rev-2); the local rescan remains
// a second detector for drift between push frames. Facts observed:
//   • revision drift — doc revision moved past sourceRevision
//   • revision drift — doc revision moved past sourceRevision
//   • context churn — a newer record for the same context bytes
//   • target removal — the context clip vanished from CLIP_LIST

import type { ProposalsStore } from '../proposals/store';
import type { ProposalRecord, StaleCause } from '../proposals/types';

const LIVE = new Set(['pending', 'ready']);

const asBig = (v: string): bigint | null => {
  try {
    return BigInt(v);
  } catch {
    return null;
  }
};

/**
 * Re-scan every live record against the current document revision and
 * mark drifted proposals stale. Returns the count newly marked.
 * Called whenever the coordinator revision advances.
 */
export function rescanStaleness(
  proposals: ProposalsStore,
  currentRevision: string | bigint,
): number {
  const rev = typeof currentRevision === 'bigint' ? currentRevision : asBig(currentRevision);
  if (rev === null) return 0;
  const { records, actions } = proposals.getState();
  let marked = 0;
  for (const rec of Object.values(records)) {
    if (!LIVE.has(rec.status)) continue;
    const src = asBig(rec.sourceRevision);
    if (src !== null && src < rev) {
      marked += actions.markStale('context_changed', rec.proposalId);
    }
  }
  return marked;
}

/**
 * Newest live contextSha256 for a context clip — when a newer record
 * for the same clip carries different context bytes, the older one has
 * been superseded (accept-time `context-changed` rejection).
 */
export function latestContextSha(
  records: Record<string, ProposalRecord>,
  clipId: string,
): string | null {
  let newest: ProposalRecord | null = null;
  for (const rec of Object.values(records)) {
    if (!LIVE.has(rec.status)) continue;
    if ((rec.context?.clipId ?? '') !== clipId) continue;
    if (!newest || rec.createdAt > newest.createdAt) newest = rec;
  }
  return newest ? newest.contextSha256 : null;
}

/**
 * Whether the proposal's target clip still exists, from an already
 * parsed CLIP_LIST view (ClipView[]). A missing target marks the
 * proposal `target_gone` at accept time.
 */
export function targetClipExists(
  clips: readonly { clipId: string }[],
  clipId: string,
): boolean {
  return clips.some((c) => c.clipId === clipId);
}

/**
 * Apply a confirmed staleness fact to the store — used by the app when
 * its own mutation receipts prove the context moved under a proposal
 * (e.g. a local edit receipt while a preview is live).
 */
export function markStaleFor(
  proposals: ProposalsStore,
  cause: StaleCause,
  proposalId?: string,
): number {
  return proposals.getState().actions.markStale(cause, proposalId);
}
