// VoidClient — typed client over the VOID Tauri command surface.
//
// Wraps engine_status / spawn_engine / stop_engine / send_command /
// send_transport / read_view invokes plus the void://control,
// void://telemetry and void://engine-lost events. All persistent edits go
// through sendCommand: the coordinator serializes them, dedups by
// command_id + payload hash, and returns one CommandReceipt. The client
// never mutates document state itself — it tracks the last acknowledged
// revision/epoch only to fill expected_revision on the next command.

import {
  ClockSnapshot,
  CommandReceipt,
  ControlEvent,
  EngineLostEvent,
  EngineStatus,
  MeterFrame,
  PanicAccepted,
  PersistentCommandDto,
  ReadItem,
  ReadRequestDto,
  ReadResponse,
  SaveResultEvent,
  SendableOp,
  TelemetryEvent,
  TransportAck,
  TransportRequestDto,
  ViewKindName,
  commandDto,
  isReceipt,
} from './dto';
import { i64str } from './i64';
import { VoidTransport, UnlistenFn, createTauriTransport } from './transport';

export const EVENT_CONTROL = 'void://control';
export const EVENT_TELEMETRY = 'void://telemetry';
export const EVENT_ENGINE_LOST = 'void://engine-lost';

export type IdGen = () => string;

function defaultIdGen(): string {
  const c = globalThis.crypto as Crypto | undefined;
  if (c?.randomUUID) return c.randomUUID();
  // last-resort: not cryptographically strong — still unique enough for
  // in-session command ids; tests inject their own generator.
  return `id-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 12)}`;
}

export interface VoidClientOptions {
  transport: VoidTransport;
  /** project this client's commands address; '' until set via setProject. */
  projectId?: string;
  ids?: IdGen;
}

export interface SendCommandOptions {
  commandId?: string;
  transactionId?: string;
  /** override expected revision (defaults to last acked revision). */
  expectedRevision?: string;
  /** override epoch (defaults to attached epoch). */
  engineEpoch?: string;
}

type EventHandler<T> = (payload: T) => void;

export class VoidClient {
  private readonly transport: VoidTransport;
  private readonly ids: IdGen;
  private projectId: string;
  private epoch = '0';
  private revision = '0';
  private attached = false;
  private workerId: string | undefined;
  private inFlightCommands = new Map<string, Promise<CommandReceipt>>();
  private listeners: EventHandler<ControlEvent>[] = [];
  private telemetryListeners: EventHandler<TelemetryEvent>[] = [];
  private engineLostListeners: EventHandler<EngineLostEvent>[] = [];
  private unlistens: UnlistenFn[] = [];
  private started = false;

  constructor(opts: VoidClientOptions) {
    this.transport = opts.transport;
    this.ids = opts.ids ?? defaultIdGen;
    this.projectId = opts.projectId ?? '';
  }

  /** Convenience: bind a client to the real Tauri runtime. */
  static tauri(opts: Omit<VoidClientOptions, 'transport'> = {}): VoidClient {
    return new VoidClient({ ...opts, transport: createTauriTransport() });
  }

  // -- lifecycle --------------------------------------------------------------

  get currentProjectId(): string {
    return this.projectId;
  }
  get engineEpoch(): string {
    return this.epoch;
  }
  /** Last coordinator-acknowledged project revision (decimal string). */
  get projectRevision(): string {
    return this.revision;
  }
  get isAttached(): boolean {
    return this.attached;
  }
  get currentWorkerId(): string | undefined {
    return this.workerId;
  }

  setProject(projectId: string): void {
    this.projectId = projectId;
  }

  engineStatus(): Promise<EngineStatus> {
    return this.transport.invoke<EngineStatus>('engine_status');
  }

  async spawnEngine(executable: string): Promise<EngineStatus> {
    const s = await this.transport.invoke<EngineStatus & { attached: true }>(
      'spawn_engine',
      { executable },
    );
    this.attached = true;
    this.workerId = s.worker_id;
    if (s.engine_epoch !== undefined) this.epoch = s.engine_epoch;
    return s;
  }

  async stopEngine(): Promise<void> {
    await this.transport.invoke('stop_engine');
    this.attached = false;
    this.workerId = undefined;
  }

  /**
   * Subscribe to the three UI channels. Idempotent. Control events are
   * fanned out to onControl listeners; receipts also reconcile the local
   * revision tracker. engine-lost marks the client detached and lets the
   * app re-spawn explicitly — never auto-restart silently.
   */
  async start(): Promise<void> {
    if (this.started) return;
    this.started = true;
    this.unlistens.push(
      await this.transport.listen<ControlEvent>(EVENT_CONTROL, (ev) => {
        if (isReceipt(ev)) this.reconcileRevision(ev);
        for (const fn of this.listeners) fn(ev);
      }),
      await this.transport.listen<TelemetryEvent>(EVENT_TELEMETRY, (ev) => {
        for (const fn of this.telemetryListeners) fn(ev);
      }),
      await this.transport.listen<EngineLostEvent>(EVENT_ENGINE_LOST, (ev) => {
        this.attached = false;
        for (const fn of this.engineLostListeners) fn(ev);
      }),
    );
  }

  async dispose(): Promise<void> {
    for (const un of this.unlistens.splice(0)) un();
    this.listeners = [];
    this.telemetryListeners = [];
    this.engineLostListeners = [];
    this.started = false;
  }

  onControl(fn: EventHandler<ControlEvent>): UnlistenFn {
    this.listeners.push(fn);
    return () => {
      this.listeners = this.listeners.filter((f) => f !== fn);
    };
  }
  onTelemetry(fn: EventHandler<TelemetryEvent>): UnlistenFn {
    this.telemetryListeners.push(fn);
    return () => {
      this.telemetryListeners = this.telemetryListeners.filter((f) => f !== fn);
    };
  }
  onEngineLost(fn: EventHandler<EngineLostEvent>): UnlistenFn {
    this.engineLostListeners.push(fn);
    return () => {
      this.engineLostListeners = this.engineLostListeners.filter(
        (f) => f !== fn,
      );
    };
  }

  // -- commands ----------------------------------------------------------------

  /**
   * Send a persistent command. One CommandReceipt per command, surfaced
   * verbatim — DUPLICATE/REJECTED are receipts, not thrown errors, per the
   * contract (only transport-level failures reject the promise).
   *
   * In-flight dedup: a second call reusing a command_id still in flight
   * returns the same pending receipt promise and does not re-send — that
   * is the client-side half of the dedup contract (the coordinator's
   * payload-hash check covers resends after completion).
   */
  async sendCommand(
    op: SendableOp,
    opts: SendCommandOptions = {},
  ): Promise<CommandReceipt> {
    const commandId = opts.commandId ?? this.ids();
    const existing = this.inFlightCommands.get(commandId);
    if (existing) return existing;

    const dto: PersistentCommandDto = commandDto({
      commandId,
      transactionId: opts.transactionId ?? this.ids(),
      projectId: this.projectId,
      engineEpoch: opts.engineEpoch ?? this.epoch,
      expectedRevision: opts.expectedRevision ?? this.revision,
      op,
    });

    const pending = this.transport
      .invoke<CommandReceipt>('send_command', { dto })
      .then((receipt) => {
        this.reconcileRevision(receipt);
        return receipt;
      })
      .finally(() => {
        this.inFlightCommands.delete(commandId);
      });
    this.inFlightCommands.set(commandId, pending);
    return pending;
  }

  /** Raw DTO path for callers that build their own command envelope. */
  async sendCommandDto(dto: PersistentCommandDto): Promise<CommandReceipt> {
    const receipt = await this.transport.invoke<CommandReceipt>(
      'send_command',
      { dto },
    );
    this.reconcileRevision(receipt);
    return receipt;
  }

  private reconcileRevision(r: CommandReceipt): void {
    // APPLIED carries the resulting revision; conflict receipts (STALE_*)
    // carry the current revision — both are authoritative for the next
    // expected_revision. Synthesize nothing when the field is absent.
    if (r.revision !== undefined && r.revision !== '') {
      this.revision = r.revision;
    }
    if (r.engine_epoch !== undefined && r.engine_epoch !== '') {
      this.epoch = r.engine_epoch;
    }
  }

  // -- transport requests -------------------------------------------------------

  /** Scoped non-persistent transport op (PLAY/STOP/SEEK/PANIC/SET_CYCLE/…). */
  async sendTransport(
    op: TransportRequestDto['op'],
    fields: Partial<
      Pick<
        TransportRequestDto,
        'position_ticks' | 'cycle_start_ticks' | 'cycle_end_ticks' | 'value'
      >
    > = {},
    requestId?: string,
  ): Promise<TransportAck | PanicAccepted> {
    const dto: TransportRequestDto = {
      request_id: requestId ?? this.ids(),
      project_id: this.projectId,
      engine_epoch: this.epoch,
      op,
      ...fields,
    };
    return this.transport.invoke<TransportAck | PanicAccepted>(
      'send_transport',
      { dto },
    );
  }

  play(): Promise<TransportAck | PanicAccepted> {
    return this.sendTransport('PLAY');
  }
  stop(): Promise<TransportAck | PanicAccepted> {
    return this.sendTransport('STOP');
  }
  seek(positionTicks: string | number | bigint): Promise<TransportAck | PanicAccepted> {
    return this.sendTransport('SEEK', { position_ticks: i64str(positionTicks) });
  }
  /** All-notes-off + stop. Acknowledged by send — never queued behind edits. */
  panic(): Promise<TransportAck | PanicAccepted> {
    return this.sendTransport('PANIC');
  }
  setCycle(
    startTicks: string | number | bigint,
    endTicks: string | number | bigint,
  ): Promise<TransportAck | PanicAccepted> {
    return this.sendTransport('SET_CYCLE', {
      cycle_start_ticks: i64str(startTicks),
      cycle_end_ticks: i64str(endTicks),
    });
  }

  // -- read views ---------------------------------------------------------------

  /** One bounded page of a view (<=2000 items / <=512KiB server-side). */
  readView(
    req: Omit<ReadRequestDto, 'request_id' | 'project_id'> & {
      projectId?: string;
      /** Pinned camelCase alias (void-studio jobListRequest). */
      includeTerminal?: boolean;
    },
  ): Promise<ReadResponse> {
    const dto: ReadRequestDto = {
      request_id: this.ids(),
      project_id: req.projectId ?? this.projectId,
      view: req.view,
      cursor: req.cursor ?? '',
      limit: req.limit ?? 0,
      track_id: req.track_id ?? '',
      start_ticks: req.start_ticks ?? '-1',
      end_ticks: req.end_ticks ?? '-1',
      include_terminal: req.include_terminal ?? req.includeTerminal ?? false,
    };
    return this.transport.invoke<ReadResponse>('read_view', { dto });
  }

  /**
   * Iterate view pages until `done`, yielding each page. Bounded by
   * maxPages (default 64) so a misbehaving cursor cannot loop forever.
   */
  async *readViewPages(
    req: Omit<ReadRequestDto, 'request_id' | 'project_id' | 'cursor'> & {
      projectId?: string;
      maxPages?: number;
    },
  ): AsyncGenerator<ReadResponse> {
    const maxPages = req.maxPages ?? 64;
    let cursor = '';
    for (let i = 0; i < maxPages; i++) {
      const page = await this.readView({ ...req, cursor });
      yield page;
      if (page.done || !page.next_cursor) return;
      cursor = page.next_cursor;
    }
    throw new Error(`read_view exceeded maxPages=${maxPages}`);
  }

  // -- undo (issued to the engine; no client-side undo stack) --------------------

  undo(transactionId = ''): Promise<CommandReceipt> {
    return this.sendCommand({ UndoOp: { transaction_id: transactionId } });
  }
  redo(transactionId = ''): Promise<CommandReceipt> {
    return this.sendCommand({ RedoOp: { transaction_id: transactionId } });
  }

  // -- convenience persistent ops ------------------------------------------------

  createProject(fields: {
    projectId: string;
    name: string;
    containerDir: string;
    sampleRate?: number;
    initialBpm?: number;
    commandId?: string;
  }): Promise<CommandReceipt> {
    this.projectId = fields.projectId;
    return this.sendCommand(
      {
        CreateProjectOp: {
          name: fields.name,
          container_dir: fields.containerDir,
          sample_rate: fields.sampleRate,
          initial_bpm: fields.initialBpm,
        },
      },
      { commandId: fields.commandId },
    );
  }

  addTrack(fields: {
    trackId: string;
    kind: 'AUDIO' | 'MIDI' | 'INSTRUMENT' | 'BUS';
    name?: string;
    index?: number;
    commandId?: string;
  }): Promise<CommandReceipt> {
    return this.sendCommand(
      {
        AddTrackOp: {
          track_id: fields.trackId,
          kind: fields.kind,
          name: fields.name,
          index: fields.index,
        },
      },
      { commandId: fields.commandId },
    );
  }

  setTrackGain(trackId: string, gainLinear: number, commandId?: string): Promise<CommandReceipt> {
    return this.sendCommand(
      { SetTrackGainOp: { track_id: trackId, gain_linear: gainLinear } },
      { commandId },
    );
  }
  setTrackPan(trackId: string, pan: number, commandId?: string): Promise<CommandReceipt> {
    return this.sendCommand(
      { SetTrackPanOp: { track_id: trackId, pan } },
      { commandId },
    );
  }
  setTrackMute(trackId: string, muted: boolean, commandId?: string): Promise<CommandReceipt> {
    return this.sendCommand(
      { SetTrackMuteOp: { track_id: trackId, muted } },
      { commandId },
    );
  }
  setTrackSolo(trackId: string, soloed: boolean, commandId?: string): Promise<CommandReceipt> {
    return this.sendCommand(
      { SetTrackSoloOp: { track_id: trackId, soloed } },
      { commandId },
    );
  }

  saveProject(reason = '', commandId?: string): Promise<CommandReceipt> {
    return this.sendCommand({ SaveProjectOp: { reason } }, { commandId });
  }
}

// Re-export event DTO unions so consumers can narrow on kind.
export type {
  ClockSnapshot,
  CommandReceipt,
  ControlEvent,
  EngineLostEvent,
  EngineStatus,
  MeterFrame,
  PanicAccepted,
  ReadItem,
  ReadResponse,
  SaveResultEvent,
  TelemetryEvent,
  TransportAck,
  ViewKindName,
};
