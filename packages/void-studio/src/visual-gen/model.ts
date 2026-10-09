/**
 * Visual-generation view models (W23, T85/T86 side) — mirrors the
 * void-visfx record/policy wire shapes. View-state only: project
 * truth lives in the coordinator; these are folded from job
 * telemetry + validated results, never fabricated.
 */

/** VisGen lifecycle — matches void-visfx VisGenStatus. */
export type VisGenStatus =
  | 'pending'
  | 'ready'
  | 'failed'
  | 'accepted'
  | 'rejected'
  | 'stale';

export interface VisGenArtifactView {
  filename: string;
  sha256: string;
  bytes: number;
  /** `assets/sha256/<hash>.<ext>` — container-relative. */
  assetRel: string;
}

/** Scene-doc layer as accepted for preview/program (declarative). */
export interface SceneLayerView {
  name: string;
  channel: 'preview' | 'program';
  kind: 'generator' | 'media';
  /** Builtin preset id when kind=generator. */
  preset: string | null;
  /** Artifact name when kind=media. */
  artifact: string | null;
  blend: string | null;
  opacity: number | null;
}

/** Provenance panel facts (T85) — job/generator/model/doc identity. */
export interface VisGenProvenanceView {
  jobId: string;
  generatorId: string;
  generatorVersion: string;
  modelId: string | null;
  runtimeId: string;
  runtimeSha256: string;
  seed: string;
  documentSha256: string;
  documentAsset: string;
}

export interface VisGenRecordView {
  recordId: string;
  jobId: string;
  status: VisGenStatus;
  /** SceneGenKind of the request. */
  kind: string;
  /** Free-text request summary (never interpreted). */
  description: string | null;
  /** Last progress percent from job telemetry (null = none). */
  percent: number | null;
  message: string | null;
  layers: SceneLayerView[];
  /** Action count in the accepted doc (compare view). */
  actionCount: number;
  artifacts: VisGenArtifactView[];
  provenance: VisGenProvenanceView | null;
  /** Candidate soloed for compare (artifact sha256). */
  comparingSha256: string | null;
  transactionId: string | null;
  rejectReason: string | null;
  staleCause: string | null;
  error: string | null;
  updatedAt: number;
}

// ---------------------------------------------------------------------------
// Camera-conducting view (T86) — consent + policy + session telemetry
// ---------------------------------------------------------------------------

export type CameraConsentView = 'not_asked' | 'granted' | 'denied' | 'revoked';
export type ConductorSessionView =
  | 'idle'
  | 'calibrated'
  | 'armed'
  | 'engaged'
  | 'ended';

/** Policy assertions the runtime must enforce — shown to the user. */
export interface CameraAssertsView {
  noRawRetention: boolean;
  noNetworkUpload: boolean;
  maxLandmarkHistory: number;
  dropOccludedFrames: boolean;
  /** ns — tracking-loss window before release-all. */
  lossReleaseWindowNs: string;
}

export interface CameraPolicyView {
  asserts: CameraAssertsView;
  minConfidence: number;
  dropBelowConfidence: number;
  smoothingWindow: number;
  clutchRequired: boolean;
  maxHeld: number;
}

export interface CalibrationView {
  calibratedAt: string;
  trackerModelId: string;
  trackerModelSha256: string;
  measuredConfidence: number;
  measuredLatencyMs: number;
  /** [minX, minY, maxX, maxY] normalized palm bounds. */
  bounds: [number, number, number, number];
}

export interface CameraView {
  consent: CameraConsentView;
  session: ConductorSessionView;
  policy: CameraPolicyView;
  calibration: CalibrationView | null;
  clutchDown: boolean;
  heldCount: number;
  droppedFrames: number;
  /** Last release-all cause seen (null = none). */
  lastReleaseCause: string | null;
}

/** The max-privacy default policy — loosening only by explicit edit. */
export const DEFAULT_CAMERA_POLICY_VIEW: CameraPolicyView = {
  asserts: {
    noRawRetention: true,
    noNetworkUpload: true,
    maxLandmarkHistory: 256,
    dropOccludedFrames: true,
    lossReleaseWindowNs: '750000000',
  },
  minConfidence: 0.5,
  dropBelowConfidence: 0.25,
  smoothingWindow: 4,
  clutchRequired: true,
  maxHeld: 12,
};
