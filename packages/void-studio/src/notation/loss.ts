// Loss-report surface for the score editor (W25, T91) — the same
// visible-loss contract as exchange/: every dropped/approximated entry
// renders, grouped deterministically for the dialog.

import type { LossEntry, LossReport } from '../exchange';
import { lossSummary } from '../exchange';

export interface LossRow {
  element: string;
  aspect: string;
  kind: LossEntry['kind'];
  reason: string;
}

export interface LossGroup {
  /** Group header — aspect bucket (e.g. "ornaments", "duration"). */
  aspect: string;
  rows: LossRow[];
}

/** Entries → display rows, in the report's canonical order. */
export function lossRows(report: LossReport | null): LossRow[] {
  if (!report) return [];
  return report.entries.map((e) => ({
    element: e.element,
    aspect: e.aspect,
    kind: e.kind,
    reason: e.reason,
  }));
}

/** Rows grouped by aspect — stable first-seen order, no sorting tricks:
 *  what the report says is what the user reads. */
export function groupedLoss(report: LossReport | null): LossGroup[] {
  const rows = lossRows(report);
  const groups = new Map<string, LossRow[]>();
  for (const r of rows) {
    const bucket = groups.get(r.aspect) ?? [];
    bucket.push(r);
    groups.set(r.aspect, bucket);
  }
  return [...groups.entries()].map(([aspect, rs]) => ({ aspect, rows: rs }));
}

export { lossSummary };

/** Apply gating: a non-empty report needs an explicit acknowledge
 *  before the staged plan may proceed (mirrors importDialog's rule —
 *  silent-loss imports are not allowed to sail through). */
export function lossNeedsAcknowledgement(report: LossReport | null): boolean {
  if (!report) return false;
  return report.entries.length > 0;
}
