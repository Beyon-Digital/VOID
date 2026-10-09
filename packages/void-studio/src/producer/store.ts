// Producer view store (W21) — the producer gate's UI state: the spec
// being composed, the mastering proposal list (view of coordinator
// records), and the A/B audition position. View-state only — no
// generation runs in the webview; records arrive via ingest.

import { createStore, StoreApi } from 'zustand/vanilla';
import {
  parseMasteringProposal,
  validateSpec,
  type AccompanimentSpec,
  type MasteringProposalDto,
} from './types';

export interface ProducerViewState {
  /** The spec the producer panel is composing — view-state draft,
   * not project truth. */
  draft: AccompanimentSpec | null;
  draftError: string | null;
  /** Coordinator-authoritative mastering proposals by id. */
  mastering: Record<string, MasteringProposalDto>;
  order: string[];
  /** A/B audition position: which side is playing. */
  auditionSide: 'a' | 'b';
  auditionOf: string | null;
  busy: boolean;
}

export interface ProducerActions {
  setDraft(spec: AccompanimentSpec): boolean;
  clearDraft(): void;
  ingestMastering(payload: unknown): number;
  upsertMastering(payload: unknown): boolean;
  removeMastering(proposalId: string): void;
  setAudition(proposalId: string | null, side?: 'a' | 'b'): void;
  setBusy(b: boolean): void;
}

export type ProducerStore = StoreApi<
  ProducerViewState & { actions: ProducerActions }
>;

export function createProducerStore(): ProducerStore {
  return createStore<ProducerViewState & { actions: ProducerActions }>()(
    (set, get) => ({
      draft: null,
      draftError: null,
      mastering: {},
      order: [],
      auditionSide: 'a',
      auditionOf: null,
      busy: false,
      actions: {
        setDraft(spec) {
          const err = validateSpec(spec);
          if (err) {
            set({ draftError: err });
            return false;
          }
          set({ draft: spec, draftError: null });
          return true;
        },
        clearDraft() {
          set({ draft: null, draftError: null });
        },
        ingestMastering(payload) {
          let n = 0;
          if (Array.isArray(payload)) {
            for (const raw of payload) {
              if (get().actions.upsertMastering(raw)) n++;
            }
          }
          return n;
        },
        upsertMastering(payload) {
          const rec = parseMasteringProposal(payload);
          if (!rec) return false;
          set((s) => {
            const mastering = { ...s.mastering, [rec.proposalId]: rec };
            const order = s.order.includes(rec.proposalId)
              ? s.order
              : [...s.order, rec.proposalId];
            return { mastering, order };
          });
          return true;
        },
        removeMastering(proposalId) {
          set((s) => {
            const mastering = { ...s.mastering };
            delete mastering[proposalId];
            return {
              mastering,
              order: s.order.filter((id) => id !== proposalId),
            };
          });
        },
        setAudition(proposalId, side) {
          set({
            auditionOf: proposalId,
            auditionSide: side ?? 'a',
          });
        },
        setBusy(b) {
          set({ busy: b });
        },
      },
    }),
  );
}
