// T42/T43 — export spec DTO exact shapes, validation parity with
// crates/void-export, job envelope, progress events, result cards.

import { describe, expect, it } from 'vitest';
import {
  buildExportSpec,
  sanitizeOutputName,
  validateExportSpec,
  ExportSpecError,
  type ExportContext,
} from './spec';
import {
  exportJobEnvelope,
  applyJobEvent,
  initialProgress,
  parseJobEvent,
  isTerminal,
} from './job';
import { exportResultsFromItems, exportListRequest, EXPORT_LIST_VIEW } from './results';
import { createExportDialogStore, activeJobs, finishedJobs } from './dialog';

const ctx: ExportContext = {
  jobId: '00000000-0000-4000-8000-0000000000e1',
  projectId: '00000000-0000-4000-8000-0000000000aa',
  checkpointId: '00000000-0000-4000-8000-0000000000c1',
  sourceRevision: '7',
  assetHashes: ['a'.repeat(64)],
  tempoMap: [{ atTicks: '0', bpm: 120 }],
};

const wavSel = {
  outputName: 'mix-v1',
  format: 'wav' as const,
  rangeStartTicks: '0',
  rangeEndTicks: '3840000',
  channels: 'stereo' as const,
  bitDepth: 'pcm24' as const,
  sampleRate: 48000,
  tail: { mode: 'none' } as const,
};

describe('ExportSpecDto wire shape', () => {
  it('builds exact camelCase DTO with decimal-string int64s', () => {
    const s = buildExportSpec(wavSel, ctx);
    expect(s).toEqual({
      jobId: ctx.jobId,
      projectId: ctx.projectId,
      checkpointId: ctx.checkpointId,
      sourceRevision: '7',
      assetHashes: ['a'.repeat(64)],
      rangeStartTicks: '0',
      rangeEndTicks: '3840000',
      format: 'wav',
      channels: 'stereo',
      bitDepth: 'pcm24',
      sampleRate: 48000,
      tail: { mode: 'none' },
      tempoMap: [{ atTicks: '0', bpm: 120 }],
      outputName: 'mix-v1',
    });
    // int64s are strings — never JS numbers.
    expect(typeof s.rangeEndTicks).toBe('string');
    expect(typeof s.sourceRevision).toBe('string');
  });

  it('midi drops wav-only fields entirely (serde Option::None = absent)', () => {
    const s = buildExportSpec({ ...wavSel, format: 'midi' }, ctx);
    expect(s.format).toBe('midi');
    expect('channels' in s).toBe(false);
    expect('bitDepth' in s).toBe(false);
    expect('sampleRate' in s).toBe(false);
  });

  it('tail variants serialize as the internally-tagged union', () => {
    const s1 = buildExportSpec({ ...wavSel, tail: { mode: 'milliseconds', ms: 250 } }, ctx);
    expect(s1.tail).toEqual({ mode: 'milliseconds', ms: 250 });
    const s2 = buildExportSpec({ ...wavSel, tail: { mode: 'ticks', ticks: '960000' } }, ctx);
    expect(s2.tail).toEqual({ mode: 'ticks', ticks: '960000' });
  });

  it('rejections mirror the Rust validator', () => {
    expect(() => buildExportSpec({ ...wavSel, rangeStartTicks: '9', rangeEndTicks: '1' }, ctx)).toThrow(ExportSpecError);
    expect(() => buildExportSpec({ ...wavSel, sampleRate: 22050 }, ctx)).toThrow(/approved/);
    expect(() => buildExportSpec({ ...wavSel, outputName: '../x' }, ctx)).toThrow();
    expect(() => buildExportSpec({ ...wavSel, tail: { mode: 'ticks', ticks: '-4' } }, ctx)).toThrow();
    expect(() => buildExportSpec({ ...wavSel, tail: { mode: 'milliseconds', ms: 99999 } }, ctx)).toThrow();
    expect(() => validateExportSpec({ ...buildExportSpec(wavSel, ctx), format: 'midi' })).toThrow(/must not carry/);
  });

  it('sanitizeOutputName matches the Rust scoped-filename rule', () => {
    expect(sanitizeOutputName(' take-02.final ')).toBe('take-02.final');
    expect(() => sanitizeOutputName('.hidden')).toThrow();
    expect(() => sanitizeOutputName('a/b')).toThrow();
    expect(() => sanitizeOutputName('x'.repeat(121))).toThrow();
    expect(() => sanitizeOutputName('')).toThrow();
  });
});

describe('job envelope', () => {
  it('mirrors job_spec_for: av_export kind, spec echo in parameters', () => {
    const spec = buildExportSpec(wavSel, ctx);
    const env = exportJobEnvelope(spec, 'b'.repeat(64), 'c'.repeat(64));
    expect(env.kind).toBe('av_export');
    expect(env.runtimeId).toBe('void-export');
    expect(env.parameters).toEqual(spec);
    expect(env.inputs).toEqual(spec.assetHashes);
    expect(env.outputScopeToken).toBe(`export:${spec.jobId}`);
    expect(env.reservations).toEqual({ ramBytes: '536870912', vramBytes: '0', cpuThreads: 1 });
    expect(env.deadlineMonotonicNs).toBe('0');
  });
});

describe('job progress', () => {
  it('folds job events; never fabricates percent', () => {
    let p = initialProgress('j1');
    expect(p.status).toBe('queued');
    expect(p.percent).toBeNull();
    p = applyJobEvent(p, { kind: 'JobEvent', project_id: 'p', job_id: 'j1', status: 'running' });
    expect(p.status).toBe('running');
    expect(p.percent).toBeNull(); // no percent reported — stays indeterminate
    p = applyJobEvent(p, { kind: 'JobEvent', project_id: 'p', job_id: 'j1', status: 'running', percent: 42 });
    expect(p.percent).toBe(42);
    p = applyJobEvent(p, { kind: 'JobEvent', project_id: 'p', job_id: 'j1', status: 'succeeded', percent: 100 });
    expect(isTerminal(p.status)).toBe(true);
  });

  it('ignores events for other jobs and unknown statuses', () => {
    const p = initialProgress('j1');
    expect(applyJobEvent(p, { kind: 'JobEvent', project_id: 'p', job_id: 'other', status: 'failed' })).toBe(p);
    expect(applyJobEvent(p, { kind: 'JobEvent', project_id: 'p', job_id: 'j1', status: 'bogus' })).toBe(p);
  });

  it('quarantine flag is sticky and surfaced', () => {
    let p = initialProgress('j1');
    p = applyJobEvent(p, { kind: 'JobEvent', project_id: 'p', job_id: 'j1', status: 'cancelled', quarantined: true });
    expect(p.quarantined).toBe(true);
    p = applyJobEvent(p, { kind: 'JobEvent', project_id: 'p', job_id: 'j1', status: 'cancelled' });
    expect(p.quarantined).toBe(true);
  });

  it('parseJobEvent narrows by shape only', () => {
    expect(parseJobEvent({ kind: 'JobEvent', job_id: 'j', status: 'running' })).not.toBeNull();
    expect(parseJobEvent({ kind: 'MeterFrame' })).toBeNull();
    expect(parseJobEvent(null)).toBeNull();
    expect(parseJobEvent({ kind: 'JobEvent' })).toBeNull();
  });
});

describe('result list', () => {
  it('parses ExportResult cards; drops malformed items', () => {
    const items = [
      { object_id: 'o1', summary_json: JSON.stringify({ kind: 'ExportResult', jobId: 'j1', status: 'succeeded', file: 'mix.wav', sha256: 'f'.repeat(64), bytes: '1234', frames: '96000', warnings: ['x'] }) },
      { object_id: 'o2', summary_json: JSON.stringify({ kind: 'ExportResult', jobId: 'j2', status: 'failed', error: 'renderer exploded' }) },
      { object_id: 'o3', summary_json: JSON.stringify({ kind: 'Other' }) },
      { object_id: 'o4', summary_json: '{nope' },
    ];
    const rs = exportResultsFromItems(items);
    expect(rs.length).toBe(2);
    expect(rs[0].file).toBe('mix.wav');
    expect(rs[0].bytes).toBe('1234');
    expect(rs[1].status).toBe('failed');
    expect(rs[1].error).toMatch(/exploded/);
  });

  it('EXPORT_LIST request carries the gap-marked view name', () => {
    const r = exportListRequest('p1', 'req1');
    expect(r.view).toBe(EXPORT_LIST_VIEW);
    expect(r.project_id).toBe('p1');
    expect(r.start_ticks).toBe('-1');
  });
});

describe('dialog store', () => {
  it('submit returns a validated spec and registers a queued job', () => {
    const store = createExportDialogStore();
    const spec = store.getState().actions.submit(ctx);
    expect(spec).not.toBeNull();
    expect(spec!.format).toBe('wav');
    const jobs = store.getState().jobs;
    expect(jobs[ctx.jobId].status).toBe('queued');
    expect(activeJobs(store.getState()).length).toBe(1);
  });

  it('submit surfaces validation errors without touching jobs', () => {
    const store = createExportDialogStore({ outputName: 'bad/name' });
    const spec = store.getState().actions.submit(ctx);
    expect(spec).toBeNull();
    expect(store.getState().errors.length).toBe(1);
    expect(Object.keys(store.getState().jobs).length).toBe(0);
  });

  it('telemetry updates live progress; results merge cards', () => {
    const store = createExportDialogStore();
    store.getState().actions.submit(ctx);
    store.getState().actions.applyTelemetry({ kind: 'JobEvent', project_id: 'p', job_id: ctx.jobId, status: 'running', percent: 33 });
    expect(store.getState().jobs[ctx.jobId].status).toBe('running');
    expect(store.getState().jobs[ctx.jobId].percent).toBe(33);
    store.getState().actions.applyTelemetry({ kind: 'JobEvent', project_id: 'p', job_id: ctx.jobId, status: 'succeeded' });
    expect(finishedJobs(store.getState()).length).toBe(1);
    // non-job telemetry ignored
    store.getState().actions.applyTelemetry({ kind: 'MeterFrame', track_id: 't' });
    store.getState().actions.applyResults([
      { object_id: 'o', summary_json: JSON.stringify({ kind: 'ExportResult', jobId: ctx.jobId, status: 'succeeded', file: 'mix.wav' }) },
    ]);
    expect(store.getState().results[0].file).toBe('mix.wav');
  });
});
