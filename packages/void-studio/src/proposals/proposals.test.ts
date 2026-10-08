import { describe, expect, it } from 'vitest';
import { assertViewStateOnly, initialState } from '../store';
import {
  AcceptPlanError,
  applyAcceptPlan,
  createProposalsStore,
  ghostNotesOf,
  planAcceptLocal,
  type ProposalRecord,
} from './index';

function rec(over: Partial<ProposalRecord> = {}): ProposalRecord {
  return {
    proposalId: 'p-1',
    projectId: 'proj-1',
    sourceRevision: '7',
    contextSha256: 'a'.repeat(64),
    status: 'ready',
    context: {
      clipId: 'clip-1',
      lockedRanges: [{ startTicks: '0', lengthTicks: '480000' }],
    },
    candidates: [
      {
        rank: 1,
        score: 0.5,
        rationale: 'interval chain',
        notes: [
          { pitch: 60, velocity: 90, onsetTicks: '0', lengthTicks: '240000' },
          { pitch: 64, velocity: 92, onsetTicks: '480000', lengthTicks: '240000' },
          { pitch: 67, velocity: 88, onsetTicks: '960000', lengthTicks: '480000' },
        ],
      },
      {
        rank: 2,
        score: 0.3,
        rationale: 'alt chain',
        notes: [{ pitch: 62, velocity: 90, onsetTicks: '0', lengthTicks: '240000' }],
      },
    ],
    provenance: {
      jobId: 'j-1',
      generatorId: 'void-symbolic-worker',
      generatorVersion: '1.0.0',
      modelId: 'interval-markov-1',
      runtimeId: 'void-symbolic-worker',
      runtimeSha256: 'b'.repeat(64),
      seed: '42',
      documentSha256: 'c'.repeat(64),
      analysis: { keyEstimate: 'D minor' },
      documentAsset: 'assets/sha256/x.json',
    },
    createdAt: '2026-10-08T00:00:00Z',
    updatedAt: '2026-10-08T00:00:00Z',
    ...over,
  };
}

const revalOk = (r: ProposalRecord) => ({
  projectId: r.projectId,
  currentContextSha256: r.contextSha256,
  targetClipExists: true,
});

describe('proposals store (T53)', () => {
  it('ingests ranked records, auto-selects ready, derives ghosts', () => {
    const s = createProposalsStore();
    expect(s.getState().actions.ingestList([rec()])).toBe(1);
    const st = s.getState();
    expect(st.selection?.candidateRank).toBe(1);
    expect(st.ghostNotes).toHaveLength(3);
    // first note overlaps the locked range → painted locked
    expect(st.ghostNotes[0].locked).toBe(true);
    expect(st.ghostNotes[1].locked).toBe(false);
    expect(st.ghostNotes.every((g) => g.ghost)).toBe(true);
  });

  it('keyboard moves selection through ranked candidates', () => {
    const s = createProposalsStore();
    s.getState().actions.ingestList([rec()]);
    s.getState().actions.moveSelection(1);
    expect(s.getState().selection?.candidateRank).toBe(2);
    expect(s.getState().ghostNotes).toHaveLength(1);
    s.getState().actions.moveSelection(1); // wraps to 1
    expect(s.getState().selection?.candidateRank).toBe(1);
  });

  it('toggleIndex builds a partial-accept subset', () => {
    const s = createProposalsStore();
    s.getState().actions.ingestList([rec()]);
    s.getState().actions.toggleIndex(2);
    s.getState().actions.toggleIndex(0);
    expect(s.getState().selection?.indices).toEqual([0, 2]);
    s.getState().actions.toggleIndex(2);
    expect(s.getState().selection?.indices).toEqual([0]);
  });

  it('ghost payload honors the view-state rules', () => {
    // Same forbidden-name regex as src/store.ts — proposals live in
    // their own view store (like jobs/), never in StudioViewState.
    const forbidden =
      /pcm|audioBuffer|waveformData|undoStack|redoStack|noteMap|songDoc|editTree/i;
    const ghosts = ghostNotesOf(rec().candidates[0]);
    for (const g of ghosts) {
      for (const k of Object.keys(g)) {
        expect(forbidden.test(k)).toBe(false);
      }
      // transient markers only — no persistent identity, no buffers
      expect(g.ghost).toBe(true);
      expect('noteId' in g).toBe(false);
    }
    // main-store invariant still holds with proposals kept outside it
    expect(() => assertViewStateOnly(initialState())).not.toThrow();
  });
});

describe('stale marking (T55)', () => {
  it('marks live proposals stale with a cause; terminal untouched', () => {
    const s = createProposalsStore();
    s.getState().actions.ingestList([
      rec(),
      rec({ proposalId: 'p-2', status: 'accepted' }),
    ]);
    expect(s.getState().actions.markStale('context_changed')).toBe(1);
    expect(s.getState().records['p-1'].status).toBe('stale');
    expect(s.getState().records['p-1'].staleCause).toBe('context_changed');
    expect(s.getState().records['p-2'].status).toBe('accepted');
    // stale selection cleared → ghosts gone
    expect(s.getState().selection).toBeNull();
    expect(s.getState().ghostNotes).toEqual([]);
  });
});

describe('accept bridge (T54)', () => {
  it('plans inserts sharing ONE transaction id; locked dropped', () => {
    const r = rec();
    const plan = planAcceptLocal(r, 1, undefined, revalOk(r), () => 'fixed-id');
    expect(plan.transactionId).toBe('fixed-id');
    expect(plan.inserts).toHaveLength(2); // note 0 is locked → dropped
    expect(plan.droppedIndices).toEqual([0]);
    expect(plan.inserts.every((i) => i.clipId === 'clip-1')).toBe(true);
  });

  it('partial subset accept keeps only chosen indices', () => {
    const r = rec();
    const plan = planAcceptLocal(r, 1, [2], revalOk(r));
    expect(plan.inserts).toHaveLength(1);
    expect(plan.inserts[0].pitch).toBe(67);
  });

  it('rejects stale context and gone target — nothing planned (T55)', () => {
    const r = rec();
    expect(() =>
      planAcceptLocal(r, 1, undefined, {
        ...revalOk(r),
        currentContextSha256: 'f'.repeat(64),
      }),
    ).toThrowError(AcceptPlanError);
    expect(() =>
      planAcceptLocal(r, 1, undefined, {
        ...revalOk(r),
        targetClipExists: false,
      }),
    ).toThrowError(/target clip gone/);
    const stale = rec({ status: 'stale' });
    expect(() => planAcceptLocal(stale, 1, undefined, revalOk(stale))).toThrowError(
      /status stale/,
    );
  });

  it('sends InsertNoteOps on the shared transaction id', async () => {
    const r = rec();
    const plan = planAcceptLocal(r, 1, undefined, revalOk(r));
    const sent: { tx?: string; op: unknown }[] = [];
    const fake = {
      sendCommand: async (op: unknown, opts?: { transactionId?: string }) => {
        sent.push({ tx: opts?.transactionId, op });
        return { status: 'APPLIED' } as never;
      },
    };
    const { noteIds } = await applyAcceptPlan(fake as never, plan);
    expect(sent).toHaveLength(plan.inserts.length);
    expect(new Set(sent.map((s) => s.tx))).toEqual(new Set([plan.transactionId]));
    expect(noteIds).toHaveLength(plan.inserts.length);
  });
});
