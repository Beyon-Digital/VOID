// Producer gate wire types (W21; T78–T80) — TS twins of
// crates/void-producer DTOs. camelCase, tick fields decimal strings,
// ppm densities — same contract as the crate's serde.

import { i64str, u64str } from 'void-client';

export type ProducerRole = 'drums' | 'bass' | 'keys' | 'synth';
export type GenMode = 'accompaniment' | 'inpaint' | 'continue' | 'vary';

export interface AccompanimentSpec {
  /** u64 seed — decimal string on the wire. */
  seed: string;
  role: ProducerRole;
  mode: GenMode;
  /** 1..8 (CONTRACTS §6 cap). */
  candidates: number;
  /** Half-open output window (decimal-string ticks). */
  outputStartTicks: string;
  outputLengthTicks: string;
  /** Locked (protected) ranges the generation may not touch. */
  lockedRanges: { startTicks: string; lengthTicks: string }[];
  /** Key hint as written in the region (e.g. "F#", "dm"). */
  keyHint: string | null;
  scaleKind: 'major' | 'natural_minor' | 'dorian' | 'minor_pentatonic' | 'major_pentatonic';
  /** Piecewise chord map: each chord holds until the next event. */
  chords: { atTicks: string; rootPc: number; quality: string }[];
  groove: {
    name: string;
    stepsPerBeat: number;
    timingTicks: number[];
    accent: number[];
  };
  densityPpm: number;
  variationPpm: number;
  tempoBpm: number;
}

export function validateSpec(s: AccompanimentSpec): string | null {
  if (s.candidates < 1 || s.candidates > 8) return 'candidates out of range';
  if (s.densityPpm > 1_000_000 || s.variationPpm > 1_000_000) return 'ppm out of range';
  if (s.tempoBpm <= 0 || s.tempoBpm > 999) return 'tempo out of range';
  try {
    i64str(s.outputStartTicks);
    i64str(s.outputLengthTicks);
    u64str(s.seed);
    for (const r of s.lockedRanges) {
      i64str(r.startTicks);
      i64str(r.lengthTicks);
    }
    for (const c of s.chords) {
      i64str(c.atTicks);
      if (c.rootPc < 0 || c.rootPc > 11) return 'rootPc out of range';
    }
  } catch {
    return 'bad i64/u64 field';
  }
  const start = BigInt(i64str(s.outputStartTicks));
  const len = BigInt(i64str(s.outputLengthTicks));
  if (len <= 0n) return 'empty output range';
  if (len > 960_000n * 4n * 512n) return 'range exceeds max';
  if (start < 0n) return 'negative range start';
  return null;
}

/** Mastering op DTO — mirrors MasteringOp serde (tagged union). */
export type MasteringOpDto =
  | { kind: 'gain'; db: number }
  | { kind: 'true_peak_limiter'; ceilingDbtp: number; maxGrDb: number }
  | { kind: 'eq_band'; freqHz: number; gainDb: number; q: number; basis: string };

export interface LoudnessReportDto {
  sampleRate: number;
  channels: number;
  frames: string;
  integratedLufs: number;
  lraLu: number;
  truePeakDbtp: number;
  samplePeakDbfs: number;
  gatedBlocks: string;
  blockLufsMax: number;
  blockLufsMin: number;
}

export type MasteringStatus =
  | 'pending'
  | 'ready'
  | 'accepted'
  | 'rejected'
  | 'stale'
  | 'failed';

export interface MasteringProposalDto {
  proposalId: string;
  projectId: string;
  sourceSha256: string;
  contextSha256: string;
  status: MasteringStatus;
  measured: LoudnessReportDto;
  ops: MasteringOpDto[];
  rationale: string[];
  audition: {
    aSha256: string;
    ops: MasteringOpDto[];
    levelMatchDb: number;
    loopStartTicks: string;
    loopLengthTicks: string;
  } | null;
  supersedes: string | null;
  createdUtc: string;
  decidedUtc: string | null;
  error: string | null;
}

/** Defensive parse of a mastering proposal record off the wire. */
export function parseMasteringProposal(raw: unknown): MasteringProposalDto | null {
  const o = raw as MasteringProposalDto;
  if (
    typeof o !== 'object' ||
    o === null ||
    typeof o.proposalId !== 'string' ||
    typeof o.status !== 'string' ||
    typeof o.sourceSha256 !== 'string' ||
    !/^[\da-f]{64}$/.test(o.sourceSha256) ||
    typeof o.measured !== 'object' ||
    o.measured === null ||
    typeof o.measured.integratedLufs !== 'number' ||
    !Array.isArray(o.ops) ||
    !Array.isArray(o.rationale)
  ) {
    return null;
  }
  const statuses: readonly string[] = [
    'pending', 'ready', 'accepted', 'rejected', 'stale', 'failed',
  ];
  if (!statuses.includes(o.status)) return null;
  return o;
}
