// Generation view store (W15) — zustand vanilla store holding
// generation request + result records. View-state only: the
// JobRunner/coordinator stays authoritative — records are folded from
// job telemetry + result cards, never fabricated. Models the T62
// semantics: source/result A-B, accept an alternate clip while the
// original stays untouched.

import { createStore, StoreApi } from 'zustand/vanilla';

export type GenerationStatus =
  | 'queued'
  | 'running'
  | 'succeeded'
  | 'failed'
  | 'cancelled';

export interface GenerationArtifact {
  /** Artifact name inside the job result (e.g. "generated.wav"). */
  name: string;
  /** SHA-256 of the imported asset bytes. */
  sha256: string;
  /** Asset-store relative path (`assets/sha256/<hash>.<ext>`). */
  assetRel: string;
  bytes: number;
}

export interface GenerationRecord {
  /** Request id — the generation job id. */
  jobId: string;
  modelId: string | null;
  /** Free-form request summary shown in the list (prompt or params). */
  prompt: string | null;
  status: GenerationStatus;
  /** Last progress percent seen (null = none reported yet). */
  percent: number | null;
  message: string | null;
  /** Artifacts the verified result carried — empty until succeeded. */
  artifacts: GenerationArtifact[];
  /** Candidate accepted into the project — set by acceptResult. */
  acceptedSha256: string | null;
  /** Candidate currently soloed for A-B comparison — set by setCompare. */
  comparingSha256: string | null;
  /** Generation request ids that were retried from this record. */
  retryOf: string | null;
  error: string | null;
  updatedAt: number;
}

export interface GenerationViewState {
  records: Record<string, GenerationRecord>;
  order: string[];
}

export interface GenerationActions {
  /** Create or refresh a request record from a submit event. */
  requestJob(rec: {
    jobId: string;
    modelId?: string | null;
    prompt?: string | null;
    retryOf?: string | null;
  }): void;
  /** Fold one job-event-shaped payload for a known generation job. */
  applyEvent(ev: {
    job_id: string;
    status?: string;
    percent?: number | null;
    message?: string | null;
  }): boolean;
  /** Attach the verified result artifact list (terminal success). */
  setResult(jobId: string, artifacts: GenerationArtifact[]): void;
  /** Mark the record failed with the runner-reported error. */
  setFailed(jobId: string, error: string): void;
  /** Mirror a cancel — running→cancelling shows as cancelled intent. */
  markCancelling(jobId: string): void;
  /** Solo one result candidate for A-B comparison (null clears). */
  setCompare(jobId: string, sha256: string | null): void;
  /**
   * Accept a result candidate — records the choice; the underlying
   * asset and every non-accepted candidate stay immutable in the
   * store (T62: original untouched, alternate becomes the clip).
   */
  acceptResult(jobId: string, sha256: string): void;
  reset(): void;
}

export type GenerationStore = StoreApi<
  GenerationViewState & { actions: GenerationActions }
>;

const TERMINAL: ReadonlySet<string> = new Set(['succeeded', 'failed', 'cancelled']);

function normStatus(s: string | undefined): GenerationStatus | null {
  switch (s) {
    case 'queued':
    case 'running':
    case 'succeeded':
    case 'failed':
    case 'cancelled':
      return s;
    case 'cancelling':
      return 'running';
    default:
      return null;
  }
}

export function createGenerationStore(): GenerationStore {
  return createStore<GenerationViewState & { actions: GenerationActions }>()(
    (set, get) => ({
      records: {},
      order: [],
      actions: {
        requestJob({ jobId, modelId, prompt, retryOf }) {
          set((s) => {
            const prev = s.records[jobId];
            const rec: GenerationRecord = {
              jobId,
              modelId: modelId ?? prev?.modelId ?? null,
              prompt: prompt ?? prev?.prompt ?? null,
              status: prev?.status ?? 'queued',
              percent: prev?.percent ?? null,
              message: prev?.message ?? null,
              artifacts: prev?.artifacts ?? [],
              acceptedSha256: prev?.acceptedSha256 ?? null,
              comparingSha256: prev?.comparingSha256 ?? null,
              retryOf: retryOf ?? prev?.retryOf ?? null,
              error: null,
              updatedAt: Date.now(),
            };
            return {
              records: { ...s.records, [jobId]: rec },
              order: prev ? s.order : [...s.order, jobId],
            };
          });
        },
        applyEvent(ev) {
          const status = normStatus(ev.status);
          const prev = get().records[ev.job_id];
          if (!prev) return false;
          set((s) => ({
            records: {
              ...s.records,
              [ev.job_id]: {
                ...prev,
                status: status ?? prev.status,
                percent: ev.percent !== undefined ? ev.percent : prev.percent,
                message: ev.message !== undefined ? ev.message : prev.message,
                updatedAt: Date.now(),
              },
            },
          }));
          return true;
        },
        setResult(jobId, artifacts) {
          const prev = get().records[jobId];
          if (!prev) return;
          set((s) => ({
            records: {
              ...s.records,
              [jobId]: {
                ...prev,
                status: 'succeeded',
                artifacts: artifacts.slice(),
                updatedAt: Date.now(),
              },
            },
          }));
        },
        setFailed(jobId, error) {
          const prev = get().records[jobId];
          if (!prev) return;
          set((s) => ({
            records: {
              ...s.records,
              [jobId]: { ...prev, status: 'failed', error, updatedAt: Date.now() },
            },
          }));
        },
        markCancelling(jobId) {
          const prev = get().records[jobId];
          if (!prev || TERMINAL.has(prev.status)) return;
          set((s) => ({
            records: {
              ...s.records,
              [jobId]: { ...prev, status: 'cancelled', updatedAt: Date.now() },
            },
          }));
        },
        setCompare(jobId, sha256) {
          const prev = get().records[jobId];
          if (!prev) return;
          if (sha256 !== null && !prev.artifacts.some((a) => a.sha256 === sha256)) return;
          set((s) => ({
            records: {
              ...s.records,
              [jobId]: { ...prev, comparingSha256: sha256, updatedAt: Date.now() },
            },
          }));
        },
        acceptResult(jobId, sha256) {
          const prev = get().records[jobId];
          if (!prev || prev.status !== 'succeeded') return;
          if (!prev.artifacts.some((a) => a.sha256 === sha256)) return;
          set((s) => ({
            records: {
              ...s.records,
              [jobId]: {
                ...prev,
                acceptedSha256: sha256,
                comparingSha256: null,
                updatedAt: Date.now(),
              },
            },
          }));
        },
        reset() {
          set({ records: {}, order: [] });
        },
      },
    }),
  );
}
