// VOID Studio dashboard — the Tauri shell's main surface.
//
// Everything here is wired to the real command surface through VoidClient:
//   engine lifecycle  -> spawn_engine / stop_engine / engine_status
//   commands          -> send_command (PersistentCommand) + CommandReceipt
//   transport         -> send_transport (PLAY/STOP/SEEK/PANIC)
//   telemetry         -> void://telemetry  (ClockSnapshot + MeterFrame)
//   read views        -> read_view pages merged into bounded store entries
//   engine-lost       -> void://engine-lost banner (survives worker death)
//
// No fake audio or meters: the meter strip renders exactly what MeterFrames
// deliver; when nothing plays it shows the honest idle/zero state.

import React from 'react';
import {
  Button,
  Fader,
  Knob,
  Meter,
  Panel,
  TimelineRuler,
  TrackHeader,
  formatBarBeat,
  injectVoidStyles,
  tokens,
} from 'void-ui';
import {
  makeViewKey,
  loadViewPage,
  studioStore,
  useStudio,
  useStudioActions,
} from 'void-studio';
import type {
  CommandReceipt,
  MeterFrame,
  ViewKindName,
} from 'void-client';
import { ensureClientStarted, getClient } from './client';

const VIEWS: ViewKindName[] = [
  'PROJECT_SUMMARY',
  'TRACK_LIST',
  'CLIP_LIST',
  'NOTE_RANGE',
  'ASSET_LIST',
  'PLUGIN_LIST',
  'RECEIPT_LIST',
];

const row: React.CSSProperties = { display: 'flex', alignItems: 'center', gap: 8 };
const label: React.CSSProperties = { fontSize: 10, color: tokens.textMuted, fontFamily: tokens.mono };
const value: React.CSSProperties = { fontSize: 12, color: tokens.text, fontFamily: tokens.mono };
const inputStyle: React.CSSProperties = {
  background: tokens.bg,
  border: `1px solid ${tokens.border}`,
  borderRadius: tokens.radius,
  color: tokens.text,
  fontFamily: tokens.mono,
  fontSize: 12,
  padding: '6px 8px',
};

// ---------------------------------------------------------------------------

function EnginePanel() {
  const engine = useStudio((s) => s.engine);
  const setEngine = useStudioActions().setEngine;
  const [exe, setExe] = React.useState('./target/debug/void-mock-worker');
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState('');

  const refresh = React.useCallback(async () => {
    try {
      const s = await getClient().engineStatus();
      setEngine({ attached: s.attached, workerId: s.worker_id, epoch: s.engine_epoch, state: s.state });
    } catch (e) {
      setError(String(e));
    }
  }, [setEngine]);

  const spawn = async () => {
    setBusy(true);
    setError('');
    try {
      const s = await getClient().spawnEngine(exe);
      setEngine({ attached: true, workerId: s.worker_id, epoch: s.engine_epoch, state: s.state });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const stop = async () => {
    setBusy(true);
    setError('');
    try {
      await getClient().stopEngine();
      setEngine({ attached: false });
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Panel title="Engine" actions={<Button onClick={refresh}>Refresh</Button>}>
      <div style={{ ...row, marginBottom: 8 }}>
        <span style={label}>status</span>
        <span style={{ ...value, color: engine.attached ? tokens.ok : tokens.textMuted }}>
          {engine.attached ? 'attached' : 'detached'}
        </span>
        {engine.state ? <span style={label}>({engine.state})</span> : null}
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '4px 12px', marginBottom: 12 }}>
        <span style={label}>worker_id</span>
        <span style={value}>{engine.workerId ?? '—'}</span>
        <span style={label}>engine_epoch</span>
        <span style={value}>{engine.epoch ?? '—'}</span>
      </div>
      <div style={{ ...row, flexWrap: 'wrap' }}>
        <input
          aria-label="Engine executable"
          value={exe}
          onChange={(e) => setExe(e.target.value)}
          style={{ ...inputStyle, flex: 1, minWidth: 220 }}
          placeholder="/path/to/engine worker"
        />
        <Button variant="primary" loading={busy} disabled={engine.attached} onClick={spawn}>
          Spawn engine
        </Button>
        <Button variant="danger" loading={busy} disabled={!engine.attached} onClick={stop}>
          Stop
        </Button>
      </div>
      {error ? (
        <p role="alert" style={{ color: tokens.danger, fontSize: 11, fontFamily: tokens.mono }}>
          {error}
        </p>
      ) : null}
    </Panel>
  );
}

// ---------------------------------------------------------------------------

function CommandPanel() {
  const projectId = useStudio((s) => s.projectId);
  const revision = useStudio((s) => s.revision);
  const attached = useStudio((s) => s.engine.attached);
  const setProject = useStudioActions().setProject;
  const [name, setName] = React.useState('demo-session');
  const [dir, setDir] = React.useState('./projects/demo-session');
  const [busy, setBusy] = React.useState(false);
  const [receipt, setReceipt] = React.useState<CommandReceipt | null>(null);
  const [error, setError] = React.useState('');

  const create = async () => {
    setBusy(true);
    setError('');
    try {
      const pid = crypto.randomUUID(); // project ids must be UUIDs (is_valid_id)
      const r = await getClient().createProject({ projectId: pid, name, containerDir: dir });
      setReceipt(r);
      if (r.status === 'APPLIED') setProject(pid, r.revision);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const undo = async () => {
    try {
      setReceipt(await getClient().undo());
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <Panel
      title="Commands"
      actions={
        <Button onClick={undo} disabled={!projectId}>
          Undo (engine op)
        </Button>
      }
    >
      <div style={{ ...row, flexWrap: 'wrap', marginBottom: 8 }}>
        <input
          aria-label="Project name"
          value={name}
          onChange={(e) => setName(e.target.value)}
          style={inputStyle}
          placeholder="project name"
        />
        <input
          aria-label="Container directory"
          value={dir}
          onChange={(e) => setDir(e.target.value)}
          style={{ ...inputStyle, flex: 1, minWidth: 200 }}
          placeholder="container dir"
        />
        <Button variant="primary" loading={busy} disabled={!attached} onClick={create}>
          CreateProject
        </Button>
      </div>
      <div style={{ display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '4px 12px' }}>
        <span style={label}>project_id</span>
        <span style={value}>{projectId || '— (none open)'}</span>
        <span style={label}>revision</span>
        <span style={value}>{revision}</span>
      </div>
      {receipt ? (
        <pre
          aria-label="Command receipt"
          style={{
            marginTop: 10,
            padding: 8,
            background: tokens.bg,
            border: `1px solid ${receipt.status === 'APPLIED' ? tokens.ok : tokens.danger}`,
            borderRadius: tokens.radius,
            fontSize: 11,
            fontFamily: tokens.mono,
            color: tokens.textSecondary,
            overflow: 'auto',
          }}
        >
          {JSON.stringify(receipt, null, 2)}
        </pre>
      ) : null}
      {error ? (
        <p role="alert" style={{ color: tokens.danger, fontSize: 11, fontFamily: tokens.mono }}>
          {error}
        </p>
      ) : null}
    </Panel>
  );
}

// ---------------------------------------------------------------------------

function TransportPanel() {
  const clock = useStudio((s) => s.telemetry.clock);
  const viewport = useStudio((s) => s.viewport);
  const zoom = useStudio((s) => s.zoom);
  const attached = useStudio((s) => s.engine.attached);
  const [ack, setAck] = React.useState('');

  const send = (fn: () => Promise<unknown>) =>
    fn()
      .then((a) => setAck(JSON.stringify(a)))
      .catch((e) => setAck(String(e)));

  const timeline = clock ? Number(clock.timeline_sample) : 0;
  // A truthful playhead needs a tick position; the wire delivers timeline
  // samples. Show samples honestly rather than fabricating a tick position.
  const transport = clock?.transport ?? 'STOPPED';

  return (
    <Panel title="Transport">
      <div style={{ ...row, marginBottom: 8 }}>
        <Button variant="primary" disabled={!attached} onClick={() => send(() => getClient().play())}>
          ▶ Play
        </Button>
        <Button disabled={!attached} onClick={() => send(() => getClient().stop())}>
          ■ Stop
        </Button>
        <Button
          variant="danger"
          disabled={!attached}
          onClick={() => send(() => getClient().panic())}
          aria-label="Panic — all notes off"
        >
          PANIC
        </Button>
        <span style={{ ...label, marginLeft: 'auto' }}>state</span>
        <span
          style={{
            ...value,
            color: transport === 'PLAYING' ? tokens.ok : transport === 'RECORDING' ? tokens.danger : tokens.text,
          }}
          role="status"
          aria-live="polite"
        >
          {transport}
        </span>
      </div>
      <TimelineRuler
        startTicks={viewport.startTicks}
        ticksPerPx={zoom.ticksPerPixel}
        widthPx={560}
        onSeek={attached ? (t) => send(() => getClient().seek(t)) : undefined}
        aria-label="Timeline — click or use arrow keys to seek"
      />
      <div style={{ display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '4px 12px', marginTop: 8 }}>
        <span style={label}>timeline_sample</span>
        <span style={value}>{clock ? clock.timeline_sample : '—'}</span>
        <span style={label}>device_sample</span>
        <span style={value}>{clock ? clock.device_sample_counter : '—'}</span>
        <span style={label}>sample_rate</span>
        <span style={value}>{clock ? clock.sample_rate : '—'}</span>
        <span style={label}>loop</span>
        <span style={value}>
          {clock ? `${formatBarBeat(clock.loop_start_ticks)} → ${formatBarBeat(clock.loop_end_ticks)}` : '—'}
        </span>
        <span style={label}>host_clock_ns</span>
        <span style={value}>{clock ? clock.host_clock_ns : '—'}</span>
        <span style={label}>seq</span>
        <span style={value}>{clock ? clock.sequence : '—'}</span>
      </div>
      {ack ? (
        <div role="status" style={{ marginTop: 6, fontSize: 10, fontFamily: tokens.mono, color: tokens.textMuted }}>
          last ack: {ack}
        </div>
      ) : null}
    </Panel>
  );
}

// ---------------------------------------------------------------------------

function MeterStrip() {
  const meters = useStudio((s) => s.telemetry.meters);
  const attached = useStudio((s) => s.engine.attached);
  const entries = Object.values(meters) as MeterFrame[];

  return (
    <Panel title="Meters">
      {!attached ? (
        <p style={{ ...label }}>engine detached — no live meter frames</p>
      ) : entries.length === 0 ? (
        <p style={label}>
          no MeterFrames received yet — meters render telemetry only; zero/idle is expected until
          the engine reports levels
        </p>
      ) : (
        <div style={{ display: 'flex', gap: 16, flexWrap: 'wrap' }}>
          {entries.map((m) => (
            <div key={m.track_id} style={{ display: 'flex', alignItems: 'flex-end', gap: 4 }}>
              <Meter
                label={`Track ${m.track_id} left`}
                level={m.peak_l}
                rms={m.rms_l}
                size={80}
                idle={false}
              />
              <Meter
                label={`Track ${m.track_id} right`}
                level={m.peak_r}
                rms={m.rms_r}
                size={80}
                idle={false}
              />
              <span style={{ ...label, writingMode: 'vertical-rl' }}>{m.track_id}</span>
            </div>
          ))}
        </div>
      )}
    </Panel>
  );
}

// ---------------------------------------------------------------------------

function TrackListPanel() {
  const views = useStudio((s) => s.views);
  const selection = useStudio((s) => s.selection);
  const actions = useStudioActions();
  const attached = useStudio((s) => s.engine.attached);
  const [busy, setBusy] = React.useState(false);
  const [err, setErr] = React.useState('');

  const key = makeViewKey('TRACK_LIST');
  const entry = views[key];

  const load = async () => {
    setBusy(true);
    setErr('');
    try {
      await loadViewPage(studioStore, getClient(), 'TRACK_LIST');
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  const tracks = React.useMemo(() => {
    if (!entry) return [];
    return entry.items.map((it) => {
      try {
        const v = JSON.parse(it.summary_json) as Record<string, unknown>;
        return {
          objectId: it.object_id,
          id: String(v.track_id ?? v.id ?? it.object_id),
          name: String(v.name ?? v.track_id ?? it.object_id),
          kind: typeof v.kind === 'string' ? v.kind : undefined,
          muted: v.muted === true,
          soloed: v.soloed === true,
          gain: typeof v.gain_linear === 'number' ? v.gain_linear : undefined,
          pan: typeof v.pan === 'number' ? v.pan : undefined,
          disabled: v.disabled === true,
        };
      } catch {
        return { objectId: it.object_id, id: it.object_id, name: it.object_id };
      }
    });
  }, [entry]);

  const toggle = (fn: (trackId: string, on: boolean) => Promise<CommandReceipt>) => {
    return (trackId: string, on: boolean) => {
      fn(trackId, on).catch((e) => setErr(String(e)));
    };
  };

  return (
    <Panel
      title="Tracks"
      actions={
        <Button loading={busy} disabled={!attached} onClick={load}>
          {entry ? 'Re-read page' : 'Read TRACK_LIST'}
        </Button>
      }
    >
      {!entry ? (
        <p style={label}>no TRACK_LIST page read yet — track data stays read-view-driven</p>
      ) : tracks.length === 0 ? (
        <p style={label}>
          TRACK_LIST page: 0 items (rev {entry.revision}
          {entry.truncated ? ', truncated at page bound' : ''})
        </p>
      ) : (
        <div role="list" aria-label="Track list">
          {tracks.map((t) => (
            <div role="listitem" key={t.objectId}>
              <TrackHeader
                trackId={t.id}
                name={t.name}
                kind={t.kind}
                muted={t.muted}
                soloed={t.soloed}
                gain={t.gain}
                pan={t.pan}
                disabled={t.disabled}
                selected={selection.trackId === t.id}
                onSelect={(id) => actions.selectTrack(id)}
                onToggleMute={toggle((id, on) => getClient().setTrackMute(id, on))}
                onToggleSolo={toggle((id, on) => getClient().setTrackSolo(id, on))}
              />
            </div>
          ))}
          {entry.truncated ? (
            <p style={{ ...label, color: tokens.warn }}>
              view truncated at the 2000-item bound — page through with the read inspector
            </p>
          ) : null}
        </div>
      )}
      {err ? (
        <p role="alert" style={{ color: tokens.danger, fontSize: 11, fontFamily: tokens.mono }}>
          {err}
        </p>
      ) : null}
    </Panel>
  );
}

// ---------------------------------------------------------------------------

function ReadViewPanel() {
  const views = useStudio((s) => s.views);
  const attached = useStudio((s) => s.engine.attached);
  const [view, setView] = React.useState<ViewKindName>('PROJECT_SUMMARY');
  const [busy, setBusy] = React.useState(false);
  const [err, setErr] = React.useState('');

  const key = makeViewKey(view);
  const entry = views[key];

  const load = async (cursor?: string) => {
    setBusy(true);
    setErr('');
    try {
      await loadViewPage(studioStore, getClient(), view, { cursor });
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Panel
      title="Read inspector"
      actions={
        <>
          <select
            aria-label="View kind"
            value={view}
            onChange={(e) => setView(e.target.value as ViewKindName)}
            style={{ ...inputStyle, padding: '4px 6px' }}
          >
            {VIEWS.map((v) => (
              <option key={v} value={v}>
                {v}
              </option>
            ))}
          </select>
          <Button loading={busy} disabled={!attached} onClick={() => load('')}>
            Read page
          </Button>
          <Button disabled={!attached || !entry || entry.done || busy} onClick={() => load(entry?.nextCursor ?? '')}>
            Next page
          </Button>
        </>
      }
    >
      {!entry ? (
        <p style={label}>no page read for {view}</p>
      ) : (
        <>
          <p style={label}>
            {view}: {entry.items.length} items · revision {entry.revision} ·{' '}
            {entry.done ? 'exhausted' : `cursor ${entry.nextCursor || '∅'}`}
            {entry.truncated ? ' · truncated at bound' : ''}
          </p>
          <ul
            style={{
              margin: '8px 0 0',
              padding: 0,
              listStyle: 'none',
              maxHeight: 220,
              overflow: 'auto',
              fontFamily: tokens.mono,
              fontSize: 11,
            }}
          >
            {entry.items.map((it, i) => (
              <li
                key={`${it.object_id}-${i}`}
                style={{
                  padding: '4px 6px',
                  borderBottom: `1px solid ${tokens.border}`,
                  color: tokens.textSecondary,
                }}
              >
                <span style={{ color: tokens.accent }}>{it.object_id}</span>{' '}
                <span>{it.summary_json.slice(0, 160)}</span>
              </li>
            ))}
          </ul>
        </>
      )}
      {err ? (
        <p role="alert" style={{ color: tokens.danger, fontSize: 11, fontFamily: tokens.mono }}>
          {err}
        </p>
      ) : null}
    </Panel>
  );
}

// ---------------------------------------------------------------------------

function EngineLostBanner() {
  const attached = useStudio((s) => s.engine.attached);
  const [lost, setLost] = React.useState<string | null>(null);

  React.useEffect(() => {
    const c = getClient();
    const un = c.onEngineLost((ev) => {
      setLost(ev.worker_id ?? 'unknown');
    });
    return un;
  }, []);

  React.useEffect(() => {
    if (attached) setLost(null);
  }, [attached]);

  if (!lost) return null;
  return (
    <div
      role="alert"
      style={{
        padding: '10px 14px',
        background: 'rgba(255,93,93,0.12)',
        border: `1px solid ${tokens.danger}`,
        borderRadius: tokens.radius,
        color: tokens.danger,
        fontFamily: tokens.mono,
        fontSize: 12,
      }}
    >
      Engine lost — worker <b>{lost}</b> disconnected (control channel closed). Spawn a new engine
      to resume; open-project state must be re-read via read_view.
      <span style={{ marginLeft: 12 }}>
        <Button variant="danger" onClick={() => setLost(null)}>
          Dismiss
        </Button>
      </span>
    </div>
  );
}

// ---------------------------------------------------------------------------

function MixerDevStrip() {
  const selection = useStudio((s) => s.selection);
  const [gain, setGain] = React.useState(0.75);
  const [pan, setPan] = React.useState(0);
  const attached = useStudio((s) => s.engine.attached);
  const [msg, setMsg] = React.useState('');

  if (!selection.trackId) return null;
  const tid = selection.trackId;

  return (
    <Panel title={`Selected: ${tid}`}>
      <div style={{ display: 'flex', gap: 24, alignItems: 'center' }}>
        <Knob
          label="gain (issues SetTrackGainOp)"
          value={gain}
          min={0}
          max={1}
          onChange={(v) => setGain(v)}
        />
        <Knob
          label="pan (issues SetTrackPanOp)"
          value={pan}
          min={-1}
          max={1}
          onChange={(v) => setPan(v)}
        />
        <Button
          variant="primary"
          disabled={!attached}
          onClick={() =>
            getClient()
              .setTrackGain(tid, gain)
              .then(() => getClient().setTrackPan(tid, pan))
              .then((r) => setMsg(JSON.stringify(r)))
              .catch((e) => setMsg(String(e)))
          }
        >
          Apply gain+pan
        </Button>
        <Fader
          label={`gain for ${tid}`}
          value={gain}
          min={0}
          max={1}
          onChange={(v) => setGain(v)}
        />
      </div>
      {msg ? <p style={{ ...label, marginTop: 6 }}>{msg}</p> : null}
    </Panel>
  );
}

// ---------------------------------------------------------------------------

export function Dashboard() {
  React.useEffect(() => {
    injectVoidStyles();
    void ensureClientStarted();
  }, []);

  return (
    <div
      style={{
        fontFamily: tokens.sans,
        background: tokens.bg,
        color: tokens.text,
        minHeight: '100vh',
        padding: 16,
        boxSizing: 'border-box',
        display: 'flex',
        flexDirection: 'column',
        gap: 12,
      }}
    >
      <header style={{ display: 'flex', alignItems: 'baseline', gap: 12 }}>
        <h1 style={{ fontSize: 16, margin: 0, fontFamily: tokens.mono }}>VOID Studio</h1>
        <span style={{ ...label }}>Tauri shell — real command surface, no fake meters</span>
      </header>
      <EngineLostBanner />
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
        <EnginePanel />
        <CommandPanel />
      </div>
      <TransportPanel />
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 12 }}>
        <TrackListPanel />
        <MeterStrip />
      </div>
      <MixerDevStrip />
      <ReadViewPanel />
    </div>
  );
}
