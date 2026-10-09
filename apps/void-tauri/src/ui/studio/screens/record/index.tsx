// S05 Record / input and take lanes (Figma 4:408).
//
// Real data: TRACK_LIST (armed-track choice + take lanes), PROJECT_SUMMARY's
// `recording` block (phase / armed tracks / takeId / lastError — emitted by
// EngineSession::recordingSummaryJson), MeterFrame telemetry for the input
// meter, INPUT_DEVICE_LIST for host input enumeration, and the takes
// view-store folders for recorded takes.
//
// rev-2 (protocol minor 1, NEEDS §7–8): arm/monitor/count-in/metronome
// and the take lifecycle are real ops — ArmTrackOp (record_enabled +
// input_device + monitor_mode ride one op), SetCountInOp,
// SetMetronomeOp, StartRecordingOp, StopRecordingOp. Stopping preserves
// the take per the engine contract and review opens only once the
// retained take is visible (UI-T10).

import * as React from 'react';
import { ActionButton, Meter, StatusBadge, TakeLane, TrackHeader, tokens } from 'void-ui';
import {
  createTakeStudioStore,
  makeViewKey,
  parseRecordingSummary,
  recordingUiState,
  createRecordingViewStore,
  useStore,
  useStudio,
  useStudioActions,
  loadViewPage,
  studioStore,
  type ReadViewEntry,
} from 'void-studio';
import { getClient } from '../../../client';
import { useProjectSummary, useStudioCompactContext } from '../../useStudioData';

function navigate(id: string) {
  window.location.hash = `/${id}`;
}

const takeStudio = createTakeStudioStore();
const recordingView = createRecordingViewStore();

const label: React.CSSProperties = {
  fontSize: 11,
  fontWeight: 500,
  letterSpacing: '0.08em',
  textTransform: 'uppercase',
  color: tokens.subtle,
  margin: '0 0 10px',
};

const card: React.CSSProperties = {
  padding: tokens.space16,
  borderRadius: tokens.radius8,
  border: `1px solid ${tokens.line}`,
  background: tokens.surface,
};

interface TrackItem {
  trackId: string;
  kind: string;
  name: string;
  index: number;
}

function trackItems(entry: ReadViewEntry | undefined): TrackItem[] {
  if (!entry?.items) return [];
  const out: TrackItem[] = [];
  for (const it of entry.items) {
    try {
      const p = JSON.parse(it.summary_json) as Record<string, unknown>;
      const trackId = typeof p.trackId === 'string' ? p.trackId : '';
      if (trackId === '') continue;
      out.push({
        trackId,
        kind: typeof p.kind === 'string' ? p.kind : '',
        name: typeof p.name === 'string' ? p.name : '',
        index: typeof p.index === 'number' ? p.index : 0,
      });
    } catch {
      // malformed item — skip it, never invent a row
    }
  }
  return out;
}

const STATE_BADGE: Record<
  string,
  { status: 'unavailable' | 'ready' | 'armed' | 'recording' | 'processing' | 'error' | 'saved' | 'queued'; text: string }
> = {
  unavailable: { status: 'unavailable', text: 'No engine' },
  ready: { status: 'ready', text: 'Ready when you are.' },
  armed: { status: 'armed', text: 'Armed' },
  countIn: { status: 'queued', text: 'Count-in' },
  recording: { status: 'recording', text: 'Recording…' },
  stopping: { status: 'processing', text: 'Stopping — preserving take' },
  review: { status: 'saved', text: 'Take kept — review' },
  failed: { status: 'error', text: 'Recording failed' },
};

export default function RecordScreen() {
  const compact = useStudioCompactContext();
  const projectId = useStudio((s) => s.projectId);
  const revision = useStudio((s) => s.revision);
  const attached = useStudio((s) => s.engine.attached);
  const selection = useStudio((s) => s.selection);
  const trackEntry = useStudio((s) => s.views[makeViewKey('TRACK_LIST')]);
  const clock = useStudio((s) => s.telemetry.clock);
  const meters = useStudio((s) => s.telemetry.meters);
  const summary = useProjectSummary();
  const selectTrack = useStudioActions().selectTrack;
  const folders = useStore(takeStudio, (s) => s.folders);
  const monitorMode = useStore(recordingView, (s) => s.monitorMode);
  const countInBars = useStore(recordingView, (s) => s.countInBars);
  const metronome = useStore(recordingView, (s) => s.metronome);
  const reviewTakeId = useStore(recordingView, (s) => s.reviewTakeId);
  const rActions = useStore(recordingView, (s) => s.actions);
  const [busyStop, setBusyStop] = React.useState(false);
  const [busyStart, setBusyStart] = React.useState(false);
  const [stopError, setStopError] = React.useState('');
  const [ctlError, setCtlError] = React.useState('');
  const [inputDevices, setInputDevices] = React.useState<
    { deviceId: string; name: string }[]
  >([]);

  const recording = parseRecordingSummary(summary.recording ?? undefined);
  const uiState = recordingUiState({ engineAttached: attached, summary: recording, reviewing: reviewTakeId !== null });
  const badge = STATE_BADGE[uiState] ?? STATE_BADGE.ready;

  // Keep TRACK_LIST fresh while this screen is open.
  React.useEffect(() => {
    if (!projectId || !attached) return;
    void loadViewPage(studioStore, getClient(), 'TRACK_LIST').catch(() => undefined);
  }, [projectId, attached, revision]);

  // rev-2 INPUT_DEVICE_LIST (NEEDS §8): engine-reported host inputs.
  React.useEffect(() => {
    if (!projectId || !attached) {
      setInputDevices([]);
      return;
    }
    let live = true;
    const read = async () => {
      const rows: { deviceId: string; name: string }[] = [];
      try {
        for await (const page of getClient().readViewPages({ view: 'INPUT_DEVICE_LIST' })) {
          for (const it of page.items ?? []) {
            try {
              const p = JSON.parse(it.summary_json) as Record<string, unknown>;
              const deviceId =
                typeof p.device_id === 'string'
                  ? p.device_id
                  : typeof p.deviceId === 'string'
                    ? p.deviceId
                    : typeof p.id === 'string'
                      ? p.id
                      : it.object_id;
              const name = typeof p.name === 'string' ? p.name : deviceId;
              if (deviceId !== '') rows.push({ deviceId, name });
            } catch {
              /* malformed row — drop, never fake a device */
            }
          }
        }
      } catch {
        /* older coordinator — the column stays honest below */
      }
      if (live) setInputDevices(rows);
    };
    void read();
    return () => {
      live = false;
    };
  }, [projectId, attached]);

  const tracks = trackItems(trackEntry);
  const audioTracks = tracks.filter((x) => x.kind === 'AUDIO' || x.kind === 'INSTRUMENT');
  const armedIds = new Set(recording?.armedTracks ?? []);
  const focusTrackId = selection.trackId ?? recording?.armedTracks[0] ?? audioTracks[0]?.trackId ?? null;
  const meter = focusTrackId ? meters[focusTrackId] : undefined;
  const wasRecording = React.useRef(false);
  const lastTakeId = React.useRef<string | null>(null);

  // UI-T10: when the engine finishes stopping a take, open review on the
  // retained take (never before — the journal owns the truth).
  React.useEffect(() => {
    if (recording?.phase === 'recording' && recording.takeId) {
      wasRecording.current = true;
      lastTakeId.current = recording.takeId;
    } else if (wasRecording.current && recording?.phase === 'idle') {
      wasRecording.current = false;
      rActions.openReview(lastTakeId.current);
    }
  }, [recording?.phase, recording?.takeId, rActions]);

  // rev-2 ops — each returns the receipt; the engine summary/telemetry
  // remains the truth (a REJECTED receipt never repaints a badge).
  const sendCtl = (send: () => Promise<unknown>) =>
    send().catch((e) => setCtlError(String(e instanceof Error ? e.message : e)));

  const stopRecording = async () => {
    setBusyStop(true);
    setStopError('');
    try {
      await getClient().sendCommand({ StopRecordingOp: { discard: false } });
    } catch (e) {
      setStopError(String(e instanceof Error ? e.message : e));
    } finally {
      setBusyStop(false);
    }
  };

  const startRecording = async () => {
    setBusyStart(true);
    setStopError('');
    try {
      await getClient().sendCommand({ StartRecordingOp: { take_id: '' } });
    } catch (e) {
      setStopError(String(e instanceof Error ? e.message : e));
    } finally {
      setBusyStart(false);
    }
  };

  const setArm = (track: TrackItem, recordEnabled: boolean) =>
    sendCtl(() =>
      getClient().sendCommand({
        ArmTrackOp: {
          track_id: track.trackId,
          record_enabled: recordEnabled,
          is_midi: track.kind === 'INSTRUMENT',
        },
      }),
    );

  const setMonitor = (mode: 'off' | 'automatic' | 'on') => {
    if (!focusTrackId) return;
    rActions.setMonitorMode(mode);
    // Monitor rides ArmTrackOp.monitor_mode — record_enabled preserves
    // the engine-reported arm state so a monitor toggle never arms.
    void sendCtl(() =>
      getClient().sendCommand({
        ArmTrackOp: {
          track_id: focusTrackId,
          record_enabled: armedIds.has(focusTrackId),
          monitor_mode: mode.toUpperCase() as 'OFF' | 'AUTOMATIC' | 'ON',
        },
      }),
    );
  };

  const setCountIn = (bars: number) => {
    rActions.setCountInBars(bars);
    void sendCtl(() =>
      getClient().sendCommand({
        SetCountInOp: bars === 0 ? { mode: 'off' } : { mode: 'bars', bars },
      }),
    );
  };

  const setMetronome = (enabled: boolean) => {
    rActions.setMetronome(enabled);
    void sendCtl(() => getClient().sendCommand({ SetMetronomeOp: { enabled } }));
  };

  const pickInput = (deviceId: string) => {
    if (!focusTrackId) return;
    void sendCtl(() =>
      getClient().sendCommand({
        ArmTrackOp: {
          track_id: focusTrackId,
          record_enabled: armedIds.has(focusTrackId),
          input_device: deviceId,
        },
      }),
    );
  };

  const focusFolder = focusTrackId
    ? Object.values(folders).find((f) => f.trackId === focusTrackId)
    : undefined;
  const focusTrack = tracks.find((x) => x.trackId === focusTrackId);
  const ctlReady = attached && focusTrackId !== null;
  const loopOn = clock ? BigInt(clock.loop_end_ticks) > BigInt(clock.loop_start_ticks) : false;

  const inputColumn = (
    <div style={{ display: 'flex', flexDirection: 'column', gap: tokens.space16, width: compact ? '100%' : 280, flexShrink: 0 }}>
      <section aria-label="Recording input" style={card}>
        <p style={label}>Recording input</p>
        <div style={{ display: 'flex', flexDirection: 'column', gap: tokens.space12 }}>
          <div>
            <span style={{ fontSize: 11, color: tokens.subtle }}>Input</span>
            {inputDevices.length > 0 ? (
              <select
                aria-label="Input device"
                disabled={!ctlReady}
                title={ctlReady ? undefined : 'engine detached or no track in focus'}
                onChange={(ev) => pickInput(ev.target.value)}
                style={{
                  marginTop: 4,
                  width: '100%',
                  fontSize: 13,
                  color: tokens.text,
                  fontFamily: tokens.mono,
                  background: tokens.raised,
                  border: `1px solid ${tokens.line}`,
                  borderRadius: tokens.radius8,
                  padding: '4px 6px',
                }}
              >
                <option value="">Engine default input</option>
                {inputDevices.map((d) => (
                  <option key={d.deviceId} value={d.deviceId}>
                    {d.name}
                  </option>
                ))}
              </select>
            ) : (
              <p style={{ margin: '2px 0 0', fontSize: 13, color: tokens.text, fontFamily: tokens.mono }}>
                engine default
              </p>
            )}
            <p style={{ margin: '2px 0 0', fontSize: 11, color: tokens.subtle }}>
              {inputDevices.length > 0
                ? 'selecting an input re-arms the focus track on that device (ArmTrackOp.input_device)'
                : 'INPUT_DEVICE_LIST reported no host inputs — engine default is used'}
            </p>
          </div>
          <div>
            <span style={{ fontSize: 11, color: tokens.subtle }}>Input level</span>
            <Meter
              level={meter ? Math.max(meter.peak_l, meter.peak_r) : 0}
              rms={meter ? Math.max(meter.rms_l, meter.rms_r) : undefined}
              label="input"
              orientation="horizontal"
              idle={!meter}
            />
            <p style={{ margin: '4px 0 0', fontSize: 11, color: tokens.subtle }}>
              {meter ? 'live from engine MeterFrame' : 'no meter frames — engine is not streaming meters'}
            </p>
          </div>
          <div>
            <span style={{ fontSize: 11, color: tokens.subtle }}>Monitor</span>
            <div role="group" aria-label="Monitor" style={{ display: 'flex', gap: 4, marginTop: 4 }}>
              {(['off', 'automatic', 'on'] as const).map((m) => (
                <ActionButton
                  key={m}
                  size="sm"
                  variant={monitorMode === m ? 'secondary' : 'ghost'}
                  disabled={!ctlReady}
                  title={
                    ctlReady
                      ? 'ArmTrackOp.monitor_mode on the focus track'
                      : 'engine detached or no track in focus'
                  }
                  onClick={() => setMonitor(m)}
                >
                  {m === 'automatic' ? 'Automatic' : m === 'off' ? 'Off' : 'On'}
                </ActionButton>
              ))}
            </div>
            <p style={{ margin: '4px 0 0', fontSize: 11, color: tokens.subtle }}>
              monitor rides ArmTrackOp on the focus track — the engine confirms it
            </p>
          </div>
          <div style={{ display: 'flex', gap: tokens.space12, flexWrap: 'wrap' }}>
            <div>
              <span style={{ fontSize: 11, color: tokens.subtle }}>Count-in</span>
              <div role="group" aria-label="Count-in" style={{ display: 'flex', gap: 4, marginTop: 4 }}>
                {[0, 1, 2, 4].map((b) => (
                  <ActionButton
                    key={b}
                    size="sm"
                    variant={countInBars === b ? 'secondary' : 'ghost'}
                    disabled={!attached}
                    title={attached ? 'SetCountInOp' : 'engine detached'}
                    onClick={() => setCountIn(b)}
                  >
                    {b === 0 ? 'Off' : b}
                  </ActionButton>
                ))}
              </div>
            </div>
            <div>
              <span style={{ fontSize: 11, color: tokens.subtle }}>Metronome</span>
              <div role="group" aria-label="Metronome" style={{ display: 'flex', gap: 4, marginTop: 4 }}>
                <ActionButton
                  size="sm"
                  variant={metronome ? 'secondary' : 'ghost'}
                  disabled={!attached}
                  title={attached ? 'SetMetronomeOp' : 'engine detached'}
                  onClick={() => setMetronome(!metronome)}
                >
                  {metronome ? 'On' : 'Off'}
                </ActionButton>
              </div>
            </div>
            <div>
              <span style={{ fontSize: 11, color: tokens.subtle }}>Cycle</span>
              <p style={{ margin: '2px 0 0', fontSize: 13, color: tokens.text }}>{loopOn ? 'On' : 'Off'}</p>
              <p style={{ margin: '2px 0 0', fontSize: 11, color: tokens.subtle }}>
                {clock ? 'from engine loop bounds' : 'engine clock not streaming'}
              </p>
            </div>
          </div>
          <ActionButton variant="ghost" size="sm" onClick={() => navigate('setup')}>
            Audio settings
          </ActionButton>
        </div>
      </section>

      <section aria-label="Arm" style={card}>
        <p style={label}>Tracks</p>
        {audioTracks.length === 0 ? (
          <p style={{ margin: 0, fontSize: 12, color: tokens.muted }}>
            No audio or instrument tracks yet — add one from the arrange screen.
          </p>
        ) : (
          <div role="list" style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            {audioTracks.map((tr) => (
              <div key={tr.trackId} style={{ display: 'flex', alignItems: 'center', gap: 6 }}>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <TrackHeader
                    trackId={tr.trackId}
                    name={tr.name}
                    kind={tr.kind}
                    selected={selection.trackId === tr.trackId}
                    onSelect={(tid) => selectTrack(tid)}
                  />
                </div>
                <StatusBadge
                  status={armedIds.has(tr.trackId) ? 'armed' : 'queued'}
                  label={armedIds.has(tr.trackId) ? 'Armed' : '—'}
                />
                <ActionButton
                  size="sm"
                  variant={armedIds.has(tr.trackId) ? 'secondary' : 'ghost'}
                  disabled={!attached}
                  title={
                    attached
                      ? armedIds.has(tr.trackId)
                        ? 'ArmTrackOp{record_enabled:false} — disarm'
                        : 'ArmTrackOp{record_enabled:true} — arm for recording'
                      : 'engine detached'
                  }
                  onClick={() => void setArm(tr, !armedIds.has(tr.trackId))}
                >
                  {armedIds.has(tr.trackId) ? 'Disarm' : 'Arm'}
                </ActionButton>
              </div>
            ))}
          </div>
        )}
        <p style={{ margin: `${tokens.space8} 0 0`, fontSize: 11, color: tokens.subtle }}>
          Arm sends ArmTrackOp — the badge stays engine-reported, so a rejected arm never repaints it.
        </p>
      </section>
    </div>
  );

  const laneHeight = 44;
  const takeLaneArea = focusFolder && focusFolder.takes.length > 0 ? (
    <div style={{ position: 'relative', borderTop: `1px solid ${tokens.line}`, marginTop: tokens.space12 }}>
      {focusFolder.takes.map((tk, i) => (
        <TakeLane
          key={tk.takeId}
          takeId={tk.takeId}
          laneIndex={tk.laneIndex}
          name={`Take ${i + 1}`}
          top={i * laneHeight}
          height={laneHeight - 4}
          startFrac={0}
          widthFrac={1}
          incomplete={!tk.complete}
          underCursor={reviewTakeId === tk.takeId}
          onPick={(id) => rActions.openReview(id)}
        />
      ))}
    </div>
  ) : (
    <div
      role="status"
      style={{
        padding: tokens.space24,
        borderRadius: tokens.radius8,
        border: `1px dashed ${tokens.line}`,
        color: tokens.muted,
        fontSize: 13,
        marginTop: tokens.space12,
      }}
    >
      {recording?.takeId && (recording.phase === 'recording' || recording.phase === 'stopping')
        ? `Recording ${recording.takeId} — the take lands here after stop preserves it.`
        : 'No recorded takes on this track yet.'}
    </div>
  );

  return (
    <div
      style={{
        display: 'flex',
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        flexDirection: compact ? 'column' : 'row',
        gap: tokens.space16,
        padding: tokens.space16,
        overflowY: 'auto',
      }}
    >
      {inputColumn}
      <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: tokens.space16 }}>
        <section aria-label="Recording" style={{ ...card, display: 'flex', alignItems: 'center', gap: tokens.space16 }}>
          <div style={{ flex: 1, minWidth: 0 }}>
            <p style={label}>Recording</p>
            <h1 style={{ margin: 0, fontSize: 20, fontWeight: 500, color: tokens.text, fontFamily: tokens.fontDisplay }}>
              {recording?.takeId ? `Recording / ${recording.takeId}` : 'Ready when you are.'}
            </h1>
            <p style={{ margin: '4px 0 0', fontSize: 11, color: tokens.subtle }}>
              Punch-in armed · local recording · originals preserved
            </p>
          </div>
          <StatusBadge status={badge.status} label={badge.text} />
          <ActionButton
            variant="primary"
            loading={busyStart}
            disabled={!attached || armedIds.size === 0 || uiState === 'recording' || busyStart || busyStop}
            title={
              !attached
                ? 'engine detached'
                : armedIds.size === 0
                  ? 'arm a track first'
                  : 'StartRecordingOp — engine mints the take id'
            }
            onClick={() => void startRecording()}
          >
            Record
          </ActionButton>
          <ActionButton
            variant="danger"
            loading={busyStop}
            disabled={uiState !== 'recording' || busyStop}
            title={
              uiState === 'recording'
                ? 'StopRecordingOp{discard:false} — stop and preserve the take'
                : 'not recording'
            }
            onClick={() => void stopRecording()}
          >
            Stop recording
          </ActionButton>
        </section>

        {recording?.lastError || stopError || ctlError ? (
          <div role="alert" style={{ ...card, borderColor: tokens.danger }}>
            <p style={{ margin: 0, fontSize: 12, color: tokens.danger }}>
              {recording?.lastError ?? ''}
              {stopError ? ` ${stopError}` : ''}
              {ctlError ? ` ${ctlError}` : ''}
            </p>
          </div>
        ) : null}

        <section aria-label="Take lanes" style={{ ...card, flex: 1 }}>
          <div style={{ display: 'flex', alignItems: 'center', gap: tokens.space12 }}>
            <p style={{ ...label, margin: 0 }}>Takes{focusTrack ? ` on ${focusTrack.name}` : ''}</p>
            <div style={{ flex: 1 }} />
            <ActionButton
              variant="ghost"
              size="sm"
              disabled={reviewTakeId === null}
              onClick={() => rActions.openReview(null)}
            >
              Close review
            </ActionButton>
          </div>
          {takeLaneArea}
          {reviewTakeId !== null ? (
            <div style={{ marginTop: tokens.space12, padding: tokens.space12, borderRadius: tokens.radius8, background: tokens.raised }}>
              <p style={{ margin: 0, fontSize: 12, color: tokens.text }}>
                Reviewing take {reviewTakeId}
                {focusFolder?.takes.find((x) => x.takeId === reviewTakeId)?.complete === false
                  ? ' — interrupted take, retained for recovery'
                  : ''}
              </p>
              <p style={{ margin: '4px 0 0', fontSize: 11, color: tokens.subtle }}>
                Recordings remain original assets. Comping chooses regions; it never rewrites your source.
              </p>
            </div>
          ) : null}
          <p style={{ margin: `${tokens.space12} 0 0`, fontSize: 11, color: tokens.subtle }}>
            Every take is kept. Stopping preserves the take and opens review — no auto-publish.
          </p>
        </section>
      </div>
    </div>
  );
}
