// Export dialog view state (W10, T42).
//
// Pure view state: the user's format/range/name/tail choices plus submit
// plumbing that produces a validated ExportSpecDto (or a list of
// readable validation errors). Nothing here touches the engine; the
// caller wires `submit` into whatever invoke surface the integrator adds
// for the export runner (protocol gap — documented in spec.ts).

import { createStore, StoreApi } from 'zustand/vanilla';
import {
  buildExportSpec,
  ExportDialogSelection,
  ExportContext,
  ExportSpecDto,
  ExportSpecError,
  ExportTailPolicy,
  APPROVED_SAMPLE_RATES,
} from './spec';
import {
  applyJobEvent,
  ExportProgress,
  initialProgress,
  isTerminal,
  parseJobEvent,
} from './job';
import {
  exportResultsFromItems,
  type ExportResultItem,
} from './results';
import type { ReadItem } from 'void-client';

export interface ExportDialogState {
  open: boolean;
  outputName: string;
  format: 'wav' | 'midi';
  rangeStartTicks: string;
  rangeEndTicks: string;
  channels: 'mono' | 'stereo';
  bitDepth: 'pcm16' | 'pcm24' | 'float32';
  sampleRate: number;
  tail: ExportTailPolicy;
  /** Validation errors from the last submit attempt (empty = clean). */
  errors: string[];
  /** Live job progress keyed by jobId (only real events, never faked). */
  jobs: Record<string, ExportProgress>;
  /** Result cards seen via the export list view. */
  results: ExportResultItem[];
}

export interface ExportDialogActions {
  setOpen(open: boolean): void;
  set<K extends keyof ExportDialogSelection>(field: K, value: ExportDialogSelection[K]): void;
  setTailNone(): void;
  setTailMs(ms: number): void;
  setTailTicks(ticks: string): void;
  /**
   * Build + validate the spec. Returns it or null; `errors` holds
   * readable messages on failure.
   */
  submit(ctx: ExportContext): ExportSpecDto | null;
  /** Feed a telemetry payload — job events update progress. */
  applyTelemetry(payload: unknown): void;
  /** Feed a read page of export cards. */
  applyResults(items: ReadItem[]): void;
  reset(): void;
}

export type ExportDialogStore = StoreApi<
  ExportDialogState & { actions: ExportDialogActions }
>;

export function initialExportDialog(): ExportDialogState {
  return {
    open: false,
    outputName: 'mix',
    format: 'wav',
    rangeStartTicks: '0',
    rangeEndTicks: '3840000',
    channels: 'stereo',
    bitDepth: 'pcm24',
    sampleRate: 48_000,
    tail: { mode: 'none' },
    errors: [],
    jobs: {},
    results: [],
  };
}

export function createExportDialogStore(
  init?: Partial<ExportDialogState>,
): ExportDialogStore {
  return createStore<ExportDialogState & { actions: ExportDialogActions }>()(
    (set, get) => ({
      ...initialExportDialog(),
      ...init,
      actions: {
        setOpen: (open) => set({ open, errors: open ? [] : get().errors }),
        set: (field, value) => set({ [field]: value } as never),
        setTailNone: () => set({ tail: { mode: 'none' } }),
        setTailMs: (ms) => set({ tail: { mode: 'milliseconds', ms } }),
        setTailTicks: (ticks) => set({ tail: { mode: 'ticks', ticks } }),
        submit: (ctx) => {
          const s = get();
          const sel: ExportDialogSelection = {
            outputName: s.outputName,
            format: s.format,
            rangeStartTicks: s.rangeStartTicks,
            rangeEndTicks: s.rangeEndTicks,
            channels: s.format === 'wav' ? s.channels : undefined,
            bitDepth: s.format === 'wav' ? s.bitDepth : undefined,
            sampleRate: s.format === 'wav' ? s.sampleRate : undefined,
            tail: s.tail,
          };
          try {
            const spec = buildExportSpec(sel, ctx);
            const progress = initialProgress(ctx.jobId);
            set({
              errors: [],
              jobs: { ...s.jobs, [ctx.jobId]: progress },
            });
            return spec;
          } catch (e) {
            const msg =
              e instanceof ExportSpecError
                ? e.message
                : e instanceof Error
                  ? e.message
                  : String(e);
            set({ errors: [msg] });
            return null;
          }
        },
        applyTelemetry: (payload) => {
          const ev = parseJobEvent(payload);
          if (!ev) return;
          const s = get();
          const prev = s.jobs[ev.job_id] ?? initialProgress(ev.job_id);
          set({ jobs: { ...s.jobs, [ev.job_id]: applyJobEvent(prev, ev) } });
        },
        applyResults: (items) => {
          const found = exportResultsFromItems(items);
          if (found.length === 0) return;
          set((s) => {
            const seen = new Set(s.results.map((r) => r.jobId));
            const merged = [...s.results];
            for (const r of found) {
              if (!seen.has(r.jobId)) merged.push(r);
              else {
                const i = merged.findIndex((x) => x.jobId === r.jobId);
                merged[i] = r;
              }
            }
            return { results: merged };
          });
        },
        reset: () => set(initialExportDialog()),
      },
    }),
  );
}

/** Convenience selectors. */
export function activeJobs(state: ExportDialogState): ExportProgress[] {
  return Object.values(state.jobs).filter((j) => !isTerminal(j.status));
}
export function finishedJobs(state: ExportDialogState): ExportProgress[] {
  return Object.values(state.jobs).filter((j) => isTerminal(j.status));
}
export { APPROVED_SAMPLE_RATES };
