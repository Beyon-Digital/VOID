// Exchange export dialog view state (W20, T77).
//
// Mirrors the W10 export/dialog.ts store but for INTERCHANGE formats —
// .dawproject, .mid, stems — whose defining trait is the loss report.
// The report the runner produces is surfaced as visible entries; the
// dialog cannot be "successfully closed" while unreadable loss info is
// outstanding (a report we cannot parse is a failure, not silence).

import { createStore, StoreApi } from 'zustand/vanilla';
import { LossReport, parseLossReport } from './types';

export type ExchangeExportFormat = 'dawproject' | 'midi' | 'stems';

export interface ExchangeExportState {
  open: boolean;
  format: ExchangeExportFormat;
  /** Include payload files (media + plugin state blobs) in the container.
   *  When false, audio/state references export as descriptors-only and
   *  the loss report records each omitted payload honestly. */
  includePayloads: boolean;
  /** Report from the last completed export run (null until done). */
  report: LossReport | null;
  /** Unreadable-report flag: the run produced a payload we could not
   *  parse — shown as an error, never as "no loss". */
  reportUnreadable: boolean;
  errors: string[];
  phase: 'idle' | 'running' | 'done' | 'failed';
}

export interface ExchangeExportActions {
  setOpen(open: boolean): void;
  setFormat(f: ExchangeExportFormat): void;
  setIncludePayloads(v: boolean): void;
  begin(): void;
  /** Feed the finished run's loss report (JSON string or object). */
  applyReport(payload: unknown): void;
  fail(message: string): void;
  reset(): void;
}

export type ExchangeExportStore = StoreApi<
  ExchangeExportState & { actions: ExchangeExportActions }
>;

export function initialExchangeExport(): ExchangeExportState {
  return {
    open: false,
    format: 'dawproject',
    includePayloads: true,
    report: null,
    reportUnreadable: false,
    errors: [],
    phase: 'idle',
  };
}

export function createExchangeExportStore(
  init?: Partial<ExchangeExportState>,
): ExchangeExportStore {
  return createStore<ExchangeExportState & { actions: ExchangeExportActions }>()(
    (set, get) => ({
      ...initialExchangeExport(),
      ...init,
      actions: {
        setOpen: (open) => set(open ? { open, errors: [] } : { open }),
        setFormat: (format) => set({ format }),
        setIncludePayloads: (includePayloads) => set({ includePayloads }),
        begin: () => set({ phase: 'running', report: null, reportUnreadable: false, errors: [] }),
        applyReport: (payload) => {
          const obj =
            typeof payload === 'string'
              ? (() => {
                  try {
                    return JSON.parse(payload);
                  } catch {
                    return null;
                  }
                })()
              : payload;
          const report = parseLossReport(obj);
          if (!report) {
            set({ phase: 'failed', reportUnreadable: true, errors: ['unreadable loss report'] });
            return;
          }
          set({ phase: 'done', report });
        },
        fail: (message) => set({ phase: 'failed', errors: [message] }),
        reset: () => set(initialExchangeExport()),
      },
    }),
  );
}
