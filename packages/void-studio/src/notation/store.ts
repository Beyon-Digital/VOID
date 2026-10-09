// Score-editor view-state store (W25, T90).
//
// VIEW-STATE ONLY: this store keeps a selection (stable element ids),
// the staged op plan, the last loss report, and UI affordances — never
// score content. Project truth lives in the coordinator
// (`crates/void-notation`); `assertViewStateOnly` in the test suite
// enforces that boundary.
//
// Stable-id rule: selection is a set of element ids, not positions —
// positions move under edits; ids don't.

import { createStore, StoreApi } from 'zustand/vanilla';
import type {
  ElementId,
  LossReport,
  PartId,
  ScoreOpDto,
  TimecodeModeDto,
} from './types';
import {
  ENGRAVING_AVAILABLE,
  SCORE_OPS_WIRE_AVAILABLE,
} from './capabilities';

/** One selection unit — an element in a part, addressed by stable id.
 *  `measureIndex`/`offsetTicks` are display caches for hit-testing,
 *  refreshed from view items — never used to address the model. */
export interface ScoreSelectionEntry {
  partId: PartId;
  elementId: ElementId;
}

export interface NotationViewState {
  /** Selected elements by stable id (ordered by selection sequence). */
  selection: ScoreSelectionEntry[];
  /** Anchor for shift-range extension (last click). */
  anchor: ScoreSelectionEntry | null;
  /** Op plan staged for the coordinator to apply (null = none). */
  stagedPlan: ScoreOpDto[] | null;
  /** Human-readable note on what the staged plan does. */
  stagedSummary: string | null;
  /** The most recent import/export loss report (visible surface — the
   *  dialog lists every entry before an import may be confirmed). */
  lossReport: LossReport | null;
  /** Whether the user acknowledged the loss entries (required to apply
   *  when a report is non-empty). */
  lossAcknowledged: boolean;
  /** Declared project timecode mode mirrored for the HUD (null = none). */
  timecode: TimecodeModeDto | null;
  /** Honest capability flags mirrored for UI gating. */
  engravingAvailable: boolean;
  scoreOpsWireAvailable: boolean;
}

export interface NotationViewActions {
  select(partId: PartId, elementId: ElementId): void;
  toggle(partId: PartId, elementId: ElementId): void;
  addToSelection(partId: PartId, elementId: ElementId): void;
  clearSelection(): void;
  isSelected(partId: PartId, elementId: ElementId): boolean;
  selectedIds(): ElementId[];
  stagePlan(plan: ScoreOpDto[], summary: string): void;
  clearPlan(): void;
  recordLossReport(report: LossReport | null): void;
  acknowledgeLoss(): void;
  setTimecode(mode: TimecodeModeDto | null): void;
  reset(): void;
}

export type NotationViewStore = StoreApi<
  NotationViewState & { actions: NotationViewActions }
>;

const initialState = (): NotationViewState => ({
  selection: [],
  anchor: null,
  stagedPlan: null,
  stagedSummary: null,
  lossReport: null,
  lossAcknowledged: false,
  timecode: null,
  engravingAvailable: ENGRAVING_AVAILABLE,
  scoreOpsWireAvailable: SCORE_OPS_WIRE_AVAILABLE,
});

export function createNotationViewStore(
  init?: Partial<NotationViewState>,
): NotationViewStore {
  return createStore<NotationViewState & { actions: NotationViewActions }>()(
    (set, get) => ({
      ...initialState(),
      ...init,
      actions: {
        select(partId, elementId) {
          set({
            selection: [{ partId, elementId }],
            anchor: { partId, elementId },
          });
        },
        toggle(partId, elementId) {
          const { selection } = get();
          const has = selection.some(
            (s) => s.partId === partId && s.elementId === elementId,
          );
          set({
            selection: has
              ? selection.filter(
                  (s) => !(s.partId === partId && s.elementId === elementId),
                )
              : [...selection, { partId, elementId }],
            anchor: { partId, elementId },
          });
        },
        addToSelection(partId, elementId) {
          const { selection } = get();
          if (selection.some((s) => s.partId === partId && s.elementId === elementId)) {
            return;
          }
          set({
            selection: [...selection, { partId, elementId }],
            anchor: { partId, elementId },
          });
        },
        clearSelection() {
          set({ selection: [], anchor: null });
        },
        isSelected(partId, elementId) {
          return get().selection.some(
            (s) => s.partId === partId && s.elementId === elementId,
          );
        },
        selectedIds() {
          return get().selection.map((s) => s.elementId);
        },
        stagePlan(plan, summary) {
          if (plan.length === 0) {
            set({ stagedPlan: null, stagedSummary: null });
            return;
          }
          set({ stagedPlan: [...plan], stagedSummary: summary });
        },
        clearPlan() {
          set({ stagedPlan: null, stagedSummary: null });
        },
        recordLossReport(report) {
          set({ lossReport: report, lossAcknowledged: false });
        },
        acknowledgeLoss() {
          set({ lossAcknowledged: true });
        },
        setTimecode(mode) {
          set({ timecode: mode });
        },
        reset() {
          set(initialState());
        },
      },
    }),
  );
}
