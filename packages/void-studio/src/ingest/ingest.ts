// Control/telemetry event → feature-store routing for the Signal Studio
// proposal / jobs / generation surfaces.
//
// The coordinator does not yet emit dedicated proposal or job-list events
// (docs/engine/NEEDS.md §9–14), so every payload here is decoded
// defensively: anything that does not parse is dropped, never guessed.
// When the real wire events land these routers already fold them into the
// view stores the screens read.

import type { ProposalsStore } from '../proposals/store';
import type { JobsStore } from '../jobs/store';
import { jobEventOf } from '../jobs/job';
import type { GenerationStore } from '../generation/store';

/** Shape of a decoded control/telemetry event envelope as emitted by
 * void-client listeners — `kind` plus a free-form payload body. */
export interface IngestEvent {
  kind?: string;
  [k: string]: unknown;
}

const isObj = (v: unknown): v is Record<string, unknown> =>
  typeof v === 'object' && v !== null;

/**
 * Extract a proposal-record payload from a control event. Accepts the
 * current coordinator shapes — `{kind:'proposal', record:{…}}`,
 * `{kind:'proposal', proposal:{…}}`, a bare record-shaped body — and
 * returns null for anything else. The store's own parser decides
 * validity.
 */
function proposalPayloadOf(ev: IngestEvent): unknown | null {
  const kind = typeof ev.kind === 'string' ? ev.kind : '';
  if (kind === 'proposal_list' || kind === 'proposals') {
    return {
      __list: true,
      rows: ev.proposals ?? ev.records ?? ev.rows ?? [],
    };
  }
  if (kind === 'proposal_removed' || kind === 'proposal_resolved') {
    return { __removed: true, proposalId: ev.proposal_id ?? ev.proposalId };
  }
  if (kind === 'proposal' || kind === 'proposal_record') {
    return ev.record ?? ev.proposal ?? ev;
  }
  return null;
}

/**
 * Fold one decoded control event into the proposals store.
 * Returns true when the event carried proposal data.
 */
export function ingestControlEvent(
  ev: IngestEvent,
  proposals: ProposalsStore,
): boolean {
  const payload = proposalPayloadOf(ev);
  if (payload === null) return false;
  const actions = proposals.getState().actions;
  if (isObj(payload) && payload.__list === true) {
    actions.ingestList(payload.rows);
    return true;
  }
  if (isObj(payload) && payload.__removed === true) {
    const id = payload.proposalId;
    if (typeof id === 'string') actions.remove(id);
    return true;
  }
  return actions.upsert(payload);
}

/**
 * Fold one decoded telemetry event into the jobs + generation stores.
 * The generation store only tracks jobs it has a request record for, so
 * both get the same event — foreign job ids are ignored there.
 */
export function ingestTelemetryEvent(
  ev: IngestEvent,
  jobs: JobsStore,
  generation: GenerationStore,
): boolean {
  const je = jobEventOf(ev);
  if (!je) return false;
  jobs.getState().actions.applyEvent(je);
  generation.getState().actions.applyEvent(je);
  return true;
}

/** Fold a job-list page (JOB_LIST view rows, when the wire view exists)
 * into the jobs store. */
export function ingestJobList(items: unknown, jobs: JobsStore): number {
  return jobs.getState().actions.ingestList(items);
}
