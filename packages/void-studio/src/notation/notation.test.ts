// Notation view-state + op-builder tests (W25, T90/T91).
//
// Asserts: selection by stable id, op plans that serialize to the exact
// wire shape `crates/void-notation` expects (serde tag=op/params,
// camelCase, string-int64), the loss surface, and the view-state-only
// boundary (the store never holds score content).

import { describe, expect, it } from 'vitest';
import {
  AAF_INTERCHANGE_AVAILABLE,
  ENGRAVING_AVAILABLE,
  FINALCUT_INTERCHANGE_AVAILABLE,
  LOGICX_INTERCHANGE_AVAILABLE,
  MUSICXML_INTERCHANGE_AVAILABLE,
  SCORE_ANCHORS_AVAILABLE,
  SCORE_OPS_WIRE_AVAILABLE,
} from './capabilities';
import {
  assignLyricPlan,
  beamPlan,
  bindTabPlan,
  chordElement,
  createNotationViewStore,
  createSlurPlan,
  deleteSelectionPlan,
  groupTupletPlan,
  groupedLoss,
  lossNeedsAcknowledgement,
  lossRows,
  lossSummary,
  moveSelectionPlan,
  noteElement,
  nudgeSelectionPlan,
  requireTicks,
  restElement,
  setDurationPlan,
  setTempoMapPlan,
  transposePlan,
  upsertAnchorPlan,
  removeAnchorPlan,
  type LossReport,
} from './index';

// -------------------------------------------------------------------------
// Wire DTO shape — the fields serde expects.
// -------------------------------------------------------------------------

describe('op builders — exact wire shapes', () => {
  it('note insert carries string-int64 position and flattened kind', () => {
    const el = noteElement(
      'n1',
      { step: 'c', alter: 0, octave: 4 },
      { offsetTicks: '960000', durationTicks: 480000, voice: 1, staff: 1 },
      { noteType: 'eighth', dots: 0 },
    );
    expect(el.position?.offsetTicks).toBe('960000');
    expect(el.position?.durationTicks).toBe('480000');
    expect(el.kind).toBe('note');
    const plan = moveSelectionPlan([
      {
        elementId: 'n1',
        measureIndex: 2,
        offsetTicks: '240000',
        voice: 1,
        staff: 1,
      },
    ]);
    expect(plan).toEqual([
      {
        op: 'moveElement',
        params: {
          elementId: 'n1',
          measureIndex: 2,
          offsetTicks: '240000',
          voice: 1,
          staff: 1,
        },
      },
    ]);
    // Whole plan serializes as tagged ops.
    const wire = JSON.parse(JSON.stringify(plan));
    expect(wire[0].op).toBe('moveElement');
    expect(wire[0].params.elementId).toBe('n1');
    expect(typeof wire[0].params.offsetTicks).toBe('string');
  });

  it('delete/transpose/duration plans', () => {
    expect(deleteSelectionPlan(['a', 'b'])).toEqual([
      { op: 'deleteElement', params: { elementId: 'a' } },
      { op: 'deleteElement', params: { elementId: 'b' } },
    ]);
    expect(transposePlan({ scope: 'elements', ids: ['a'] }, 7)).toEqual([
      { op: 'transpose', params: { scope: 'elements', ids: ['a'], semitones: 7 } },
    ]);
    expect(transposePlan({ scope: 'part', partId: 'P1' }, -2)).toEqual([
      { op: 'transpose', params: { scope: 'part', partId: 'P1', semitones: -2 } },
    ]);
    expect(setDurationPlan([{ elementId: 'a', durationTicks: '240000' }])).toEqual([
      { op: 'setDuration', params: { elementId: 'a', durationTicks: '240000' } },
    ]);
  });

  it('lyric/tab/slur/tuplet/beam plans', () => {
    expect(assignLyricPlan('h', '1', 'begin', 'hel')).toEqual([
      {
        op: 'assignLyric',
        params: { hostId: 'h', lyricId: undefined, number: '1', syllabic: 'begin', text: 'hel' },
      },
    ]);
    expect(bindTabPlan('h', 0, 6, 2)).toEqual([
      { op: 'bindTab', params: { hostId: 'h', tabId: undefined, member: 0, string: 6, fret: 2 } },
    ]);
    expect(createSlurPlan('a', 'c', 1)).toEqual([
      { op: 'createSlur', params: { slurId: undefined, startElementId: 'a', endElementId: 'c', number: 1 } },
    ]);
    expect(groupTupletPlan(['a', 'b', 'c'], 3, 2, 'eighth')).toEqual([
      { op: 'groupTuplet', params: { tupletId: undefined, memberIds: ['a', 'b', 'c'], actual: 3, normal: 2, normalType: 'eighth' } },
    ]);
    expect(
      beamPlan(1, [
        { element: 'a', role: 'begin' },
        { element: 'b', role: 'end' },
      ]),
    ).toEqual([
      { op: 'beam', params: { beamId: undefined, number: 1, members: [{ element: 'a', role: 'begin' }, { element: 'b', role: 'end' }] } },
    ]);
  });

  it('anchor + tempo-map plans (movie scoring)', () => {
    expect(setTempoMapPlan([{ atTicks: '0', bpm: 120 }])).toEqual([
      { op: 'setTempoMap', params: { points: [{ atTicks: '0', bpm: 120 }] } },
    ]);
    expect(
      upsertAnchorPlan({ id: 'hit1', label: 'door', atSeconds: '5/2' }),
    ).toEqual([
      {
        op: 'upsertAnchor',
        params: { anchor: { id: 'hit1', label: 'door', atSeconds: '5/2' } },
      },
    ]);
    expect(removeAnchorPlan('hit1')).toEqual([
      { op: 'removeAnchor', params: { anchorId: 'hit1' } },
    ]);
  });

  it('requireTicks enforces the string-int64 contract', () => {
    expect(requireTicks('960000')).toBe('960000');
    expect(requireTicks(960000)).toBe('960000');
    expect(() => requireTicks('12.5')).toThrow(RangeError);
    expect(() => requireTicks(Number.MAX_SAFE_INTEGER + 1)).toThrow(RangeError);
  });

  it('nudge pre-validates overflow instead of failing mid-plan', () => {
    const plan = nudgeSelectionPlan(
      [{ elementId: 'a', measureIndex: 0, currentOffsetTicks: '960000', voice: 1, staff: 1 }],
      '240000',
    );
    expect(plan[0]).toEqual({
      op: 'moveElement',
      params: {
        elementId: 'a',
        measureIndex: 0,
        offsetTicks: '1200000',
        voice: 1,
        staff: 1,
      },
    });
    expect(() =>
      nudgeSelectionPlan(
        [{ elementId: 'a', measureIndex: 0, currentOffsetTicks: '100', voice: 1, staff: 1 }],
        '-200',
      ),
    ).toThrow(RangeError);
  });

  it('chord needs pitches; rest carries measureRest', () => {
    expect(() =>
      chordElement('c1', [], { offsetTicks: '0', durationTicks: '1', voice: 1, staff: 1 }),
    ).toThrow(RangeError);
    const r = restElement(
      'r1',
      { offsetTicks: '0', durationTicks: '3840000', voice: 1, staff: 1 },
      { measureRest: true },
    );
    expect((r.data as { measureRest: boolean }).measureRest).toBe(true);
  });
});

// -------------------------------------------------------------------------
// View-state store — selection by stable id, staged plans, loss surface.
// -------------------------------------------------------------------------

describe('notation view store', () => {
  it('selects/toggles/clears by stable element id', () => {
    const s = createNotationViewStore();
    s.getState().actions.select('P1', 'n1a');
    expect(s.getState().actions.isSelected('P1', 'n1a')).toBe(true);
    expect(s.getState().actions.isSelected('P2', 'n1a')).toBe(false);
    s.getState().actions.addToSelection('P1', 'n1b');
    s.getState().actions.toggle('P1', 'n1a');
    expect(s.getState().actions.selectedIds()).toEqual(['n1b']);
    s.getState().actions.clearSelection();
    expect(s.getState().selection).toEqual([]);
  });

  it('stages and clears op plans without touching score content', () => {
    const s = createNotationViewStore();
    const plan = deleteSelectionPlan(['n1a', 'n1b']);
    s.getState().actions.stagePlan(plan, 'delete 2 notes');
    expect(s.getState().stagedPlan).toHaveLength(2);
    expect(s.getState().stagedSummary).toBe('delete 2 notes');
    s.getState().actions.clearPlan();
    expect(s.getState().stagedPlan).toBeNull();
    s.getState().actions.stagePlan([], 'noop');
    expect(s.getState().stagedPlan).toBeNull();
  });

  it('records the loss report and requires acknowledgement', () => {
    const s = createNotationViewStore();
    const report: LossReport = {
      direction: 'import',
      entries: [
        { element: 'part/P1/measure[1]/note', aspect: 'grace', kind: 'dropped', reason: 'x' },
        { element: 'part/P1/measure[1]/print', aspect: 'layout', kind: 'dropped', reason: 'y' },
      ],
    };
    s.getState().actions.recordLossReport(report);
    expect(s.getState().lossReport?.entries).toHaveLength(2);
    expect(s.getState().lossAcknowledged).toBe(false);
    expect(lossNeedsAcknowledgement(report)).toBe(true);
    s.getState().actions.acknowledgeLoss();
    expect(s.getState().lossAcknowledged).toBe(true);
  });

  it('loss surface groups deterministically', () => {
    const report: LossReport = {
      direction: 'import',
      entries: [
        { element: 'p/m[1]/a', aspect: 'grace', kind: 'dropped', reason: 'x' },
        { element: 'p/m[1]/b', aspect: 'grace', kind: 'dropped', reason: 'y' },
        { element: 'p/m[2]/c', aspect: 'layout', kind: 'approximated', reason: 'z' },
      ],
    };
    expect(lossRows(report)).toHaveLength(3);
    const groups = groupedLoss(report);
    expect(groups.map((g) => g.aspect)).toEqual(['grace', 'layout']);
    expect(groups[0].rows).toHaveLength(2);
    expect(lossSummary(report)).toEqual({ dropped: 2, approximated: 1 });
  });

  it('view-state only — the store holds ids/plans/flags, never score content', () => {
    const s = createNotationViewStore();
    s.getState().actions.select('P1', 'n1a');
    s.getState().actions.stagePlan(deleteSelectionPlan(['n1a']), 'x');
    const state = s.getState() as unknown as Record<string, unknown>;
    const forbidden = ['elements', 'measures', 'notes', 'pitches', 'parts', 'score'];
    for (const k of Object.keys(state)) {
      expect(forbidden).not.toContain(k);
    }
    // Only ids in selection.
    for (const sel of s.getState().selection) {
      expect(typeof sel.elementId).toBe('string');
      expect(typeof sel.partId).toBe('string');
    }
  });
});

// -------------------------------------------------------------------------
// Honest capability flags.
// -------------------------------------------------------------------------

describe('capability flags stay honest', () => {
  it('native-blocked surfaces are OFF; real lane surfaces are ON', () => {
    expect(ENGRAVING_AVAILABLE).toBe(false); // needs Verovio/native+GUI
    expect(SCORE_OPS_WIRE_AVAILABLE).toBe(false); // no protocol score op yet
    expect(AAF_INTERCHANGE_AVAILABLE).toBe(false);
    expect(FINALCUT_INTERCHANGE_AVAILABLE).toBe(false);
    expect(LOGICX_INTERCHANGE_AVAILABLE).toBe(false);
    expect(MUSICXML_INTERCHANGE_AVAILABLE).toBe(true);
    expect(SCORE_ANCHORS_AVAILABLE).toBe(true);
    const allFalse = [
      ENGRAVING_AVAILABLE,
      SCORE_OPS_WIRE_AVAILABLE,
      AAF_INTERCHANGE_AVAILABLE,
      FINALCUT_INTERCHANGE_AVAILABLE,
      LOGICX_INTERCHANGE_AVAILABLE,
    ];
    expect(allFalse.every((f) => f === false)).toBe(true);
  });
});
