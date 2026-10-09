// Score/notation wire DTOs (W25, T90/T91) — mirror `crates/void-notation`
// serde shapes. String-int64 rule: every tick field is a STRING on the
// wire (`offsetTicks`, `durationTicks`, `atTicks`).
//
// The crate is the model authority: these types describe what crosses
// the wire and what the view stages, never a second model.

import type { LossReport } from '../exchange';

export type ElementId = string;
export type PartId = string;

/** Steps A–G (serde is lowercase single letters). */
export type StepLetter = 'a' | 'b' | 'c' | 'd' | 'e' | 'f' | 'g';

export interface PitchDto {
  step: StepLetter;
  /** Chromatic alteration −2..=+2 (0 = natural/key signature). */
  alter: number;
  octave: number;
}

export interface PositionDto {
  offsetTicks: string;
  durationTicks: string;
  voice: number;
  staff: number;
}

export type NoteTypeDto =
  | 'longa' | 'breve' | 'whole' | 'half' | 'quarter'
  | 'eighth' | '16th' | '32nd' | '64th' | '128th' | '256th'
  | '512th' | '1024th';

export type SyllabicDto = 'single' | 'begin' | 'middle' | 'end';

export type BeamRoleDto =
  | 'begin' | 'continue' | 'end' | 'forwardHook' | 'backwardHook';

export interface BeamMembershipDto {
  element: ElementId;
  role: BeamRoleDto;
}

export type PlacementDto = 'above' | 'below';

export interface TieDto {
  start: boolean;
  stop: boolean;
}

/**
 * Element — `#[serde(flatten)]` merges `{kind, data}` into the element
 * object itself. `data` is the per-kind payload; builders in ops.ts
 * construct the modelled subset.
 */
export interface ElementDto<K extends string = string, D = unknown> {
  id: ElementId;
  position?: PositionDto;
  kind: K;
  data?: D;
}

export interface NoteDataDto {
  pitch: PitchDto;
  noteType?: NoteTypeDto;
  dots: number;
  tie: TieDto;
  accidental?: string;
}

export interface RestDataDto {
  noteType?: NoteTypeDto;
  dots: number;
  measureRest: boolean;
}

export interface ChordDataDto {
  pitches: PitchDto[];
  noteType?: NoteTypeDto;
  dots: number;
  tie: TieDto;
}

export interface TupletDataDto {
  actual: number;
  normal: number;
  normalType?: NoteTypeDto;
  members: ElementId[];
}

export interface BeamDataDto {
  number: number;
  members: BeamMembershipDto[];
}

export interface SlurDataDto {
  start: ElementId;
  end: ElementId;
  number: number;
}

/** `ArticulationKind` serde: `{"kind":"staccato"}` or
 * `{"kind":"other","value":"…"}` — kebab-case kind names. */
export interface ArticulationKindDto {
  kind: string;
  value?: string;
}

export interface ArticulationDataDto {
  host: ElementId;
  kind: ArticulationKindDto;
  placement?: PlacementDto;
}

export interface LyricDataDto {
  host: ElementId;
  number: string;
  syllabic: SyllabicDto;
  text: string;
}

export interface TabDataDto {
  host: ElementId;
  member: number;
  string: number;
  fret: number;
}

// ---------------------------------------------------------------------------
// ScoreOp — `#[serde(tag = "op", content = "params")]` + camelCase.
// ---------------------------------------------------------------------------

export type TransposeScopeDto =
  | { scope: 'all' }
  | { scope: 'part'; partId: PartId }
  | { scope: 'elements'; ids: ElementId[] };

export interface TempoPointDto {
  atTicks: string;
  bpm: number;
}

export interface AnchorDto {
  id: string;
  label: string;
  kind?: { kind: string; name?: string } | null;
  /** Exact rational string — "2" or "5/2". */
  atSeconds: string;
  timecodeHint?: string;
}

export type DropFrameDto =
  | { kind: 'nonDrop' }
  | { kind: 'drop'; perMinute: number };

export interface TimecodeModeDto {
  num: number;
  den: number;
  drop: DropFrameDto;
  startFrames: number;
}

export type ScoreOpDto =
  | { op: 'insertElement'; params: { partId: PartId; measureIndex: number; element: ElementDto } }
  | { op: 'deleteElement'; params: { elementId: ElementId } }
  | { op: 'moveElement'; params: { elementId: ElementId; measureIndex: number; offsetTicks: string; voice: number; staff: number } }
  | { op: 'transpose'; params: TransposeScopeDto & { semitones: number } }
  | { op: 'setDuration'; params: { elementId: ElementId; durationTicks: string } }
  | { op: 'setPitch'; params: { elementId: ElementId; pitch: PitchDto } }
  | { op: 'assignLyric'; params: { hostId: ElementId; lyricId?: ElementId; number: string; syllabic: SyllabicDto; text: string } }
  | { op: 'bindTab'; params: { hostId: ElementId; tabId?: ElementId; member: number; string: number; fret: number } }
  | { op: 'attachArticulation'; params: { hostId: ElementId; articulationId?: ElementId; kind: ArticulationKindDto; placement?: PlacementDto } }
  | { op: 'createSlur'; params: { slurId?: ElementId; startElementId: ElementId; endElementId: ElementId; number: number } }
  | { op: 'groupTuplet'; params: { tupletId?: ElementId; memberIds: ElementId[]; actual: number; normal: number; normalType?: NoteTypeDto } }
  | { op: 'beam'; params: { beamId?: ElementId; number: number; members: BeamMembershipDto[] } }
  | { op: 'setTempoMap'; params: { points: TempoPointDto[] } }
  | { op: 'upsertAnchor'; params: { anchor: AnchorDto } }
  | { op: 'removeAnchor'; params: { anchorId: string } };

/** Result envelope the coordinator reports after applying a plan:
 *  which op index failed (transaction rolled back), or the undo token
 *  id (opaque string — the token itself is a serde blob). */
export interface OpPlanResult {
  ok: boolean;
  error?: string;
  /** Serialized UndoToken blob — opaque to the view; kept for undo UI. */
  undoToken?: string;
}

export type { LossReport };
