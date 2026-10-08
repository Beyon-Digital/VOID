// W12 — jobs view model: job card parsing (JOB_LIST rows + JobRecord
// nests), JobEvent folding, budget badges, cancel op payload, model
// list parsing. All shapes are protocol-gap DTOs (docs/engine/NEEDS.md).

import { describe, expect, it } from 'vitest';
import {
  applyJobEventToCard,
  cancelJobOp,
  isTerminal,
  jobEventOf,
  jobListRequest,
  modelListRequest,
  parseJobCard,
  parseModelList,
  parseModelRow,
  modelRunnable,
  submitJobOp,
  createJobsStore,
  type JobCard,
} from './index';

const row = {
  jobId: '00000000-0000-4000-8000-0000000000j1'.replace('j', 'a'),
  projectId: '00000000-0000-4000-8000-0000000000aa',
  kind: 'symbolic',
  status: 'running',
  percent: 42,
  message: 'synthesizing',
  modelId: 'fake-synth',
  modelVersion: '1.0.0',
  runtimeId: 'void-fake-worker',
  reservations: { ramBytes: '67108864', vramBytes: '0', cpuThreads: 1 },
  cpuSeconds: '30',
  memoryBytes: '67108864',
  deadlineMonotonicNs: '0',
  artifacts: [
    { name: 'out.wav', sha256: 'a'.repeat(64), bytes: '4844', asset: 'assets/sha256/' + 'a'.repeat(64) + '.wav' },
  ],
  createdAt: '2026-10-08T00:00:00Z',
  updatedAt: '2026-10-08T00:00:01Z',
};

describe('parseJobCard', () => {
  it('parses a flat JOB_LIST row verbatim', () => {
    const c = parseJobCard(row)!;
    expect(c.jobId).toBe(row.jobId);
    expect(c.status).toBe('running');
    expect(c.percent).toBe(42);
    expect(c.ramBytes).toBe('67108864');
    expect(c.cpuSeconds).toBe('30');
    expect(c.artifacts[0].asset).toContain('assets/sha256/');
    expect(c.artifacts[0].bytes).toBe('4844');
  });

  it('parses a JobRecord-like {spec,status} nest', () => {
    const nested = {
      spec: {
        jobId: row.jobId,
        projectId: row.projectId,
        kind: 'analysis',
        runtimeId: 'void-jobs',
        reservations: { ramBytes: '1024', vramBytes: '0', cpuThreads: 2 },
        deadlineMonotonicNs: '999',
      },
      status: 'queued',
    };
    const c = parseJobCard(nested)!;
    expect(c.jobId).toBe(row.jobId);
    expect(c.status).toBe('queued');
    expect(c.cpuThreads).toBe(2);
    expect(c.deadlineMonotonicNs).toBe('999');
    expect(c.percent).toBeNull();
  });

  it('rejects rows without id/status or with bad status', () => {
    expect(parseJobCard({})).toBeNull();
    expect(parseJobCard({ jobId: 'x' })).toBeNull();
    expect(parseJobCard({ jobId: 'x', status: 'frobbing' })).toBeNull();
    expect(parseJobCard('running')).toBeNull();
  });

  it('never fabricates a percent', () => {
    const c = parseJobCard({ ...row, percent: undefined })!;
    expect(c.percent).toBeNull();
  });
});

describe('job events', () => {
  const card = parseJobCard(row)!;
  it('folds a snake_case JobEvent into the card', () => {
    const next = applyJobEventToCard(card, {
      kind: 'JobEvent',
      project_id: row.projectId,
      job_id: card.jobId,
      status: 'succeeded',
      percent: 100,
    });
    expect(next.status).toBe('succeeded');
    expect(next.percent).toBe(100);
    expect(isTerminal(next.status)).toBe(true);
  });

  it('ignores foreign jobs + unknown statuses', () => {
    const same = applyJobEventToCard(card, {
      kind: 'JobEvent',
      project_id: row.projectId,
      job_id: 'other',
      status: 'failed',
    });
    expect(same).toEqual(card);
    const bad = applyJobEventToCard(card, {
      kind: 'JobEvent',
      project_id: row.projectId,
      job_id: card.jobId,
      status: 'paused',
    });
    expect(bad).toEqual(card);
  });

  it('keeps quarantined once set', () => {
    const c1 = applyJobEventToCard(card, {
      kind: 'JobEvent',
      project_id: row.projectId,
      job_id: card.jobId,
      status: 'cancelled',
      quarantined: true,
    });
    expect(c1.quarantined).toBe(true);
  });

  it('jobEventOf narrows telemetry payloads', () => {
    expect(jobEventOf({ kind: 'JobEvent', job_id: 'j', status: 'running' })).not.toBeNull();
    expect(jobEventOf({ kind: 'ViewportEvent' })).toBeNull();
    expect(jobEventOf(42)).toBeNull();
  });
});

describe('ops + view requests', () => {
  it('builds the CancelJob op payload', () => {
    expect(cancelJobOp('j1')).toEqual({ op: 'cancel_job', jobId: 'j1' });
  });

  it('builds the SubmitJob op envelope', () => {
    const spec = {
      jobId: 'j', projectId: 'p', sourceRevision: '1',
      contextSha256: 'a'.repeat(64), kind: 'analysis' as const,
      runtimeId: 'r', runtimeSha256: 'b'.repeat(64), inputs: [],
      parameters: {}, reservations: { ramBytes: '1', vramBytes: '0', cpuThreads: 1 },
      deadlineMonotonicNs: '0', outputScopeToken: 's',
    };
    expect(submitJobOp(spec)).toEqual({ op: 'submit_job', spec });
  });

  it('view requests carry the documented kinds', () => {
    expect(jobListRequest('p1')).toEqual({ view: 'job_list', projectId: 'p1', includeTerminal: false });
    expect(modelListRequest()).toEqual({ view: 'model_list' });
  });
});

describe('jobs store', () => {
  it('ingests a list, folds events, marks cancelling, resolves artifacts', () => {
    const store = createJobsStore();
    expect(store.getState().actions.ingestList({ jobs: [row] })).toBe(1);
    const id = row.jobId;
    store.getState().actions.applyEvent({
      kind: 'JobEvent', project_id: row.projectId, job_id: id,
      status: 'running', percent: 55, message: 'halfway',
    });
    expect(store.getState().cards[id].percent).toBe(55);
    store.getState().actions.markCancelling(id);
    expect(store.getState().cards[id].status).toBe('cancelling');
    store.getState().actions.applyEvent({
      kind: 'JobEvent', project_id: row.projectId, job_id: id,
      status: 'cancelled',
    });
    expect(store.getState().cards[id].status).toBe('cancelled');
    store.getState().actions.setArtifacts(id, row.artifacts);
    expect(store.getState().cards[id].artifacts[0].name).toBe('out.wav');
  });

  it('list ingest never clobbers fresher event progress', () => {
    const store = createJobsStore();
    store.getState().actions.ingestList({ jobs: [row] });
    const id = row.jobId;
    store.getState().actions.applyEvent({
      kind: 'JobEvent', project_id: row.projectId, job_id: id,
      status: 'running', percent: 80,
    });
    store.getState().actions.ingestList({ jobs: [{ ...row, percent: undefined }] });
    expect(store.getState().cards[id].percent).toBe(80);
  });

  it('queued cancel goes straight to cancelled', () => {
    const store = createJobsStore();
    store.getState().actions.ingestList({ jobs: [{ ...row, status: 'queued' }] });
    store.getState().actions.markCancelling(row.jobId);
    expect(store.getState().cards[row.jobId].status).toBe('cancelled');
  });

  it('terminal cards never re-cancel', () => {
    const store = createJobsStore();
    store.getState().actions.ingestList({ jobs: [{ ...row, status: 'succeeded' }] });
    store.getState().actions.markCancelling(row.jobId);
    expect(store.getState().cards[row.jobId].status).toBe('succeeded');
  });
});

describe('model registry view', () => {
  const model = {
    modelId: 'fake-synth',
    version: '1.0.0',
    name: 'Fake Synth',
    kind: 'audio',
    runtime: 'argv',
    status: 'available',
    budgets: { cpuSeconds: '30', memoryBytes: '67108864', wallNs: '5000000000', cpuThreads: 1 },
    artifacts: [{ sha256: 'a'.repeat(64), required: true, present: true }],
    manifestSha256: 'c'.repeat(64),
    license: 'ISC',
    source: 'bundled',
  };

  it('parses a MODEL_LIST payload', () => {
    const ms = parseModelList({ models: [model] });
    expect(ms).toHaveLength(1);
    expect(ms[0].modelId).toBe('fake-synth');
    expect(ms[0].memoryBytes).toBe('67108864');
    expect(modelRunnable(ms[0])).toBe(true);
  });

  it('missing/degraded/rejected statuses surface honestly', () => {
    for (const s of ['missing', 'degraded', 'rejected', 'available']) {
      const m = parseModelRow({ ...model, status: s })!;
      expect(m.status).toBe(s);
    }
    expect(modelRunnable(parseModelRow({ ...model, status: 'degraded' })!)).toBe(true);
    expect(modelRunnable(parseModelRow({ ...model, status: 'missing' })!)).toBe(false);
    expect(modelRunnable(parseModelRow({ ...model, status: 'rejected' })!)).toBe(false);
  });

  it('counts missing + optional artifacts', () => {
    const m = parseModelRow({
      ...model,
      artifacts: [
        { sha256: 'a'.repeat(64), required: true, present: false },
        { sha256: 'b'.repeat(64), required: false, present: true },
      ],
    })!;
    expect(m.artifactCount).toBe(2);
    expect(m.missingArtifacts).toBe(1);
    expect(m.optionalArtifacts).toBe(1);
  });

  it('rejects rows without identity or with bad status', () => {
    expect(parseModelRow({})).toBeNull();
    expect(parseModelRow({ modelId: 'x', status: 'weird' })).toBeNull();
    expect(parseModelList({ models: [{ modelId: 'x', status: 'available' }] })).toHaveLength(1);
  });
});

// Type-level guard: JobCard fields stay decimal-string honest.
const _typecheck: JobCard['ramBytes'] = '0';
void _typecheck;
