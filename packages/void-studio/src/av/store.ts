// AV export view-state store (W24).
//
// Pure view state: the user's codec/size/name choices plus the job
// progress map. Truth lives in the coordinator — this store only folds
// job events and result cards into renderable state, exactly like the
// audio export dialog store.

import { createStore, StoreApi } from 'zustand/vanilla';
import {
  AV_CODEC_MATRIX,
  AvCodecGateReport,
  avCodecGate,
  selectableAvCodecs,
  type AvCodecSpec,
} from './codec';
import {
  avFramePlan,
  AvExportSpecDto,
  AvSpecError,
  validateAvExportSpec,
  type AvInputDto,
  type AvTailPolicy,
  type AvFrameRate,
} from './spec';
import {
  applyJobEvent,
  initialProgress,
  isTerminal,
  parseJobEvent,
  type ExportProgress,
} from './job';
import {
  avExportResultsFromItems,
  type AvExportResultItem,
} from './results';
import type { ReadItem } from 'void-client';

export interface AvExportDraft {
  jobId: string;
  projectId: string;
  checkpointId: string;
  sourceRevision: string;
  sampleRate: number;
  rangeSamples: string;
  inputs: AvInputDto[];
}

export interface AvExportPanelState {
  open: boolean;
  outputName: string;
  codecId: string;
  width: number;
  height: number;
  rateNum: string;
  rateDen: string;
  tail: AvTailPolicy;
  /** Real gate reports — the only "supported" flags shown. */
  codecGates: AvCodecGateReport[];
  ffmpegPresent: boolean;
  encodersPresent: Set<string>;
  draft: AvExportDraft | null;
  /** jobId → progress (only real reported percent). */
  jobs: Record<string, ExportProgress>;
  results: AvExportResultItem[];
  errors: string[];
}

export interface AvExportPanelActions {
  setOpen(open: boolean): void;
  setField<K extends keyof AvExportPanelState>(key: K, value: AvExportPanelState[K]): void;
  setDraft(draft: AvExportDraft | null): void;
  setEncoderAvailability(present: boolean, encoders: string[]): void;
  buildSpec(): AvExportSpecDto | null;
  onJobEvent(payload: unknown): void;
  onReadItems(items: ReadItem[]): void;
  reset(): void;
}

const DEFAULT_STATE: Omit<AvExportPanelState, never> = {
  open: false,
  outputName: 'av-export',
  codecId: 'ffv1_flac_mkv',
  width: 1920,
  height: 1080,
  rateNum: '30000',
  rateDen: '1001',
  tail: { mode: 'none' },
  codecGates: [],
  ffmpegPresent: false,
  encodersPresent: new Set<string>(),
  draft: null,
  jobs: {},
  results: [],
  errors: [],
};

function regate(state: AvExportPanelState): AvCodecGateReport[] {
  return AV_CODEC_MATRIX.map((c) =>
    avCodecGate(c.id, state.ffmpegPresent, (n) => state.encodersPresent.has(n)),
  );
}

export type AvExportPanelStore = StoreApi<AvExportPanelState & { actions: AvExportPanelActions }>;

export function createAvExportPanelStore(): AvExportPanelStore {
  return createStore<AvExportPanelState & { actions: AvExportPanelActions }>()((set, get) => ({
    ...DEFAULT_STATE,
    codecGates: AV_CODEC_MATRIX.map((c) =>
      avCodecGate(c.id, false, () => false),
    ),
    actions: {
      setOpen: (open) => set({ open }),
      setField: (key, value) => set({ [key]: value } as Partial<AvExportPanelState>),
      setDraft: (draft) => set({ draft }),
      setEncoderAvailability: (present, encoders) =>
        set((s) => ({
          ffmpegPresent: present,
          encodersPresent: new Set(encoders),
          codecGates: regate({ ...s, ffmpegPresent: present, encodersPresent: new Set(encoders) }),
        })),
      buildSpec: () => {
        const s = get();
        const draft = s.draft;
        if (!draft) {
          set({ errors: ['no export draft — pick a checkpoint first'] });
          return null;
        }
        const gate = avCodecGate(s.codecId, s.ffmpegPresent, (n) => s.encodersPresent.has(n));
        if (!gate.selectable) {
          set({ errors: [gate.reason ?? `codec ${s.codecId} unavailable`] });
          return null;
        }
        const spec: AvExportSpecDto = {
          jobId: draft.jobId,
          projectId: draft.projectId,
          checkpointId: draft.checkpointId,
          sourceRevision: draft.sourceRevision,
          sampleRate: draft.sampleRate,
          channels: 'stereo',
          frameRateNum: s.rateNum,
          frameRateDen: s.rateDen,
          rounding: 'nearest_ties_away',
          rangeSamples: draft.rangeSamples,
          width: s.width,
          height: s.height,
          codecId: s.codecId,
          inputs: draft.inputs,
          cues: [],
          tail: s.tail,
          timeoutMs: '300000',
          maxOutputBytes: '68719476736',
          outputName: s.outputName,
        };
        try {
          validateAvExportSpec(spec);
          avFramePlan(spec);
        } catch (e) {
          set({ errors: e instanceof AvSpecError ? [`${e.field}: ${e.message}`] : [String(e)] });
          return null;
        }
        set({ errors: [] });
        return spec;
      },
      onJobEvent: (payload) => {
        const ev = parseJobEvent(payload);
        if (!ev) return;
        set((s) => ({
          jobs: { ...s.jobs, [ev.job_id]: applyJobEvent(s.jobs[ev.job_id] ?? initialProgress(ev.job_id), ev) },
        }));
      },
      onReadItems: (items) => set((s) => ({ results: [...s.results, ...avExportResultsFromItems(items)] })),
      reset: () =>
        set({
          ...DEFAULT_STATE,
          codecGates: AV_CODEC_MATRIX.map((c) => avCodecGate(c.id, false, () => false)),
          encodersPresent: new Set<string>(),
        }),
    },
  }));
}

/** Derived selectors (view-state only). */
export function avActiveJobs(s: AvExportPanelState): ExportProgress[] {
  return Object.values(s.jobs).filter((j) => !isTerminal(j.status));
}

export function avFinishedJobs(s: AvExportPanelState): ExportProgress[] {
  return Object.values(s.jobs).filter((j) => isTerminal(j.status));
}

export function avSelectableCodecs(s: AvExportPanelState): AvCodecSpec[] {
  return selectableAvCodecs(s.ffmpegPresent, (n) => s.encodersPresent.has(n));
}

export function avRateOf(s: AvExportPanelState): AvFrameRate {
  return { num: BigInt(s.rateNum || '0'), den: BigInt(s.rateDen || '1') };
}
