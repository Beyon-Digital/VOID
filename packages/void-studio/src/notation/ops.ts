// Edit-gesture → ScoreOp plan builders (W25, T90).
//
// Every builder returns a `ScoreOpDto[]` — a single-transaction op plan
// the coordinator applies atomically against `crates/void-notation`.
// Builders validate shape only (ids present, ticks parse); model truth
// (host exists, beam capacity, transpose spelling) is the crate's job —
// it rejects and rolls back, never guesses.

import type {
  AnchorDto,
  BeamMembershipDto,
  ElementDto,
  ElementId,
  NoteDataDto,
  NoteTypeDto,
  PartId,
  PitchDto,
  PlacementDto,
  ScoreOpDto,
  SyllabicDto,
  TempoPointDto,
  TieDto,
} from './types';

/** String-int64 helper — rejects anything that isn't an integer literal. */
export function requireTicks(value: string | number | bigint, what = 'ticks'): string {
  if (typeof value === 'bigint' || typeof value === 'number') {
    if (!Number.isSafeInteger(value)) {
      throw new RangeError(`${what} out of safe-integer range: ${value}`);
    }
    return String(value);
  }
  if (!/^-?\d+$/.test(value.trim())) {
    throw new RangeError(`${what} must be an integer string, got ${JSON.stringify(value)}`);
  }
  return value.trim();
}

function insertOp(partId: PartId, measureIndex: number, element: ElementDto): ScoreOpDto {
  return { op: 'insertElement', params: { partId, measureIndex, element } };
}

// ---------------------------------------------------------------------------
// Element constructors — the modelled subset of kinds.
// ---------------------------------------------------------------------------

export function noteElement(
  id: ElementId,
  pitch: PitchDto,
  position: { offsetTicks: string | number | bigint; durationTicks: string | number | bigint; voice: number; staff: number },
  extra?: { noteType?: NoteTypeDto; dots?: number; tie?: TieDto; accidental?: string },
): ElementDto<'note', NoteDataDto> {
  return {
    id,
    position: {
      offsetTicks: requireTicks(position.offsetTicks, 'offsetTicks'),
      durationTicks: requireTicks(position.durationTicks, 'durationTicks'),
      voice: position.voice,
      staff: position.staff,
    },
    kind: 'note',
    data: {
      pitch,
      noteType: extra?.noteType,
      dots: extra?.dots ?? 0,
      tie: extra?.tie ?? { start: false, stop: false },
      accidental: extra?.accidental,
    },
  };
}

export function restElement(
  id: ElementId,
  position: { offsetTicks: string | number | bigint; durationTicks: string | number | bigint; voice: number; staff: number },
  extra?: { noteType?: NoteTypeDto; dots?: number; measureRest?: boolean },
): ElementDto<'rest'> {
  return {
    id,
    position: {
      offsetTicks: requireTicks(position.offsetTicks, 'offsetTicks'),
      durationTicks: requireTicks(position.durationTicks, 'durationTicks'),
      voice: position.voice,
      staff: position.staff,
    },
    kind: 'rest',
    data: {
      noteType: extra?.noteType,
      dots: extra?.dots ?? 0,
      measureRest: extra?.measureRest ?? false,
    },
  };
}

export function chordElement(
  id: ElementId,
  pitches: PitchDto[],
  position: { offsetTicks: string | number | bigint; durationTicks: string | number | bigint; voice: number; staff: number },
  extra?: { noteType?: NoteTypeDto; dots?: number; tie?: TieDto },
): ElementDto<'chord'> {
  if (pitches.length === 0) throw new RangeError('chord needs ≥1 pitch');
  return {
    id,
    position: {
      offsetTicks: requireTicks(position.offsetTicks, 'offsetTicks'),
      durationTicks: requireTicks(position.durationTicks, 'durationTicks'),
      voice: position.voice,
      staff: position.staff,
    },
    kind: 'chord',
    data: {
      pitches,
      noteType: extra?.noteType,
      dots: extra?.dots ?? 0,
      tie: extra?.tie ?? { start: false, stop: false },
    },
  };
}

// ---------------------------------------------------------------------------
// Gesture builders — each returns a complete transaction plan.
// ---------------------------------------------------------------------------

export function insertNotePlan(
  partId: PartId,
  measureIndex: number,
  note: ElementDto,
): ScoreOpDto[] {
  return [insertOp(partId, measureIndex, note)];
}

/** Delete every selected id — cascades (lyrics/tabs/slurs/beam shrink)
 *  are the crate's job, the plan just names the targets. */
export function deleteSelectionPlan(elementIds: readonly ElementId[]): ScoreOpDto[] {
  return elementIds.map((elementId) => ({
    op: 'deleteElement',
    params: { elementId },
  }));
}

/** Move elements to an explicit measure/offset (drag result). One op
 *  per element — still one transaction. */
export function moveSelectionPlan(
  moves: readonly {
    elementId: ElementId;
    measureIndex: number;
    offsetTicks: string | number | bigint;
    voice: number;
    staff: number;
  }[],
): ScoreOpDto[] {
  return moves.map((m) => ({
    op: 'moveElement',
    params: {
      elementId: m.elementId,
      measureIndex: m.measureIndex,
      offsetTicks: requireTicks(m.offsetTicks, 'offsetTicks'),
      voice: m.voice,
      staff: m.staff,
    },
  }));
}

/** Nudge: shift a set of timed elements by `deltaTicks` — the view knows
 *  each element's current offset from its selection snapshot, so a
 *  nudge is a move to offset+delta (rejecting negatives at build time
 *  keeps the plan honest rather than failing mid-transaction). */
export function nudgeSelectionPlan(
  items: readonly {
    elementId: ElementId;
    measureIndex: number;
    currentOffsetTicks: string | number | bigint;
    durationTicks?: string | number | bigint;
    voice: number;
    staff: number;
  }[],
  deltaTicks: string | number | bigint,
): ScoreOpDto[] {
  const delta = BigInt(requireTicks(deltaTicks, 'deltaTicks'));
  return items.map((it) => {
    const target = BigInt(requireTicks(it.currentOffsetTicks)) + delta;
    if (target < 0n) {
      throw new RangeError(
        `nudge would push ${it.elementId} before the measure start`,
      );
    }
    return {
      op: 'moveElement',
      params: {
        elementId: it.elementId,
        measureIndex: it.measureIndex,
        offsetTicks: target.toString(),
        voice: it.voice,
        staff: it.staff,
      },
    };
  });
}

export function transposePlan(
  scope:
    | { scope: 'all' }
    | { scope: 'part'; partId: PartId }
    | { scope: 'elements'; ids: readonly ElementId[] },
  semitones: number,
): ScoreOpDto[] {
  if (!Number.isInteger(semitones)) throw new RangeError('semitones must be integer');
  const params =
    scope.scope === 'elements'
      ? { scope: 'elements' as const, ids: [...scope.ids], semitones }
      : scope.scope === 'part'
        ? { scope: 'part' as const, partId: scope.partId, semitones }
        : { scope: 'all' as const, semitones };
  return [{ op: 'transpose', params }];
}

export function setDurationPlan(
  edits: readonly { elementId: ElementId; durationTicks: string | number | bigint }[],
): ScoreOpDto[] {
  return edits.map((e) => ({
    op: 'setDuration',
    params: { elementId: e.elementId, durationTicks: requireTicks(e.durationTicks, 'durationTicks') },
  }));
}

/** Attach a lyric line to a note/chord (upsert on host+number). */
export function assignLyricPlan(
  hostId: ElementId,
  number: string,
  syllabic: SyllabicDto,
  text: string,
  lyricId?: ElementId,
): ScoreOpDto[] {
  return [{
    op: 'assignLyric',
    params: { hostId, lyricId, number, syllabic, text },
  }];
}

/** Bind string/fret to a note or chord member (upsert on host+member). */
export function bindTabPlan(
  hostId: ElementId,
  member: number,
  string: number,
  fret: number,
  tabId?: ElementId,
): ScoreOpDto[] {
  return [{
    op: 'bindTab',
    params: { hostId, tabId, member, string, fret },
  }];
}

export function attachArticulationPlan(
  hostId: ElementId,
  kind: string,
  placement?: PlacementDto,
  articulationId?: ElementId,
): ScoreOpDto[] {
  return [{
    op: 'attachArticulation',
    params: { hostId, articulationId, kind: { kind }, placement },
  }];
}

export function createSlurPlan(
  startElementId: ElementId,
  endElementId: ElementId,
  number = 1,
  slurId?: ElementId,
): ScoreOpDto[] {
  return [{
    op: 'createSlur',
    params: { slurId, startElementId, endElementId, number },
  }];
}

export function groupTupletPlan(
  memberIds: readonly ElementId[],
  actual: number,
  normal: number,
  normalType?: NoteTypeDto,
  tupletId?: ElementId,
): ScoreOpDto[] {
  return [{
    op: 'groupTuplet',
    params: { tupletId, memberIds: [...memberIds], actual, normal, normalType },
  }];
}

export function beamPlan(
  number: number,
  members: readonly BeamMembershipDto[],
  beamId?: ElementId,
): ScoreOpDto[] {
  return [{ op: 'beam', params: { beamId, number, members: [...members] } }];
}

export function setTempoMapPlan(points: readonly TempoPointDto[]): ScoreOpDto[] {
  return [{ op: 'setTempoMap', params: { points: [...points] } }];
}

export function upsertAnchorPlan(anchor: AnchorDto): ScoreOpDto[] {
  return [{ op: 'upsertAnchor', params: { anchor } }];
}

export function removeAnchorPlan(anchorId: string): ScoreOpDto[] {
  return [{ op: 'removeAnchor', params: { anchorId } }];
}
