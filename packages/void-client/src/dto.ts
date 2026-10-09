// DTO types for the VOID control/telemetry surface.
//
// These types mirror apps/void-tauri/src-tauri/src/codec.rs exactly: every
// signed/unsigned 64-bit field (revision, engine_epoch, *_ticks,
// timeline_sample, device_sample_counter, sequence, host_clock_ns,
// tempo_map_revision) is a DECIMAL STRING in JSON — never a JS number —
// because int64 values exceed Number.MAX_SAFE_INTEGER. Ops are tagged
// unions: exactly one key naming the op, value is its field object.

import { i64str, u64str } from './i64';

/** Decimal-string int64/u64 as carried on the wire. */
export type I64 = string;

// ---------------------------------------------------------------------------
// Enums (names match codec.rs variant names)
// ---------------------------------------------------------------------------

export type TrackKind = 'AUDIO' | 'MIDI' | 'INSTRUMENT' | 'BUS';
export type TransportOpName =
  | 'PLAY'
  | 'STOP'
  | 'SEEK'
  | 'PANIC'
  | 'SET_CYCLE'
  | 'SET_TEMPO_LIVE';
export type ViewKindName =
  | 'PROJECT_SUMMARY'
  | 'TRACK_LIST'
  | 'CLIP_LIST'
  | 'NOTE_RANGE'
  | 'ASSET_LIST'
  | 'PLUGIN_LIST'
  | 'RECEIPT_LIST'
  // rev-2 (protocol minor 1). The wire also accepts the pinned snake_case
  // spellings used by void-studio request builders ('job_list', …).
  | 'TAKE_LIST'
  | 'INPUT_DEVICE_LIST'
  | 'JOB_LIST'
  | 'MODEL_LIST'
  | 'PROPOSAL_LIST'
  | 'SCENE_LIST'
  | 'take_list'
  | 'input_device_list'
  | 'job_list'
  | 'model_list'
  | 'proposal_list'
  | 'scene_list';

/** ArmTrackOp.monitor_mode values (MonitorMode enum, rev-2). */
export type MonitorMode = 'OFF' | 'AUTOMATIC' | 'ON';

/** Launch op quantize values (LaunchQuantize enum, rev-2). */
export type LaunchQuantize = 'IMMEDIATE' | 'BAR' | 'BEAT' | 'CUSTOM';
export type AckStatus = 'APPLIED' | 'DUPLICATE' | 'REJECTED' | 'OUTCOME_UNKNOWN';
export type TransportState = 'STOPPED' | 'PLAYING' | 'RECORDING' | 'PAUSED';
export type SaveStatus = 'SAVE_DURABLE' | 'SAVE_FAILED';
export type ErrorCode =
  | 'NONE'
  | 'BAD_REQUEST'
  | 'UNSUPPORTED_VERSION'
  | 'UNSUPPORTED_CAPABILITY'
  | 'NOT_AUTHORIZED'
  | 'NOT_FOUND'
  | 'STALE_REVISION'
  | 'STALE_EPOCH'
  | 'COMMAND_ID_REUSE'
  | 'BUSY'
  | 'DEVICE_UNAVAILABLE'
  | 'ASSET_MISSING'
  | 'PLUGIN_UNAVAILABLE'
  | 'DISK_FULL'
  | 'WORKER_FAILED'
  | 'CANCELLED'
  | 'OUTCOME_UNKNOWN';

// ---------------------------------------------------------------------------
// Persistent op field objects (union members of PersistentOp in the .fbs)
// ---------------------------------------------------------------------------

export interface CreateProjectOp {
  name: string;
  container_dir: string;
  sample_rate?: number;
  initial_bpm?: number;
}
export interface OpenProjectOp {
  container_dir: string;
}
export interface CloseProjectOp {
  // no fields
}
export interface AddTrackOp {
  track_id: string;
  kind: TrackKind;
  name?: string;
  index?: number; // -1 appends
}
export interface RemoveTrackOp {
  track_id: string;
}
export interface SetTrackNameOp {
  track_id: string;
  name: string;
}
export interface SetTrackGainOp {
  track_id: string;
  gain_linear: number; // finite only
}
export interface SetTrackPanOp {
  track_id: string;
  pan: number; // -1.0..1.0
}
export interface SetTrackMuteOp {
  track_id: string;
  muted: boolean;
}
export interface SetTrackSoloOp {
  track_id: string;
  soloed: boolean;
}
export interface InsertAudioClipOp {
  clip_id: string;
  track_id: string;
  asset_id: string;
  start_ticks: I64;
  length_ticks: I64;
  offset_ticks?: I64;
}
export interface InsertMidiClipOp {
  clip_id: string;
  track_id: string;
  start_ticks: I64;
  length_ticks: I64;
}
export interface RemoveClipOp {
  clip_id: string;
}
export interface MoveClipOp {
  clip_id: string;
  track_id: string;
  start_ticks: I64;
}
export interface TrimClipOp {
  clip_id: string;
  start_ticks: I64;
  length_ticks: I64;
  offset_ticks?: I64;
}
export interface SplitClipOp {
  clip_id: string;
  at_ticks: I64;
  new_clip_id: string;
}
export interface InsertNoteOp {
  clip_id: string;
  note_id: string;
  pitch: number; // 0..127
  velocity: number; // 1..127
  start_ticks: I64;
  length_ticks: I64;
}
export interface RemoveNoteOp {
  clip_id: string;
  note_id: string;
}
export interface SetNoteOp {
  clip_id: string;
  note_id: string;
  pitch?: number; // -1 (or omitted) = unchanged
  velocity?: number;
  start_ticks?: I64;
  length_ticks?: I64;
}
export interface SetTempoOp {
  at_ticks: I64;
  bpm: number;
}
export interface SetTimeSignatureOp {
  at_ticks: I64;
  numerator: number;
  denominator: number;
}
export interface SetLoopRangeOp {
  start_ticks: I64;
  end_ticks: I64;
  enabled: boolean;
}
export interface SaveProjectOp {
  reason?: string;
}
export interface CreateCheckpointOp {
  reason?: string;
}
export interface UndoOp {
  transaction_id?: string; // empty = latest
}
export interface RedoOp {
  transaction_id?: string;
}
export interface AttachAssetOp {
  asset_id: string;
  sha256: string;
  media_type?: string;
  rel_path: string; // project-relative, never absolute/traversing
  channels?: number;
  duration_ticks?: I64;
}
export interface InsertPluginOp {
  track_id: string;
  slot?: number;
  plugin_instance_id: string;
  format: string; // e.g. "VST3", "AU"
  plugin_uid: string;
}
export interface RemovePluginOp {
  plugin_instance_id: string;
}
export interface SetPluginParamOp {
  plugin_instance_id: string;
  param_id: string;
  value: number;
}
export interface OpenPluginEditorOp {
  plugin_instance_id: string;
}
export interface ClosePluginEditorOp {
  plugin_instance_id: string;
}

// -- rev-2 ops (protocol minor 1) --------------------------------------------

/** Arm/disarm a track for recording (NEEDS §7). */
export interface ArmTrackOp {
  track_id: string;
  record_enabled?: boolean; // default true
  /** INPUT_DEVICE_LIST row id; '' = engine default input. */
  input_device?: string;
  monitor_mode?: MonitorMode; // default AUTOMATIC
  is_midi?: boolean;
}

/** Start a recording take; take_id '' = engine mints one. */
export interface StartRecordingOp {
  take_id?: string;
}
export interface StopRecordingOp {
  /** true = discard the take instead of committing it. */
  discard?: boolean;
}
/** Count-in: mode 'off' | 'bars' with bars 1..16. */
export interface SetCountInOp {
  mode: 'off' | 'bars';
  bars?: number;
}
export interface SetMetronomeOp {
  enabled: boolean;
  gain?: number; // 0..1, default 0.5
  recording_only?: boolean;
}
export interface SetPunchInOutOp {
  enabled: boolean;
  in_ticks?: I64; // required when enabled: 0 <= in < out
  out_ticks?: I64;
}

/** serde shape of void_jobs::JobSpec — camelCase, decimal-string numerics. */
export interface JobSpecEnvelope {
  jobId: string;
  projectId: string;
  sourceRevision: I64;
  contextSha256: string;
  kind:
    | 'symbolic'
    | 'transcription'
    | 'separation'
    | 'audio_generation'
    | 'visual_generation'
    | 'analysis'
    | 'av_export';
  runtimeId: string;
  runtimeSha256?: string;
  modelId?: string;
  modelSha256?: string;
  inputs: string[];
  parameters?: unknown;
  reservations?: { ramBytes?: I64; vramBytes?: I64; cpuThreads?: number };
  deadlineMonotonicNs?: I64;
  outputScopeToken: string;
  cloudConsentId?: string;
}
export interface SubmitJobOp {
  spec: JobSpecEnvelope;
}
export interface CancelJobOp {
  job_id: string;
}
export interface PauseJobOp {
  job_id: string;
}
export interface InstallModelOp {
  model_id: string;
  model_version: string;
  source_uri: string;
}
export interface RequestProposalOp {
  context_digest: string;
  seed?: I64;
  max_proposals?: number; // 1..8
}
export interface ResolveProposalOp {
  proposal_id: string;
  accept: boolean;
  /** candidate index within the proposal; -1/absent = the chosen one. */
  candidate_rank?: number;
}
export interface PreviewNoteDto {
  note_id?: string;
  pitch: number; // 0..127
  velocity: number; // 1..127
  start_ticks: I64;
  length_ticks: I64;
}
/** Transient non-persistent preview (audition) layer for a proposal. */
export interface PreviewLayerOp {
  proposal_id: string;
  clip_id: string;
  notes: PreviewNoteDto[];
  enable?: boolean; // default true
}
/** Ingest a host-side file into container assets (NEEDS §17). */
export interface IngestAssetOp {
  rel_path: string;
  media_type: string;
}
/** Relink a missing asset when the candidate's bytes match sha256. */
export interface RelinkAssetOp {
  asset_id: string;
  sha256: string;
}
export interface SetPluginBypassOp {
  plugin_instance_id: string;
  bypassed: boolean;
}
export interface RescanPluginsOp {
  /** '' = full rescan of the scanned-plugin cache. */
  plugin_uid?: string;
}
export interface RestorePluginStateOp {
  plugin_instance_id: string;
  state_asset_id: string;
}
/** Checkpoint into a NEW container directory (NEEDS §3 save-as). */
export interface SaveProjectAsOp {
  container_dir: string;
  name?: string;
  reason?: string;
}
export interface LaunchSceneOp {
  scene_id: string;
  quantize?: LaunchQuantize; // default BAR
  quantize_ticks?: I64; // required >0 when quantize = CUSTOM
}
export interface StopSceneOp {
  /** '' = stop every playing slot. */
  scene_id?: string;
  quantize?: LaunchQuantize;
  quantize_ticks?: I64;
}
export interface LaunchClipOp {
  slot_id: string;
  quantize?: LaunchQuantize;
  quantize_ticks?: I64;
}

/**
 * Pinned flat rev-2 op shapes (void-studio jobs/job.ts): a snake_case
 * `op` discriminator with sibling fields, equivalent to the tagged form.
 */
export interface SubmitJobFlatOp {
  op: 'submit_job';
  spec: JobSpecEnvelope;
}
export interface CancelJobFlatOp {
  op: 'cancel_job';
  jobId: string;
}
export interface PauseJobFlatOp {
  op: 'pause_job';
  jobId: string;
}
export interface InstallModelFlatOp {
  op: 'install_model';
  modelId: string;
  modelVersion: string;
  sourceUri: string;
}
export type FlatPersistentOp =
  | SubmitJobFlatOp
  | CancelJobFlatOp
  | PauseJobFlatOp
  | InstallModelFlatOp;

/** Tagged persistent op: exactly one key. */
export type PersistentOp =
  | { CreateProjectOp: CreateProjectOp }
  | { OpenProjectOp: OpenProjectOp }
  | { CloseProjectOp: CloseProjectOp }
  | { AddTrackOp: AddTrackOp }
  | { RemoveTrackOp: RemoveTrackOp }
  | { SetTrackNameOp: SetTrackNameOp }
  | { SetTrackGainOp: SetTrackGainOp }
  | { SetTrackPanOp: SetTrackPanOp }
  | { SetTrackMuteOp: SetTrackMuteOp }
  | { SetTrackSoloOp: SetTrackSoloOp }
  | { InsertAudioClipOp: InsertAudioClipOp }
  | { InsertMidiClipOp: InsertMidiClipOp }
  | { RemoveClipOp: RemoveClipOp }
  | { MoveClipOp: MoveClipOp }
  | { TrimClipOp: TrimClipOp }
  | { SplitClipOp: SplitClipOp }
  | { InsertNoteOp: InsertNoteOp }
  | { RemoveNoteOp: RemoveNoteOp }
  | { SetNoteOp: SetNoteOp }
  | { SetTempoOp: SetTempoOp }
  | { SetTimeSignatureOp: SetTimeSignatureOp }
  | { SetLoopRangeOp: SetLoopRangeOp }
  | { SaveProjectOp: SaveProjectOp }
  | { CreateCheckpointOp: CreateCheckpointOp }
  | { UndoOp: UndoOp }
  | { RedoOp: RedoOp }
  | { AttachAssetOp: AttachAssetOp }
  | { InsertPluginOp: InsertPluginOp }
  | { RemovePluginOp: RemovePluginOp }
  | { SetPluginParamOp: SetPluginParamOp }
  | { OpenPluginEditorOp: OpenPluginEditorOp }
  | { ClosePluginEditorOp: ClosePluginEditorOp }
  // rev-2 (protocol minor 1)
  | { ArmTrackOp: ArmTrackOp }
  | { StartRecordingOp: StartRecordingOp }
  | { StopRecordingOp: StopRecordingOp }
  | { SetCountInOp: SetCountInOp }
  | { SetMetronomeOp: SetMetronomeOp }
  | { SetPunchInOutOp: SetPunchInOutOp }
  | { SubmitJobOp: SubmitJobOp }
  | { CancelJobOp: CancelJobOp }
  | { PauseJobOp: PauseJobOp }
  | { InstallModelOp: InstallModelOp }
  | { RequestProposalOp: RequestProposalOp }
  | { ResolveProposalOp: ResolveProposalOp }
  | { PreviewLayerOp: PreviewLayerOp }
  | { IngestAssetOp: IngestAssetOp }
  | { RelinkAssetOp: RelinkAssetOp }
  | { SetPluginBypassOp: SetPluginBypassOp }
  | { RescanPluginsOp: RescanPluginsOp }
  | { RestorePluginStateOp: RestorePluginStateOp }
  | { SaveProjectAsOp: SaveProjectAsOp }
  | { LaunchSceneOp: LaunchSceneOp }
  | { StopSceneOp: StopSceneOp }
  | { LaunchClipOp: LaunchClipOp };

/** Tagged union plus the pinned flat rev-2 op DTOs. */
export type SendableOp = PersistentOp | FlatPersistentOp;

type KeysOfUnion<T> = T extends T ? keyof T : never;
export type PersistentOpName = KeysOfUnion<PersistentOp>;

// ---------------------------------------------------------------------------
// Request DTOs (webview -> commands.rs / codec.rs)
// ---------------------------------------------------------------------------

/** PersistentCommand DTO as consumed by codec::persistent_command_json. */
export interface PersistentCommandDto {
  command_id: string;
  transaction_id: string;
  project_id: string;
  engine_epoch: I64; // u64 decimal string
  expected_revision: I64; // u64 decimal string
  op: SendableOp;
  /** Mirrored at top level for lifecycle registration in commands.rs. */
  container_dir?: string;
}

/** TransportRequest DTO as consumed by codec::transport_request_json. */
export interface TransportRequestDto {
  request_id: string;
  project_id: string;
  engine_epoch: I64;
  op: TransportOpName;
  position_ticks?: I64; // SEEK only; -1 = n/a
  cycle_start_ticks?: I64; // SET_CYCLE only
  cycle_end_ticks?: I64;
  value?: number; // generic payload (e.g. tempo)
}

/** ReadRequest DTO as consumed by codec::read_request_json. */
export interface ReadRequestDto {
  request_id: string;
  project_id: string;
  view: ViewKindName;
  cursor?: string; // "" = start
  limit?: number; // 0 = server default (<= 2000 objects / 512 KiB)
  track_id?: string; // optional scope filter
  start_ticks?: I64; // NOTE_RANGE window; -1 = unbounded
  end_ticks?: I64;
  /** JOB_LIST/PROPOSAL_LIST: include terminal-state rows (rev-2). */
  include_terminal?: boolean;
}

// ---------------------------------------------------------------------------
// Response / event DTOs (worker -> webview; codec.rs event_frame_json /
// telemetry_json output shapes)
// ---------------------------------------------------------------------------

export interface CommandReceipt {
  kind: 'CommandReceipt';
  command_id: string;
  transaction_id?: string;
  status: AckStatus;
  error: ErrorCode;
  /** u64 decimal string; absent on coordinator-side synthetic receipts. */
  revision?: I64;
  engine_epoch?: I64;
  message?: string;
  payload_hash?: string;
}

export interface ReadItem {
  object_id: string;
  /** Small bounded JSON projection of the object (engine-defined shape). */
  summary_json: string;
}

export interface ReadResponse {
  kind: 'ReadResponse';
  request_id: string;
  revision: I64;
  items: ReadItem[];
  next_cursor: string; // "" when exhausted
  done: boolean;
  error: ErrorCode;
}

export interface PluginScanResult {
  kind: 'PluginScanResult';
  request_id: string;
  ok: boolean;
  quarantined: boolean;
  plugin_uid: string;
  binary_sha256: string;
  name: string;
  version: string;
  arch: string;
  error: ErrorCode;
  message: string;
}

export type ControlEvent = CommandReceipt | ReadResponse | PluginScanResult;

// -- telemetry channel --------------------------------------------------------

export interface ClockSnapshot {
  kind: 'ClockSnapshot';
  project_id: string;
  engine_epoch: I64;
  timeline_sample: I64; // position on the (possibly looping) timeline
  device_sample_counter: I64; // monotonic device counter — no loop wrap
  sample_rate: number;
  transport: TransportState;
  loop_start_ticks: I64;
  loop_end_ticks: I64;
  tempo_map_revision: I64;
  sequence: I64;
  host_clock_ns: I64;
}

export interface MeterFrame {
  kind: 'MeterFrame';
  project_id: string;
  engine_epoch: I64;
  track_id: string;
  peak_l: number;
  peak_r: number;
  rms_l: number;
  rms_r: number;
  clipped: boolean;
  sequence: I64;
}

export interface SaveResultEvent {
  kind: 'SaveResultEvent';
  project_id: string;
  command_id: string;
  status: SaveStatus;
  revision: I64;
  checkpoint_id: string;
  manifest_sha256: string;
  error: ErrorCode;
  message: string;
}

export interface TransportAck {
  kind: 'TransportAck';
  request_id: string;
  ok: boolean;
  error: ErrorCode;
}

/** Returned by send_transport for PANIC (acknowledged by send itself). */
export interface PanicAccepted {
  accepted: true;
}

/** rev-2: job status/progress transition (NEEDS §11). */
export interface JobEvent {
  kind: 'JobEvent';
  project_id: string;
  job_id: string;
  status:
    | 'queued'
    | 'running'
    | 'succeeded'
    | 'failed'
    | 'cancelling'
    | 'cancelled';
  /** <0 means the job has not reported progress. */
  percent: number;
  message: string;
  quarantined: boolean;
}

/** rev-2: a live proposal was invalidated (NEEDS §14). */
export interface ProposalStaleEvent {
  kind: 'ProposalStaleEvent';
  project_id: string;
  proposal_id: string;
  cause: 'context_changed' | 'target_gone' | 'session_ended' | string;
}

/** rev-2: an input device an armed track needs disappeared (NEEDS §7). */
export interface InputDeviceLostEvent {
  kind: 'InputDeviceLostEvent';
  project_id: string;
  device_id: string;
  device_name: string;
}

export type TelemetryEvent =
  | ClockSnapshot
  | MeterFrame
  | SaveResultEvent
  | TransportAck
  | JobEvent
  | ProposalStaleEvent
  | InputDeviceLostEvent;

// -- engine lifecycle ---------------------------------------------------------

export interface EngineStatus {
  attached: boolean;
  worker_id?: string;
  engine_epoch?: I64;
  state?: string;
}

export interface EngineLostEvent {
  worker_id: string;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

export function isReceipt(v: unknown): v is CommandReceipt {
  return (
    typeof v === 'object' &&
    v !== null &&
    (v as { kind?: string }).kind === 'CommandReceipt'
  );
}

export function receiptOk(r: CommandReceipt): boolean {
  return r.status === 'APPLIED' || r.status === 'DUPLICATE';
}

/** Zero revision / epoch as decimal string. */
export const ZERO_I64: I64 = '0';

/** Build a PersistentCommandDto with current epoch/revision context. */
export function commandDto(fields: {
  commandId: string;
  transactionId?: string;
  projectId: string;
  engineEpoch: string | number | bigint;
  expectedRevision: string | number | bigint;
  op: SendableOp;
}): PersistentCommandDto {
  const dto: PersistentCommandDto = {
    command_id: fields.commandId,
    transaction_id: fields.transactionId ?? '',
    project_id: fields.projectId,
    // engine_epoch and expected_revision are u64 fields in the .fbs —
    // values up to 2^64-1 are legal, so they validate as u64, not i64.
    engine_epoch: u64str(fields.engineEpoch),
    expected_revision: u64str(fields.expectedRevision),
    op: fields.op,
  };
  // commands.rs registers lifecycle projects using a top-level container_dir.
  const c = (fields.op as { CreateProjectOp?: CreateProjectOp }).CreateProjectOp
    ?.container_dir;
  const o = (fields.op as { OpenProjectOp?: OpenProjectOp }).OpenProjectOp
    ?.container_dir;
  const dir = c ?? o;
  if (dir) dto.container_dir = dir;
  return dto;
}
