// W15 — generation view store: request/result records folded from job
// telemetry, T62 A-B compare + accept-alternate semantics. View-state
// only; records never invent runner facts.

import { describe, expect, it } from 'vitest';
import { createGenerationStore, type GenerationArtifact } from './index';

const art = (sha: string): GenerationArtifact => ({
  name: 'generated.wav',
  sha256: sha,
  assetRel: `assets/sha256/${sha}.wav`,
  bytes: 256044,
});

const shaA = 'a'.repeat(64);
const shaB = 'b'.repeat(64);

describe('generation store', () => {
  it('creates a queued record from requestJob and keeps order', () => {
    const s = createGenerationStore();
    s.getState().actions.requestJob({
      jobId: 'j1',
      modelId: 'void.audio.generate-musicgen-small',
      prompt: 'warm pad',
    });
    const rec = s.getState().records.j1;
    expect(rec.status).toBe('queued');
    expect(rec.modelId).toBe('void.audio.generate-musicgen-small');
    expect(rec.prompt).toBe('warm pad');
    expect(s.getState().order).toEqual(['j1']);
  });

  it('folds telemetry progress and terminal status', () => {
    const s = createGenerationStore();
    s.getState().actions.requestJob({ jobId: 'j1', modelId: null, prompt: null });
    expect(
      s.getState().actions.applyEvent({ job_id: 'j1', status: 'running', percent: 40 }),
    ).toBe(true);
    expect(s.getState().records.j1.percent).toBe(40);
    s.getState().actions.setResult('j1', [art(shaA)]);
    const rec = s.getState().records.j1;
    expect(rec.status).toBe('succeeded');
    expect(rec.artifacts[0].sha256).toBe(shaA);
  });

  it('ignores events for unknown jobs — never fabricates records', () => {
    const s = createGenerationStore();
    expect(
      s.getState().actions.applyEvent({ job_id: 'ghost', status: 'running' }),
    ).toBe(false);
    expect(s.getState().records.ghost).toBeUndefined();
  });

  it('markCancelling shows cancelled intent; terminal records are untouched', () => {
    const s = createGenerationStore();
    s.getState().actions.requestJob({ jobId: 'j1', modelId: null, prompt: null });
    s.getState().actions.applyEvent({ job_id: 'j1', status: 'running' });
    s.getState().actions.markCancelling('j1');
    expect(s.getState().records.j1.status).toBe('cancelled');
    s.getState().actions.setResult('j1', [art(shaA)]);
    expect(s.getState().records.j1.status).toBe('succeeded');
    s.getState().actions.markCancelling('j1');
    expect(s.getState().records.j1.status).toBe('succeeded');
  });

  it('T62: A-B compare accepts alternates while originals stay immutable', () => {
    const s = createGenerationStore();
    s.getState().actions.requestJob({ jobId: 'j1', modelId: null, prompt: 'pad' });
    s.getState().actions.setResult('j1', [art(shaA), art(shaB)]);
    // compare only real candidates
    s.getState().actions.setCompare('j1', 'c'.repeat(64));
    expect(s.getState().records.j1.comparingSha256).toBeNull();
    s.getState().actions.setCompare('j1', shaB);
    expect(s.getState().records.j1.comparingSha256).toBe(shaB);
    // accept the alternate — both artifacts stay in the record
    s.getState().actions.acceptResult('j1', shaB);
    const rec = s.getState().records.j1;
    expect(rec.acceptedSha256).toBe(shaB);
    expect(rec.comparingSha256).toBeNull();
    expect(rec.artifacts.map((a) => a.sha256)).toEqual([shaA, shaB]);
    // cannot accept an artifact that isn't part of the verified result
    s.getState().actions.acceptResult('j1', 'd'.repeat(64));
    expect(s.getState().records.j1.acceptedSha256).toBe(shaB);
  });

  it('records retry lineage via retryOf', () => {
    const s = createGenerationStore();
    s.getState().actions.requestJob({ jobId: 'j1', modelId: null, prompt: 'drone' });
    s.getState().actions.markCancelling('j1');
    s.getState().actions.requestJob({ jobId: 'j2', modelId: null, prompt: 'drone', retryOf: 'j1' });
    expect(s.getState().records.j2.retryOf).toBe('j1');
    expect(s.getState().records.j1.status).toBe('cancelled');
  });

  it('reset clears all records', () => {
    const s = createGenerationStore();
    s.getState().actions.requestJob({ jobId: 'j1', modelId: null, prompt: null });
    s.getState().actions.reset();
    expect(s.getState().records).toEqual({});
    expect(s.getState().order).toEqual([]);
  });
});
