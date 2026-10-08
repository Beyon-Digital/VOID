import { describe, expect, it } from 'vitest';
import {
  acceptMasteringOp,
  auditionOp,
  createProducerStore,
  parseMasteringProposal,
  requestAccompanimentOp,
  validateSpec,
  type AccompanimentSpec,
  type MasteringProposalDto,
} from './index';

function spec(): AccompanimentSpec {
  return {
    seed: '42',
    role: 'bass',
    mode: 'accompaniment',
    candidates: 4,
    outputStartTicks: '0',
    outputLengthTicks: '3840000',
    lockedRanges: [{ startTicks: '1920000', lengthTicks: '960000' }],
    keyHint: 'F#',
    scaleKind: 'major',
    chords: [
      { atTicks: '0', rootPc: 6, quality: 'major' },
      { atTicks: '1920000', rootPc: 1, quality: 'dominant7' },
    ],
    groove: {
      name: 'straight',
      stepsPerBeat: 4,
      timingTicks: new Array(16).fill(0),
      accent: new Array(16).fill(0),
    },
    densityPpm: 450_000,
    variationPpm: 300_000,
    tempoBpm: 120,
  };
}

function prop(): MasteringProposalDto {
  return {
    proposalId: 'mprop_abc123def456abc123def456abc1',
    projectId: 'p1',
    sourceSha256: 'a'.repeat(64),
    contextSha256: 'b'.repeat(64),
    status: 'ready',
    measured: {
      sampleRate: 48000,
      channels: 2,
      frames: '192000',
      integratedLufs: -16.2,
      lraLu: 8.0,
      truePeakDbtp: -2.0,
      samplePeakDbfs: -2.3,
      gatedBlocks: '40',
      blockLufsMax: -14,
      blockLufsMin: -18,
    },
    ops: [{ kind: 'gain', db: 2.2 }],
    rationale: ['integrated -16.2 vs -14 → +2.2 dB'],
    audition: {
      aSha256: 'a'.repeat(64),
      ops: [{ kind: 'gain', db: 2.2 }],
      levelMatchDb: 2.2,
      loopStartTicks: '0',
      loopLengthTicks: '960000',
    },
    supersedes: null,
    createdUtc: '2026-10-08T00:00:00Z',
    decidedUtc: null,
    error: null,
  };
}

describe('producer spec validation + ops', () => {
  it('valid spec builds an op', () => {
    expect(validateSpec(spec())).toBeNull();
    const op = requestAccompanimentOp(spec());
    expect(op.op).toBe('request_accompaniment');
    expect(op.spec.seed).toBe('42');
  });
  it('rejects invalid fields', () => {
    const s = spec();
    s.candidates = 9;
    expect(validateSpec(s)).not.toBeNull();
    const s2 = spec();
    s2.outputLengthTicks = '0';
    expect(validateSpec(s2)).not.toBeNull();
    const s3 = spec();
    s3.chords[0].rootPc = 12;
    expect(validateSpec(s3)).not.toBeNull();
    expect(() => requestAccompanimentOp(s3)).toThrow();
  });
  it('mastering op builders validate ids', () => {
    const p = prop();
    expect(acceptMasteringOp(p.proposalId).op).toBe('accept_mastering');
    expect(auditionOp(p.proposalId, 'b').side).toBe('b');
    expect(() => acceptMasteringOp('')).toThrow();
  });
});

describe('producer store', () => {
  it('draft + mastering ingest + audition state', () => {
    const s = createProducerStore();
    expect(s.getState().actions.setDraft(spec())).toBe(true);
    const bad = spec();
    bad.candidates = 0;
    expect(s.getState().actions.setDraft(bad)).toBe(false);
    expect(s.getState().draftError).not.toBeNull();

    expect(s.getState().actions.upsertMastering(prop())).toBe(true);
    expect(s.getState().mastering[prop().proposalId].status).toBe('ready');
    expect(s.getState().actions.ingestMastering([prop(), { junk: 1 }])).toBe(1);

    s.getState().actions.setAudition(prop().proposalId, 'b');
    expect(s.getState().auditionSide).toBe('b');
    s.getState().actions.removeMastering(prop().proposalId);
    expect(s.getState().order).toHaveLength(0);
  });
  it('parseMasteringProposal is defensive', () => {
    expect(parseMasteringProposal(prop())).not.toBeNull();
    expect(parseMasteringProposal(null)).toBeNull();
    const bad = prop() as unknown as Record<string, unknown>;
    bad.sourceSha256 = 'not-hex';
    expect(parseMasteringProposal(bad)).toBeNull();
    const bad2 = prop() as unknown as Record<string, unknown>;
    bad2.status = 'exploding';
    expect(parseMasteringProposal(bad2)).toBeNull();
  });
});
