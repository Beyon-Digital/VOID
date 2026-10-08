// Jobs view store (W12) — zustand vanilla store holding the job list,
// live progress and budget badge state. The runner/coordinator remains
// the authority: cards are folded from JOB_LIST payloads + JobEvent
// telemetry; local mutations only ever mirror what arrived on the wire.

import { createStore, StoreApi } from 'zustand/vanilla';
import type { JobEventDto } from '../export/job';
import {
  applyJobEventToCard,
  isTerminal,
  jobEventOf,
  parseJobCard,
  type JobCard,
} from './job';

export interface JobsViewState {
  /** jobId → card, order kept by the list payload. */
  cards: Record<string, JobCard>;
  order: string[];
  /** True once the coordinator answered one JOB_LIST read. */
  loaded: boolean;
}

export interface JobsActions {
  /** Replace the list from a JOB_LIST payload (defensive parse). */
  ingestList(payload: unknown): number;
  /** Fold one telemetry payload — job events only, others ignored. */
  ingestTelemetry(payload: unknown): boolean;
  /** Apply an already-parsed job event. */
  applyEvent(ev: JobEventDto): void;
  /** Optimistic cancel echo — queued→cancelled, running→cancelling. */
  markCancelling(jobId: string): void;
  /** Attach artifacts when the result card arrives. */
  setArtifacts(jobId: string, artifacts: JobCard['artifacts']): void;
  reset(): void;
}

export type JobsStore = StoreApi<JobsViewState & { actions: JobsActions }>;

export function createJobsStore(): JobsStore {
  return createStore<JobsViewState & { actions: JobsActions }>()((set, get) => ({
    cards: {},
    order: [],
    loaded: false,
    actions: {
      ingestList(payload) {
        const rows = Array.isArray(payload)
          ? payload
          : typeof payload === 'object' && payload !== null
            ? ((payload as Record<string, unknown>).jobs ??
              (payload as Record<string, unknown>).rows ??
              [])
            : [];
        if (!Array.isArray(rows)) return 0;
        const cards: Record<string, JobCard> = {};
        const order: string[] = [];
        for (const row of rows) {
          const card = parseJobCard(row);
          if (card) {
            // Keep live progress a telemetry stream already delivered —
            // list rows may lag a few hundred ms behind the last event.
            const prev = get().cards[card.jobId];
            cards[card.jobId] =
              prev && prev.percent !== null && card.percent === null
                ? { ...card, percent: prev.percent, message: prev.message ?? card.message }
                : card;
            order.push(card.jobId);
          }
        }
        set({ cards, order, loaded: true });
        return order.length;
      },
      ingestTelemetry(payload) {
        const ev = jobEventOf(payload);
        if (!ev) return false;
        get().actions.applyEvent(ev);
        return true;
      },
      applyEvent(ev) {
        const card = get().cards[ev.job_id];
        if (!card) {
          // Unknown job: create a minimal card only from event facts —
          // a job card is never fabricated beyond its own identity.
          const stub = parseJobCard({
            jobId: ev.job_id,
            projectId: ev.project_id,
            status: ev.status,
          });
          if (!stub) return;
          set((s) => ({
            cards: { ...s.cards, [ev.job_id]: stub },
            order: [...s.order, ev.job_id],
          }));
          return;
        }
        set((s) => ({
          cards: { ...s.cards, [ev.job_id]: applyJobEventToCard(card, ev) },
        }));
      },
      markCancelling(jobId) {
        const card = get().cards[jobId];
        if (!card || isTerminal(card.status)) return;
        const status =
          card.status === 'queued' ? ('cancelled' as const) : ('cancelling' as const);
        set((s) => ({
          cards: { ...s.cards, [jobId]: { ...card, status } },
        }));
      },
      setArtifacts(jobId, artifacts) {
        const card = get().cards[jobId];
        if (!card) return;
        set((s) => ({ cards: { ...s.cards, [jobId]: { ...card, artifacts } } }));
      },
      reset() {
        set({ cards: {}, order: [], loaded: false });
      },
    },
  }));
}
