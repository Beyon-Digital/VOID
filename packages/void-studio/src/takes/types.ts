// Take + comp domain types (W17/REC-04, REC-05; T66).
//
// OWNERSHIP: a TakeRecord is the studio-side descriptor of an
// engine-produced recording (W08 journals takes under
// `recordings/<takeId>/`). Records are IMMUTABLE — comping edits the
// arrangement's clip list through ordinary ops, never a take or its
// source asset. There is no TAKE_LIST view in protocol major.1, so
// folders arrive from the caller (recording coordinator/app state)
// and are kept here as view data, not a document clone.

/** A fade shape at a comp seam or clip edge. Stored in specs/view
 * state — the wire has no per-clip fade fields (NEEDS.md rev2), so a
 * FadeSpec is honest metadata the engine lane later consumes. */
export type FadeShape = 'equal-power' | 'equal-gain' | 'linear';

export interface FadeSpec {
  shape: FadeShape;
  /** Fade length in ticks; must be > 0 and bounded by the shorter
   * adjacent segment half-length (see seamFadePlan). */
  lengthTicks: string;
}

export type TakeKind = 'AUDIO' | 'MIDI';

/**
 * One recorded take. `regionStartTicks..+regionLengthTicks` is the
 * timeline span the take covers (a loop take covers the loop region);
 * `offsetTicks` is the position inside the source asset where the
 * recorded material begins.
 */
export interface TakeRecord {
  takeId: string;
  /** Folder this take belongs to (one folder per record pass stack). */
  folderId: string;
  trackId: string;
  kind: TakeKind;
  /** Lane order inside the folder (0 = first pass). */
  laneIndex: number;
  /** Source asset the take references — never mutated by comping. */
  assetId?: string;
  /** SHA-256 of the source when the journal knows it (relink honesty). */
  sourceHash?: string;
  regionStartTicks: string;
  regionLengthTicks: string;
  offsetTicks: string;
  /** false for a recovered incomplete take (interrupted recording). */
  complete: boolean;
  /** ISO-8601 record time when known — display/sort only. */
  recordedAt?: string;
}

/** A take folder: the ordered stack of takes recorded over one
 * region on one track (REC-04 take folders). */
export interface TakeFolder {
  folderId: string;
  trackId: string;
  takes: TakeRecord[];
}

/** One segment of a comp: which take plays over `[startTicks,
 * startTicks+lengthTicks)` of the comp region. */
export interface CompSegment {
  segmentId: string;
  takeId: string;
  startTicks: string;
  lengthTicks: string;
  /** Optional explicit fade-in at this segment's start seam;
   * `seamFadePlan` derives the honest default when absent. */
  fadeIn?: FadeSpec;
}

/**
 * A comp spec: an ordered, contiguous, non-overlapping cover of the
 * comp region picking one take per segment. This is the *spec*; the
 * applied comp is ordinary clips produced by compToOps under one
 * transaction (reversible via normal undo — REC-05 flatten is
 * never destructive to takes).
 */
export interface CompSpec {
  compId: string;
  trackId: string;
  regionStartTicks: string;
  regionLengthTicks: string;
  segments: CompSegment[];
}

/** Which existing timeline clip(s) the comp replaces inside its
 * region — supplied by the caller from CLIP_LIST projections. */
export interface CompTarget {
  clipIds: string[];
}
