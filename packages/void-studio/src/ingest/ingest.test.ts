import { describe, expect, it } from 'vitest';
import { createProposalsStore } from '../proposals/store';
import { createJobsStore } from '../jobs/store';
import { createGenerationStore } from '../generation/store';
import {
  ingestControlEvent,
  ingestJobList,
  ingestTelemetryEvent,
} from './ingest';
import {
  latestContextSha,
  rescanStaleness,
  targetClipExists,
} from './stale';
import {
  acceptProposalCandidate,
  dismissProposal,
  undoAcceptedTransaction,
} from './accept';
import { FakeTransport, VoidClient } from 'void-client';
import type { CommandReceipt } from 'void-client';

function proposal(over: Record<string, unknown> = {}) {
  return {
    proposalId: 'p1',
    projectId: 'proj',
    sourceRevision: '5',
    contextSha256: 'sha-a',
    status: 'ready',
    context: { clipId: 'clip-1', lockedRanges: [] },
    candidates: [
      {
        rank: 1,
        score: 0.7,
        rationale: 'stepwise motion',
        notes: [
          { pitch: 64, velocity: 96, onsetTicks: '960000', lengthTicks: '240000' },
          { pitch: 67, velocity: 96, onsetTicks: '1200000', lengthTicks: '240000' },
        ],
      },
    ],
    provenance: {
      jobId: 'j1',
      generatorId: 'gen',
      generatorVersion: '1',
      modelId: 'm1',
      runtimeId: 'r1',
      runtimeSha256: 'rs',
      seed: 's',
      documentSha256: 'ds',
      analysis: null,
      documentAsset: 'assets/x',
    },
    createdAt: '2026-01-01T00:00:00Z',
    updatedAt: '2026-01-01T00:00:00Z',
    ...over,
  };
}

describe('ingestControlEvent (proposal routing)', () => {
  it('upserts a record from a proposal event', () => {
    const s = createProposalsStore();
    expect(ingestControlEvent({ kind: 'proposal', record: proposal() }, s)).toBe(true);
    expect(s.getState().records.p1?.status).toBe('ready');
  });

  it('ingests a proposal_list payload', () => {
    const s = createProposalsStore();
    ingestControlEvent(
      { kind: 'proposal_list', proposals: [proposal(), proposal({ proposalId: 'p2' })] },
      s,
    );
    expect(s.getState().order).toEqual(['p1', 'p2']);
  });

  it('removes on proposal_removed', () => {
    const s = createProposalsStore();
    ingestControlEvent({ kind: 'proposal', record: proposal() }, s);
    ingestControlEvent({ kind: 'proposal_removed', proposalId: 'p1' }, s);
    expect(s.getState().records.p1).toBeUndefined();
  });

  it('drops malformed payloads instead of guessing', () => {
    const s = createProposalsStore();
    expect(ingestControlEvent({ kind: 'proposal', record: { junk: 1 } }, s)).toBe(false);
    expect(ingestControlEvent({ kind: 'totally_other' }, s)).toBe(false);
    expect(s.getState().order).toHaveLength(0);
  });
});

describe('ingestTelemetryEvent (jobs/generation routing)', () => {
  it('folds a JobEvent into the jobs store', () => {
    const jobs = createJobsStore();
    const gen = createGenerationStore();
    gen.getState().actions.requestJob({ jobId: 'j9', modelId: 'm1' });
    const ok = ingestTelemetryEvent(
      { kind: 'JobEvent', job_id: 'j9', status: 'running', percent: 40 },
      jobs,
      gen,
    );
    expect(ok).toBe(true);
    // First event creates the stub from identity facts only; the next
    // event carries the live percent (jobs store contract).
    ingestTelemetryEvent(
      { kind: 'JobEvent', job_id: 'j9', status: 'running', percent: 40 },
      jobs,
      gen,
    );
    expect(jobs.getState().cards.j9?.percent).toBe(40);
    expect(gen.getState().records.j9?.status).toBe('running');
  });

  it('ignores non-job telemetry', () => {
    const jobs = createJobsStore();
    const gen = createGenerationStore();
    expect(
      ingestTelemetryEvent({ kind: 'TransportAck' }, jobs, gen),
    ).toBe(false);
  });

  it('ingests job-list pages defensively', () => {
    const jobs = createJobsStore();
    const n = ingestJobList(
      [
        {
          jobId: 'j1',
          projectId: 'p',
          kind: 'audio_generation',
          status: 'queued',
          createdAt: 'x',
          updatedAt: 'x',
        },
        { broken: true },
      ],
      jobs,
    );
    expect(n).toBe(1);
    expect(jobs.getState().order).toEqual(['j1']);
  });
});

describe('staleness (UI-T15)', () => {
  it('marks live records stale when the doc revision drifts', () => {
    const s = createProposalsStore();
    ingestControlEvent({ kind: 'proposal', record: proposal() }, s);
    ingestControlEvent(
      { kind: 'proposal', record: proposal({ proposalId: 'p2', sourceRevision: '9' }) },
      s,
    );
    expect(rescanStaleness(s, '7')).toBe(1);
    expect(s.getState().records.p1?.status).toBe('stale');
    expect(s.getState().records.p1?.staleCause).toBe('context_changed');
    expect(s.getState().records.p2?.status).toBe('ready');
  });

  it('leaves terminal records alone', () => {
    const s = createProposalsStore();
    ingestControlEvent(
      { kind: 'proposal', record: proposal({ status: 'accepted' }) },
      s,
    );
    expect(rescanStaleness(s, '99')).toBe(0);
    expect(s.getState().records.p1?.status).toBe('accepted');
  });

  it('detects superseded context by sha', () => {
    const s = createProposalsStore();
    ingestControlEvent({ kind: 'proposal', record: proposal() }, s);
    ingestControlEvent(
      {
        kind: 'proposal',
        record: proposal({
          proposalId: 'p2',
          contextSha256: 'sha-b',
          createdAt: '2026-01-02T00:00:00Z',
        }),
      },
      s,
    );
    expect(latestContextSha(s.getState().records, 'clip-1')).toBe('sha-b');
    expect(latestContextSha(s.getState().records, 'other')).toBeNull();
  });

  it('checks the target against parsed CLIP_LIST rows', () => {
    expect(targetClipExists([{ clipId: 'clip-1' }], 'clip-1')).toBe(true);
    expect(targetClipExists([{ clipId: 'clip-1' }], 'clip-2')).toBe(false);
  });
});

function client(t: FakeTransport): VoidClient {
  let n = 0;
  return new VoidClient({ transport: t, ids: () => `id-${++n}` });
}

function respond(t: FakeTransport): void {
  t.respond('send_command', (args) => {
    const dto = (args as { dto: { transaction_id: string } }).dto;
    const receipt: CommandReceipt = {
      kind: 'CommandReceipt',
      command_id: 'c',
      transaction_id: dto.transaction_id,
      status: 'APPLIED',
      error: 'NONE',
      revision: '6',
    };
    return receipt;
  });
}

describe('accept lifecycle', () => {
  it('accepts through real InsertNoteOps and one undo reverts them', async () => {
    const t = new FakeTransport();
    const s = createProposalsStore();
    ingestControlEvent({ kind: 'proposal', record: proposal() }, s);
    s.getState().actions.select('p1', 1);
    const ops: unknown[] = [];
    t.respond('send_command', (args) => {
      const dto = (args as { dto: { op: unknown; transaction_id: string } }).dto;
      ops.push(dto.op);
      const receipt: CommandReceipt = {
        kind: 'CommandReceipt',
        command_id: 'c',
        transaction_id: dto.transaction_id,
        status: 'APPLIED',
        error: 'NONE',
        revision: '6',
      };
      return receipt;
    });
    const out = await acceptProposalCandidate(client(t), s, 'p1', {
      projectId: 'proj',
      targetClipExists: true,
    });
    expect(out.ok).toBe(true);
    expect(out.noteIds).toHaveLength(2);
    expect(ops).toHaveLength(2);
    expect((ops[0] as { InsertNoteOp: { clip_id: string } }).InsertNoteOp.clip_id).toBe('clip-1');
    expect(s.getState().records.p1?.status).toBe('accepted');
    expect(s.getState().records.p1?.accepted?.transactionId).toBe(out.transactionId);

    const undone = await undoAcceptedTransaction(client(t), s, 'p1');
    expect(undone.ok).toBe(true);
    expect(s.getState().records.p1?.status).toBe('ready');
  });

  it('refuses a stale proposal — no ops sent', async () => {
    const t = new FakeTransport();
    respond(t);
    const s = createProposalsStore();
    ingestControlEvent(
      { kind: 'proposal', record: proposal({ status: 'stale' }) },
      s,
    );
    const out = await acceptProposalCandidate(client(t), s, 'p1', {
      projectId: 'proj',
      targetClipExists: true,
    });
    expect(out.ok).toBe(false);
    expect(out.reason).toMatch(/previewable|status/i);
  });

  it('refuses when the target clip is gone and marks it', async () => {
    const t = new FakeTransport();
    respond(t);
    const s = createProposalsStore();
    ingestControlEvent({ kind: 'proposal', record: proposal() }, s);
    s.getState().actions.select('p1', 1);
    const out = await acceptProposalCandidate(client(t), s, 'p1', {
      projectId: 'proj',
      targetClipExists: false,
    });
    expect(out.ok).toBe(false);
    expect(s.getState().records.p1?.status).toBe('stale');
    expect(s.getState().records.p1?.staleCause).toBe('target_gone');
  });

  it('partial accept commits exactly the selected indices', async () => {
    const t = new FakeTransport();
    const s = createProposalsStore();
    ingestControlEvent({ kind: 'proposal', record: proposal() }, s);
    s.getState().actions.select('p1', 1);
    s.getState().actions.toggleIndex(1);
    const sent: { InsertNoteOp: { start_ticks: string } }[] = [];
    t.respond('send_command', (args) => {
      const dto = (args as {
        dto: { op: { InsertNoteOp: { start_ticks: string } }; transaction_id: string };
      }).dto;
      sent.push(dto.op);
      const receipt: CommandReceipt = {
        kind: 'CommandReceipt',
        command_id: 'c',
        transaction_id: dto.transaction_id,
        status: 'APPLIED',
        error: 'NONE',
        revision: '6',
      };
      return receipt;
    });
    const out = await acceptProposalCandidate(client(t), s, 'p1', {
      projectId: 'proj',
      targetClipExists: true,
    });
    expect(out.ok).toBe(true);
    expect(sent).toHaveLength(1);
    expect(sent[0].InsertNoteOp.start_ticks).toBe('1200000');
  });

  it('dismiss removes ghosts but leaves committed notes untouched', () => {
    const s = createProposalsStore();
    ingestControlEvent({ kind: 'proposal', record: proposal() }, s);
    s.getState().actions.select('p1', 1);
    expect(s.getState().ghostNotes.length).toBeGreaterThan(0);
    dismissProposal(s, 'p1');
    expect(s.getState().records.p1).toBeUndefined();
    expect(s.getState().ghostNotes).toHaveLength(0);
  });
});
