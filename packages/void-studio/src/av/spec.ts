// AV export spec DTO — the TS twin of crates/void-av `AvExportSpec`
// (W24, T87–T89). Field names/shapes match serde JSON exactly:
// camelCase keys, 64-bit fields as DECIMAL STRINGS, internally-tagged
// input unions ({kind:"checkpoint_file"|"asset_blob"|"lavfi_test"}).
//
// PROTOCOL GAP (integrator note): as with ExportSpecDto, there is no
// AV-export PersistentCommand in void_control.fbs; the integrator's
// shell surfaces this payload to the void-av runner. This module builds
// and validates exactly what the runner re-validates.

import { u64str } from 'void-client';

// ---------------------------------------------------------------------------
// Rational frame-rate math (bigint, exact — mirrors void_av::rate)
// ---------------------------------------------------------------------------

/** Exact rational frame rate, stored un-normalized like the Rust side. */
export interface AvFrameRate {
  num: bigint;
  den: bigint;
}

export const NTSC_30: AvFrameRate = { num: 30000n, den: 1001n };
export const NTSC_24: AvFrameRate = { num: 24000n, den: 1001n };

/** Rounding applied at boundaries — declared, never implicit. */
export type AvFrameRounding = 'nearest_ties_away' | 'floor' | 'ceil';

function roundRatio(num: bigint, den: bigint, rounding: AvFrameRounding): bigint {
  const q = num / den;
  const r = num % den;
  switch (rounding) {
    case 'floor':
      return q;
    case 'ceil':
      return r === 0n ? q : q + 1n;
    case 'nearest_ties_away':
      return r * 2n >= den ? q + 1n : q;
  }
}

/** Samples covered by `frames` frames at `rate`/`sampleRate`. */
export function samplesForFrames(
  rate: AvFrameRate,
  frames: bigint,
  sampleRate: number,
  rounding: AvFrameRounding,
): bigint {
  return roundRatio(frames * BigInt(sampleRate) * rate.den, rate.num, rounding);
}

/** Exact start sample of frame `frame` — rational floor. */
export function frameStartSample(rate: AvFrameRate, frame: bigint, sampleRate: number): bigint {
  return (frame * BigInt(sampleRate) * rate.den) / rate.num;
}

/**
 * Frame containing `sample`. Frame f owns [start(f), start(f+1)) where
 * start is a rational floor — the inverse is
 * `floor(((sample+1)*num - 1) / (sr*den))`, NOT `floor(sample*num/(sr*den))`
 * (which under-counts at frame-start samples whose ideal position is
 * fractional: NTSC-30 frame 1 starts at sample 1601 even though
 * 1601*30000/(48000*1001) < 1).
 */
export function frameIndexAtSample(rate: AvFrameRate, sample: bigint, sampleRate: number): bigint {
  return ((sample + 1n) * rate.num - 1n) / (BigInt(sampleRate) * rate.den);
}

/** Frames needed to cover `samples` samples — rational ceil. */
export function framesCoveringSamples(
  rate: AvFrameRate,
  samples: bigint,
  sampleRate: number,
): bigint {
  const num = samples * rate.num;
  const den = BigInt(sampleRate) * rate.den;
  return (num + den - 1n) / den;
}

/** Cue placement evidence — mirrors `AvFramePlan::PlannedCue`. */
export interface PlannedCueDto {
  atSamples: string;
  frameIndex: string;
  frameStartSample: string;
  offsetSamples: string;
  withinOneFrame: boolean;
}

/** Place a cue at `sample` — within_one_frame iff offset ≤ one frame of samples. */
export function placeCue(
  rate: AvFrameRate,
  sample: bigint,
  sampleRate: number,
): PlannedCueDto {
  const frameIndex = frameIndexAtSample(rate, sample, sampleRate);
  const frameStart = frameStartSample(rate, frameIndex, sampleRate);
  const offset = sample - frameStart;
  const oneFrame = samplesForFrames(rate, 1n, sampleRate, 'ceil');
  return {
    atSamples: u64str(sample),
    frameIndex: u64str(frameIndex),
    frameStartSample: u64str(frameStart),
    offsetSamples: u64str(offset),
    withinOneFrame: offset <= oneFrame,
  };
}

// ---------------------------------------------------------------------------
// Spec DTO (serde twin of AvExportSpec)
// ---------------------------------------------------------------------------

export type AvInputRole = 'video' | 'audio';

export type AvInputDto =
  | { kind: 'checkpoint_file'; rel_path: string; sha256: string; role: AvInputRole }
  | { kind: 'asset_blob'; sha256: string; role: AvInputRole }
  | { kind: 'lavfi_test'; src: string; role: AvInputRole };

export type AvTailPolicy =
  | { mode: 'none' }
  | { mode: 'samples'; samples: string }
  | { mode: 'frames'; frames: string };

export interface AvCueDto {
  name: string;
  atSamples: string;
}

/** serde shape of crates/void-av::AvExportSpec — do not rename fields. */
export interface AvExportSpecDto {
  jobId: string;
  projectId: string;
  checkpointId: string;
  sourceRevision: string;
  sampleRate: number;
  channels: 'mono' | 'stereo';
  frameRateNum: string;
  frameRateDen: string;
  rounding: AvFrameRounding;
  rangeSamples: string;
  width: number;
  height: number;
  codecId: string;
  inputs: AvInputDto[];
  cues: AvCueDto[];
  tail: AvTailPolicy;
  timeoutMs: string;
  maxOutputBytes: string;
  deadlineMonotonicNs?: string;
  outputName: string;
}

export class AvSpecError extends Error {
  constructor(
    message: string,
    readonly field: string,
  ) {
    super(message);
    this.name = 'AvSpecError';
  }
}

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const SHA_RE = /^[0-9a-f]{64}$/;
const NAME_RE = /^[a-zA-Z0-9][a-zA-Z0-9._-]*$/;
// Conservative lavfi graph allowlist — argv-passed, but suspicious
// characters (spaces, ';', '`', '$') are still rejected so nothing
// reaches the subprocess looking like shell.
const LAVFI_RE = /^[a-zA-Z0-9:=_,.\-'\[\]]+$/;

/** Mirrors spec::MAX_* bounds in crates/void-av. */
export const AV_BOUNDS = {
  maxVideoFrames: 7_200_000n,
  maxSamples: 4_147_200_000n,
  maxTimeoutMs: 1_800_000n,
  maxOutputBytes: 68_719_476_736n,
  maxDimension: 16384,
} as const;

function parseU64(field: string, v: string): bigint {
  if (!/^[0-9]+$/.test(v)) throw new AvSpecError(`${field} must be a decimal string`, field);
  return BigInt(v);
}

/** Validation parity with `AvExportSpec::validate`. Throws AvSpecError. */
export function validateAvExportSpec(spec: AvExportSpecDto): void {
  if (!UUID_RE.test(spec.jobId)) throw new AvSpecError('jobId must be a uuid', 'jobId');
  if (!UUID_RE.test(spec.projectId)) throw new AvSpecError('projectId must be a uuid', 'projectId');
  if (!UUID_RE.test(spec.checkpointId))
    throw new AvSpecError('checkpointId must be a uuid', 'checkpointId');
  if (!NAME_RE.test(spec.outputName))
    throw new AvSpecError('outputName must be an identifier-safe name', 'outputName');
  if (spec.outputName.startsWith('-'))
    throw new AvSpecError('outputName must not look like a flag', 'outputName');
  if (spec.channels !== 'mono' && spec.channels !== 'stereo')
    throw new AvSpecError('channels must be mono|stereo', 'channels');
  const num = parseU64('frameRateNum', spec.frameRateNum);
  const den = parseU64('frameRateDen', spec.frameRateDen);
  if (num === 0n || den === 0n) throw new AvSpecError('frame rate must be positive', 'frameRateNum');
  if (spec.width === 0 || spec.height === 0 || spec.width > AV_BOUNDS.maxDimension || spec.height > AV_BOUNDS.maxDimension)
    throw new AvSpecError('width/height out of bounds', 'width');
  const range = parseU64('rangeSamples', spec.rangeSamples);
  if (range > AV_BOUNDS.maxSamples) throw new AvSpecError('rangeSamples too large', 'rangeSamples');
  const timeout = parseU64('timeoutMs', spec.timeoutMs);
  if (timeout === 0n || timeout > AV_BOUNDS.maxTimeoutMs)
    throw new AvSpecError('timeoutMs out of bounds', 'timeoutMs');
  const cap = parseU64('maxOutputBytes', spec.maxOutputBytes);
  if (cap === 0n || cap > AV_BOUNDS.maxOutputBytes)
    throw new AvSpecError('maxOutputBytes out of bounds', 'maxOutputBytes');
  if (spec.sampleRate <= 0) throw new AvSpecError('sampleRate must be positive', 'sampleRate');

  const hasV = spec.inputs.some((i) => i.role === 'video');
  const hasA = spec.inputs.some((i) => i.role === 'audio');
  if (!hasV || !hasA) throw new AvSpecError('spec needs at least one video and one audio input', 'inputs');
  for (const input of spec.inputs) {
    if (input.kind === 'checkpoint_file') {
      if (input.rel_path.includes('..') || input.rel_path.startsWith('/'))
        throw new AvSpecError('checkpoint rel_path must stay inside the checkpoint dir', 'inputs');
      if (!SHA_RE.test(input.sha256)) throw new AvSpecError('input sha256 must be 64 lowercase hex', 'inputs');
    } else if (input.kind === 'asset_blob') {
      if (!SHA_RE.test(input.sha256)) throw new AvSpecError('asset sha256 must be 64 lowercase hex', 'inputs');
    } else if (!LAVFI_RE.test(input.src)) {
      throw new AvSpecError('lavfi source contains disallowed characters', 'inputs');
    }
  }
  for (const cue of spec.cues) {
    parseU64('cues[].atSamples', cue.atSamples);
  }
  if (spec.tail.mode === 'samples') parseU64('tail.samples', spec.tail.samples);
  if (spec.tail.mode === 'frames') parseU64('tail.frames', spec.tail.frames);
}

/**
 * Audio-total duration → `S.UUUUUU`, mirroring
 * `AvExportSpec::duration_seconds` (audio program only — the encoder
 * uses it as the input-scoped `-t` on the audio source; video is
 * bounded by `-frames:v`).
 */
export function avDurationSeconds(
  audioSamples: bigint,
  sampleRate: number,
  rounding: AvFrameRounding = 'nearest_ties_away',
): string {
  const us = roundRatio(audioSamples * 1_000_000n, BigInt(sampleRate), rounding);
  return `${us / 1_000_000n}.${(us % 1_000_000n).toString().padStart(6, '0')}`;
}

/** Frame-plan totals — mirrors `AvExportSpec::frame_plan`. */
export interface AvFramePlanDto {
  rangeSamples: string;
  audioTotalSamples: string;
  videoTotalFrames: string;
  frameRateNum: string;
  frameRateDen: string;
  cues: PlannedCueDto[];
}

export function avFramePlan(spec: AvExportSpecDto): AvFramePlanDto {
  validateAvExportSpec(spec);
  const rate: AvFrameRate = { num: BigInt(spec.frameRateNum), den: BigInt(spec.frameRateDen) };
  const range = BigInt(spec.rangeSamples);
  // Rust semantics: a `Samples` tail extends the audio program (and
  // video coverage grows to cover it); a `Frames` tail extends VIDEO
  // ONLY — audio stays at `range` (tail frames are visual hold).
  const tailSamples = spec.tail.mode === 'samples' ? BigInt(spec.tail.samples) : 0n;
  const tailFrames = spec.tail.mode === 'frames' ? BigInt(spec.tail.frames) : 0n;
  const audioTotal = range + tailSamples;
  if (audioTotal > AV_BOUNDS.maxSamples) throw new AvSpecError('audio total too large', 'tail');
  const videoRange = framesCoveringSamples(rate, range, spec.sampleRate);
  const tailFromSamples = tailSamples > 0n
    ? framesCoveringSamples(rate, audioTotal, spec.sampleRate) - videoRange
    : 0n;
  const videoFrames = videoRange + (tailFrames > tailFromSamples ? tailFrames : tailFromSamples);
  if (videoFrames > AV_BOUNDS.maxVideoFrames) throw new AvSpecError('video total too large', 'tail');
  for (const cue of spec.cues) {
    if (BigInt(cue.atSamples) >= audioTotal)
      throw new AvSpecError(`cue at sample ${cue.atSamples} outside rendered program`, 'cues');
  }
  return {
    rangeSamples: u64str(range),
    audioTotalSamples: u64str(audioTotal),
    videoTotalFrames: u64str(videoFrames),
    frameRateNum: u64str(rate.num),
    frameRateDen: u64str(rate.den),
    cues: spec.cues.map((c) => ({ ...placeCue(rate, BigInt(c.atSamples), spec.sampleRate), atSamples: u64str(BigInt(c.atSamples)) })),
  };
}
