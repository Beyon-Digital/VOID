/**
 * Visual-generation view store (W23) — zustand vanilla store holding
 * vis-gen request/result records + camera-conducting view state.
 * View-state only: records fold job telemetry + coordinator results;
 * nothing here is project truth.
 */

import { createStore, StoreApi } from 'zustand/vanilla';
import {
  CalibrationView,
  CameraConsentView,
  CameraView,
  ConductorSessionView,
  DEFAULT_CAMERA_POLICY_VIEW,
  SceneLayerView,
  VisGenArtifactView,
  VisGenProvenanceView,
  VisGenRecordView,
  VisGenStatus,
} from './model';

export interface VisGenViewState {
  records: Record<string, VisGenRecordView>;
  order: string[];
  camera: CameraView;
}

export interface VisGenActions {
  /** Register a request record (submit → pending). */
  requestRecord(rec: {
    recordId: string;
    jobId: string;
    kind: string;
    description?: string | null;
  }): void;
  /** Fold job-telemetry progress for the record's job. */
  applyJobEvent(jobId: string, ev: {
    status?: string;
    percent?: number | null;
    message?: string | null;
  }): boolean;
  /** Attach the collected result (ready) — layers + provenance. */
  setReady(recordId: string, r: {
    layers: SceneLayerView[];
    actionCount: number;
    artifacts: VisGenArtifactView[];
    provenance: VisGenProvenanceView;
  }): void;
  setFailed(recordId: string, code: string, message: string): void;
  accept(recordId: string, transactionId: string): void;
  reject(recordId: string, reason?: string | null): void;
  markStale(recordId: string, cause: string): void;
  /** Solo one artifact for compare (null clears). */
  setCompare(recordId: string, sha256: string | null): void;

  // --- camera conducting (T86) ---
  setConsent(c: CameraConsentView): void;
  setCalibration(c: CalibrationView): void;
  setSession(s: ConductorSessionView): void;
  setClutch(down: boolean): void;
  setHeldCount(n: number): void;
  noteDroppedFrames(n: number): void;
  noteReleaseAll(cause: string): void;
  resetCamera(): void;
  reset(): void;
}

export type VisGenStore = StoreApi<
  VisGenViewState & { actions: VisGenActions }
>;

const TERMINAL: ReadonlySet<VisGenStatus> = new Set([
  'failed',
  'accepted',
  'rejected',
  'stale',
]);

function initialCamera(): CameraView {
  return {
    consent: 'not_asked',
    session: 'idle',
    policy: DEFAULT_CAMERA_POLICY_VIEW,
    calibration: null,
    clutchDown: false,
    heldCount: 0,
    droppedFrames: 0,
    lastReleaseCause: null,
  };
}

export function createVisGenStore(): VisGenStore {
  return createStore<VisGenViewState & { actions: VisGenActions }>()(
    (set, get) => ({
      records: {},
      order: [],
      camera: initialCamera(),
      actions: {
        requestRecord({ recordId, jobId, kind, description }) {
          set((s) => {
            const prev = s.records[recordId];
            const rec: VisGenRecordView = {
              recordId,
              jobId,
              status: prev?.status ?? 'pending',
              kind,
              description: description ?? prev?.description ?? null,
              percent: prev?.percent ?? null,
              message: prev?.message ?? null,
              layers: prev?.layers ?? [],
              actionCount: prev?.actionCount ?? 0,
              artifacts: prev?.artifacts ?? [],
              provenance: prev?.provenance ?? null,
              comparingSha256: prev?.comparingSha256 ?? null,
              transactionId: prev?.transactionId ?? null,
              rejectReason: prev?.rejectReason ?? null,
              staleCause: prev?.staleCause ?? null,
              error: null,
              updatedAt: Date.now(),
            };
            return {
              records: { ...s.records, [recordId]: rec },
              order: prev ? s.order : [recordId, ...s.order],
            };
          });
        },
        applyJobEvent(jobId, ev) {
          const s = get();
          const hit = Object.values(s.records).find((r) => r.jobId === jobId);
          if (!hit) return false;
          const status =
            ev.status === 'running' || ev.status === 'queued'
              ? 'pending'
              : hit.status;
          set((s2) => ({
            records: {
              ...s2.records,
              [hit.recordId]: {
                ...s2.records[hit.recordId],
                status: TERMINAL.has(hit.status) ? hit.status : status,
                percent: ev.percent ?? hit.percent,
                message: ev.message ?? hit.message,
                updatedAt: Date.now(),
              },
            },
          }));
          return true;
        },
        setReady(recordId, r) {
          set((s) => {
            const prev = s.records[recordId];
            if (!prev || prev.status !== 'pending') return {};
            return {
              records: {
                ...s.records,
                [recordId]: {
                  ...prev,
                  status: 'ready',
                  layers: r.layers,
                  actionCount: r.actionCount,
                  artifacts: r.artifacts,
                  provenance: r.provenance,
                  error: null,
                  updatedAt: Date.now(),
                },
              },
            };
          });
        },
        setFailed(recordId, code, message) {
          set((s) => {
            const prev = s.records[recordId];
            if (!prev || TERMINAL.has(prev.status)) return {};
            return {
              records: {
                ...s.records,
                [recordId]: {
                  ...prev,
                  status: 'failed',
                  error: `${code}: ${message}`,
                  updatedAt: Date.now(),
                },
              },
            };
          });
        },
        accept(recordId, transactionId) {
          set((s) => {
            const prev = s.records[recordId];
            if (!prev || prev.status !== 'ready') return {};
            return {
              records: {
                ...s.records,
                [recordId]: {
                  ...prev,
                  status: 'accepted',
                  transactionId,
                  updatedAt: Date.now(),
                },
              },
            };
          });
        },
        reject(recordId, reason) {
          set((s) => {
            const prev = s.records[recordId];
            if (!prev || TERMINAL.has(prev.status)) return {};
            return {
              records: {
                ...s.records,
                [recordId]: {
                  ...prev,
                  status: 'rejected',
                  rejectReason: reason ?? null,
                  updatedAt: Date.now(),
                },
              },
            };
          });
        },
        markStale(recordId, cause) {
          set((s) => {
            const prev = s.records[recordId];
            if (!prev || TERMINAL.has(prev.status)) return {};
            return {
              records: {
                ...s.records,
                [recordId]: {
                  ...prev,
                  status: 'stale',
                  staleCause: cause,
                  updatedAt: Date.now(),
                },
              },
            };
          });
        },
        setCompare(recordId, sha256) {
          set((s) => {
            const prev = s.records[recordId];
            if (!prev) return {};
            return {
              records: {
                ...s.records,
                [recordId]: {
                  ...prev,
                  comparingSha256: sha256,
                  updatedAt: Date.now(),
                },
              },
            };
          });
        },
        setConsent(c) {
          set((s) => ({
            camera: {
              ...s.camera,
              consent: c,
              // deny/revoke disarms: conducting cannot keep running
              session:
                c === 'denied' || c === 'revoked' ? 'idle' : s.camera.session,
              clutchDown:
                c === 'denied' || c === 'revoked' ? false : s.camera.clutchDown,
              heldCount:
                c === 'denied' || c === 'revoked' ? 0 : s.camera.heldCount,
            },
          }));
        },
        setCalibration(c) {
          set((s) => ({
            camera: {
              ...s.camera,
              calibration: c,
              session:
                s.camera.session === 'idle' && s.camera.consent === 'granted'
                  ? 'armed'
                  : s.camera.session === 'idle'
                    ? 'calibrated'
                    : s.camera.session,
            },
          }));
        },
        setSession(sess) {
          set((s) => ({ camera: { ...s.camera, session: sess } }));
        },
        setClutch(down) {
          set((s) => ({ camera: { ...s.camera, clutchDown: down } }));
        },
        setHeldCount(n) {
          set((s) => ({ camera: { ...s.camera, heldCount: n } }));
        },
        noteDroppedFrames(n) {
          set((s) => ({
            camera: { ...s.camera, droppedFrames: s.camera.droppedFrames + n },
          }));
        },
        noteReleaseAll(cause) {
          set((s) => ({
            camera: {
              ...s.camera,
              heldCount: 0,
              clutchDown: false,
              lastReleaseCause: cause,
            },
          }));
        },
        resetCamera() {
          set((s) => ({ camera: { ...initialCamera(), policy: s.camera.policy } }));
        },
        reset() {
          set({ records: {}, order: [], camera: initialCamera() });
        },
      },
    }),
  );
}
