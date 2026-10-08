// Proposals view store (W13) — ranked suggestion list + selection +
// ghost preview, all view-state derivations of coordinator-authoritative
// proposal records. The store never applies edits: accept/reject are
// intents routed through the bridge onto the normal command path.

import { createStore, StoreApi } from 'zustand/vanilla';
import { ghostNotesOf, type GhostNote, type TickRangeLike } from './ghost';
import {
  parseProposalRecord,
  type ProposalRecord,
  type ProposalStatus,
  type StaleCause,
} from './types';

export type { GhostNote } from './ghost';
export type { ProposalRecord, ProposalStatus } from './types';

export interface ProposalSelection {
  proposalId: string;
  candidateRank: number;
  /** Partial-accept note subset (candidate indices). Empty = all. */
  indices: number[];
}

export interface ProposalsViewState {
  records: Record<string, ProposalRecord>;
  order: string[];
  /** Selection for the ranked list (keyboard navigates this). */
  selection: ProposalSelection | null;
  /** Ghost preview for the selected candidate — transient, view-only. */
  ghostNotes: GhostNote[];
  /** True while an accept/reject bridge call is in flight. */
  busy: boolean;
}

export interface ProposalsActions {
  ingestList(payload: unknown): number;
  upsert(payload: unknown): boolean;
  remove(proposalId: string): void;
  /** Move keyboard selection through ranked candidates. */
  moveSelection(delta: number): void;
  /** Toggle a note index in/out of the partial-accept subset. */
  toggleIndex(index: number): void;
  /** Mark live proposals stale — never silently (T55). */
  markStale(cause: StaleCause, proposalId?: string): number;
  setBusy(busy: boolean): void;
  select(proposalId: string, candidateRank?: number): void;
  clear(): void;
  reset(): void;
}

export type ProposalsStore = StoreApi<
  ProposalsViewState & { actions: ProposalsActions }
>;

const LIVE: ProposalStatus[] = ['pending', 'ready'];

function ghostFor(
  records: Record<string, ProposalRecord>,
  sel: ProposalSelection | null,
): GhostNote[] {
  if (!sel) return [];
  const rec = records[sel.proposalId];
  if (!rec || rec.status !== 'ready') return [];
  const cand = rec.candidates.find((c) => c.rank === sel.candidateRank);
  const locked = rec.context?.lockedRanges ?? [];
  return ghostNotesOf(cand, locked);
}

function firstRank(rec: ProposalRecord): number {
  return rec.candidates[0]?.rank ?? 1;
}

export function createProposalsStore(): ProposalsStore {
  return createStore<ProposalsViewState & { actions: ProposalsActions }>()(
    (set, get) => ({
      records: {},
      order: [],
      selection: null,
      ghostNotes: [],
      busy: false,
      actions: {
        ingestList(payload) {
          const rows = Array.isArray(payload)
            ? payload
            : ((payload as Record<string, unknown>)?.proposals ??
              (payload as Record<string, unknown>)?.rows ??
              []);
          if (!Array.isArray(rows)) return 0;
          const records: Record<string, ProposalRecord> = {};
          const order: string[] = [];
          for (const row of rows) {
            const rec = parseProposalRecord(row);
            if (rec) {
              records[rec.proposalId] = rec;
              order.push(rec.proposalId);
            }
          }
          let sel = get().selection;
          if (!sel) {
            const first = order
              .map((id) => records[id])
              .find((r) => r.status === 'ready');
            if (first)
              sel = {
                proposalId: first.proposalId,
                candidateRank: firstRank(first),
                indices: [],
              };
          }
          set({
            records,
            order,
            selection: sel,
            ghostNotes: ghostFor(records, sel),
          });
          return order.length;
        },
        upsert(payload) {
          const rec = parseProposalRecord(payload);
          if (!rec) return false;
          const { records, order, selection } = get();
          const next = { ...records, [rec.proposalId]: rec };
          const nextOrder = order.includes(rec.proposalId)
            ? order
            : [...order, rec.proposalId];
          // Auto-select newest ready proposal when nothing selected.
          const sel =
            selection ??
            (rec.status === 'ready'
              ? {
                  proposalId: rec.proposalId,
                  candidateRank: firstRank(rec),
                  indices: [],
                }
              : null);
          set({
            records: next,
            order: nextOrder,
            selection: sel,
            ghostNotes: ghostFor(next, sel),
          });
          return true;
        },
        remove(proposalId) {
          const { records, order, selection } = get();
          if (!records[proposalId]) return;
          const next = { ...records };
          delete next[proposalId];
          const sel =
            selection?.proposalId === proposalId ? null : selection;
          set({
            records: next,
            order: order.filter((id) => id !== proposalId),
            selection: sel,
            ghostNotes: ghostFor(next, sel),
          });
        },
        select(proposalId, candidateRank) {
          const { records } = get();
          const rec = records[proposalId];
          if (!rec || rec.status !== 'ready') return;
          const sel: ProposalSelection = {
            proposalId,
            candidateRank: candidateRank ?? firstRank(rec),
            indices: [],
          };
          set({ selection: sel, ghostNotes: ghostFor(records, sel) });
        },
        moveSelection(delta) {
          const { records, selection } = get();
          if (!selection) return;
          const rec = records[selection.proposalId];
          if (!rec || rec.status !== 'ready') return;
          const ranks = rec.candidates.map((c) => c.rank).sort((a, b) => a - b);
          const at = ranks.indexOf(selection.candidateRank);
          const next =
            ranks[(at + delta + ranks.length) % ranks.length] ??
            selection.candidateRank;
          const sel = { ...selection, candidateRank: next, indices: [] };
          set({ selection: sel, ghostNotes: ghostFor(records, sel) });
        },
        toggleIndex(index) {
          const { records, selection } = get();
          if (!selection) return;
          const rec = records[selection.proposalId];
          const cand = rec?.candidates.find(
            (c) => c.rank === selection.candidateRank,
          );
          if (!cand || index < 0 || index >= cand.notes.length) return;
          const has = selection.indices.includes(index);
          const indices = has
            ? selection.indices.filter((i) => i !== index)
            : [...selection.indices, index].sort((a, b) => a - b);
          set({ selection: { ...selection, indices } });
        },
        markStale(cause, proposalId) {
          const { records, order, selection } = get();
          const next = { ...records };
          let n = 0;
          for (const id of order) {
            const rec = next[id];
            if (!rec) continue;
            if (proposalId && id !== proposalId) continue;
            if (!LIVE.includes(rec.status)) continue;
            next[id] = { ...rec, status: 'stale', staleCause: cause };
            n++;
          }
          const sel =
            selection && next[selection.proposalId]?.status !== 'ready'
              ? null
              : selection;
          set({
            records: next,
            selection: sel,
            ghostNotes: ghostFor(next, sel),
          });
          return n;
        },
        setBusy(busy) {
          set({ busy });
        },
        clear() {
          set({ selection: null, ghostNotes: [] });
        },
        reset() {
          set({
            records: {},
            order: [],
            selection: null,
            ghostNotes: [],
            busy: false,
          });
        },
      },
    }),
  );
}
