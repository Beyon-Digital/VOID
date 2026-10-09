// Import dialog view state (W20, T77).
//
// Pure view state: the picked source (.dawproject | .mid | stems dir),
// the parse progress, and — crucially — the loss report rendered as
// VISIBLE entries. Nothing is hidden: every dropped/approximated element
// lands in `report.entries` for the dialog to list, matching the crate's
// sorted canonical order so what the user sees is what the report says.
// The coordinator owns project truth; this store only stages the
// decision + the report.

import { createStore, StoreApi } from 'zustand/vanilla';
import {
  LossReport,
  lossSummary,
  parseLossReport,
} from './types';

export type ImportSourceFormat = 'dawproject' | 'midi' | 'stems';

export type ImportPhase =
  | 'idle'
  | 'parsing'       // reading the container / smf
  | 'review'        // loss report presented for confirmation
  | 'applying'      // wire-op plan being applied (single transaction)
  | 'done'
  | 'failed';

export interface ImportDialogState {
  open: boolean;
  phase: ImportPhase;
  sourceFormat: ImportSourceFormat;
  /** Display name of the picked file/dir (no absolute path needed). */
  sourceName: string;
  /** The canonical loss report from the last parse (empty until review). */
  report: LossReport | null;
  /** Readable error from the last attempt (parse/apply), '' = none. */
  error: string;
  /** True when the user acknowledged the loss entries (required to apply
   *  when report is non-empty — a silent-loss import cannot proceed). */
  acknowledged: boolean;
  /** Track count + note count surfaced post-parse for the summary row. */
  summary: { tracks: number; notes: number } | null;
}

export interface ImportDialogActions {
  setOpen(open: boolean): void;
  setSource(format: ImportSourceFormat, name: string): void;
  /** Begin a parse (UI spins until the runner feeds a report). */
  beginParse(): void;
  /**
   * Feed the runner's loss report payload (JSON or object). Moves to
   * `review` on a valid report, `failed` on malformed payload — a report
   * we cannot read must not be treated as "no loss".
   */
  applyReport(payload: unknown, summary?: { tracks: number; notes: number }): void;
  /** Mark the entries as read — required before `beginApply`. */
  acknowledge(): void;
  /** Gate: applying is allowed only when report is empty or acknowledged. */
  canApply(): boolean;
  beginApply(): void;
  applyDone(): void;
  fail(message: string): void;
  reset(): void;
}

export type ImportDialogStore = StoreApi<
  ImportDialogState & { actions: ImportDialogActions }
>;

export function initialImportDialog(): ImportDialogState {
  return {
    open: false,
    phase: 'idle',
    sourceFormat: 'dawproject',
    sourceName: '',
    report: null,
    error: '',
    acknowledged: false,
    summary: null,
  };
}

export function createImportDialogStore(
  init?: Partial<ImportDialogState>,
): ImportDialogStore {
  return createStore<ImportDialogState & { actions: ImportDialogActions }>()(
    (set, get) => ({
      ...initialImportDialog(),
      ...init,
      actions: {
        setOpen: (open) => set(open ? { open, error: '' } : { open }),
        setSource: (format, name) =>
          set({ sourceFormat: format, sourceName: name, error: '' }),
        beginParse: () =>
          set({ phase: 'parsing', report: null, acknowledged: false, error: '' }),
        applyReport: (payload, summary) => {
          const report =
            typeof payload === 'object' && payload !== null && 'entries' in payload
              ? parseLossReport(payload)
              : parseLossReport(
                  typeof payload === 'string'
                    ? (() => {
                        try {
                          return JSON.parse(payload);
                        } catch {
                          return null;
                        }
                      })()
                    : payload,
                );
          if (!report) {
            set({
              phase: 'failed',
              error: 'unreadable loss report — import refused (a report we cannot read cannot be treated as lossless)',
            });
            return;
          }
          set({
            phase: 'review',
            report,
            summary: summary ?? null,
            acknowledged: report.entries.length === 0,
          });
        },
        acknowledge: () => set({ acknowledged: true }),
        canApply: () => {
          const s = get();
          if (s.phase !== 'review' || !s.report) return false;
          return s.report.entries.length === 0 || s.acknowledged;
        },
        beginApply: () => {
          if (get().actions.canApply()) set({ phase: 'applying' });
        },
        applyDone: () => set({ phase: 'done' }),
        fail: (message) => set({ phase: 'failed', error: message }),
        reset: () => set(initialImportDialog()),
      },
    }),
  );
}

export { lossSummary };
