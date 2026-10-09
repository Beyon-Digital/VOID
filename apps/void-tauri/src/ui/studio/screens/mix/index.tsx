// S04 — Mix / routing and device chain (figma 4:308).
//
// Data contract:
//   strips      ← TRACK_LIST items → stripsFromTrackItems (mixer/model)
//   routing     ← routeModelFromTrackItems (buses are real; sends are NOT
//                 expressible in the schema — the rail says so honestly)
//   gain/pan/…  ← mixer controller ops with receipts (UI-T05: one gesture
//                 = one transaction id, sendWithStaleRetry, undo via the
//                 engine's own receipt path — never optimistic writes)
//   device chain← PLUGIN_LIST items → deviceChainFromItems (mixer/devices);
//                 the editor button sends the real OpenPluginEditorOp and
//                 reports a rejection verbatim (UI-T35 — never a web
//                 placeholder window)
//   automation  ← mixAutomation feature store (view state; curves are
//                 unsent intent — the wire carries no automation ops)
//   meters      ← telemetry.meters via meterStrip (live/idle honesty;
//                 idle strips show 'idle', never fake decay)

import * as React from 'react';
import {
  ActionButton,
  ChannelFader,
  DeviceSlot,
  IconButton,
  ParameterKnob,
  StatusBadge,
  tokens,
} from 'void-ui';
import {
  deviceChainFromItems,
  gainToDb,
  makeLane,
  makeViewKey,
  meterStrip,
  MIN_DB,
  paramKey,
  removeInsertOp,
  routeModelFromTrackItems,
  sendWithStaleRetry,
  setGainDb,
  setMuted,
  setPan,
  setSoloed,
  studioStore,
  stripsFromTrackItems,
  useStore,
  useStudio,
  loadViewPage,
  type AutomationMode,
  type MixerStripModel,
} from 'void-studio';
import type { MeterFrame, ReadItem } from 'void-client';
import { getClient } from '../../../client';
import { useStudioCompactContext } from '../../useStudioData';
import { mixAutomation, mixView, mixerBinding, refreshTracks } from './mixStore';

const AUTOMATION_MODES: AutomationMode[] = ['off', 'read', 'touch', 'latch', 'write', 'trim'];

const EMPTY_ITEMS: ReadItem[] = [];

function formatDb(db: number): string {
  return db <= MIN_DB ? '-∞' : `${db >= 0 ? '+' : ''}${db.toFixed(1)}`;
}

function formatPan(pan: number): string {
  if (Math.abs(pan) < 0.01) return 'C';
  const pct = Math.round(Math.abs(pan) * 100);
  return pan < 0 ? `${pct}L` : `${pct}R`;
}

/** Meter fill 0..1 when the strip is live, undefined when idle — never decayed. */
function meterFill(
  trackId: string,
  frames: Record<string, MeterFrame>,
  meterAt: Record<string, number>,
  nowMs: number,
): number | undefined {
  const m = meterStrip(frames[trackId], nowMs, meterAt[trackId]);
  if (m.status !== 'live') return undefined;
  const f = frames[trackId];
  return Math.min(1, Math.max(0, Math.max(f.peak_l ?? 0, f.peak_r ?? 0)));
}

export default function MixScreen() {
  const compact = useStudioCompactContext();
  const projectId = useStudio((s) => s.projectId);
  const revision = useStudio((s) => s.revision);
  const attached = useStudio((s) => s.engine.attached);
  const trackEntry = useStudio((s) => s.views[makeViewKey('TRACK_LIST')]);
  const pluginEntry = useStudio((s) => s.views[makeViewKey('PLUGIN_LIST')]);
  const meters = useStudio((s) => s.telemetry.meters);
  const strips = useStore(mixView, (s) => s.strips);
  const meterAt = useStore(mixView, (s) => s.meterAt);
  const lanes = useStore(mixAutomation, (s) => s.lanes);
  const visibleByTrack = useStore(mixAutomation, (s) => s.visibleByTrack);
  const writeArmed = useStore(mixAutomation, (s) => s.writeArmed);

  const [railOpen, setRailOpen] = React.useState(!compact);
  const [detailOpen, setDetailOpen] = React.useState(!compact);
  const [selectedId, setSelectedId] = React.useState<string | null>(null);
  const [editorError, setEditorError] = React.useState<string | null>(null);
  const [nowMs, setNowMs] = React.useState(() => Date.now());

  // Meter freshness is wall-clock — tick while live frames stream.
  React.useEffect(() => {
    const t = window.setInterval(() => setNowMs(Date.now()), 100);
    return () => window.clearInterval(t);
  }, []);

  // Stable references: `?? []` inline would be a fresh array every render and
  // the [items] effect below would loop setStrips → re-render forever.
  const items: ReadItem[] = trackEntry?.items ?? EMPTY_ITEMS;
  const pluginItems: ReadItem[] = pluginEntry?.items ?? EMPTY_ITEMS;

  // Re-derive strips on every TRACK_LIST page; keep last-accepted mixer
  // values and pending flags where the summary doesn't carry the field.
  React.useEffect(() => {
    const prev = mixView.getState().strips;
    mixView.getState().actions.setStrips(
      stripsFromTrackItems(items).map((s) => {
        const old = prev.find((p) => p.trackId === s.trackId);
        return old
          ? {
              ...s,
              gainLinear: s.gainLinear ?? old.gainLinear,
              pan: s.pan ?? old.pan,
              muted: s.muted ?? old.muted,
              soloed: s.soloed ?? old.soloed,
              pending: old.pending,
            }
          : s;
      }),
    );
  }, [items]);

  // Stamp meter arrival when the telemetry map lands — freshness is the
  // wall-clock gap between arrival and render (METER_FRESH_MS in meters.ts).
  React.useEffect(() => {
    const actions = mixView.getState().actions;
    const now = Date.now();
    for (const id of Object.keys(meters ?? {})) actions.noteMeter(id, now);
  }, [meters]);

  React.useEffect(() => {
    if (!projectId || !attached) return;
    void refreshTracks();
    void loadViewPage(studioStore, getClient(), 'PLUGIN_LIST').catch(() => undefined);
  }, [projectId, attached, revision]);

  const route = React.useMemo(() => routeModelFromTrackItems(items), [items]);
  const stripList = React.useMemo(
    () => [...strips].sort((a, b) => (a.kind === 'BUS' ? 1 : 0) - (b.kind === 'BUS' ? 1 : 0)),
    [strips],
  );
  const master = React.useMemo(() => stripList.find((s) => s.kind === 'BUS'), [stripList]);

  const selected = React.useMemo(
    () => stripList.find((s) => s.trackId === selectedId) ?? stripList[0] ?? null,
    [stripList, selectedId],
  );

  const chain = React.useMemo(
    () => (selected ? deviceChainFromItems(pluginItems, selected.trackId) : []),
    [pluginItems, selected],
  );

  // Automation lanes for the selected track — created on demand so the
  // store only ever holds params the user actually toggled.
  const gainLaneId = selected ? `auto:${paramKey({ kind: 'trackGain', trackId: selected.trackId })}` : '';
  const panLaneId = selected ? `auto:${paramKey({ kind: 'trackPan', trackId: selected.trackId })}` : '';
  const gainLane = selected ? lanes[gainLaneId] : undefined;
  const panLane = selected ? lanes[panLaneId] : undefined;
  const visibleLanes = selected ? visibleByTrack[selected.trackId] ?? [] : [];

  const ensureLane = (id: string, param: 'trackGain' | 'trackPan') => {
    if (!selected || mixAutomation.getState().lanes[id]) return;
    mixAutomation.getState().actions.upsertLane(makeLane(id, { kind: param, trackId: selected.trackId }));
  };

  const toggleLane = (id: string, param: 'trackGain' | 'trackPan') => {
    if (!selected) return;
    ensureLane(id, param);
    mixAutomation.getState().actions.setLaneVisible(selected.trackId, id, !visibleLanes.includes(id));
  };

  const openEditor = async (instanceId: string, name: string) => {
    setEditorError(null);
    const outcome = await sendWithStaleRetry(
      getClient(),
      { OpenPluginEditorOp: { plugin_instance_id: instanceId } },
      { transactionId: crypto.randomUUID(), refresh: () => loadViewPage(studioStore, getClient(), 'PLUGIN_LIST') },
    );
    if (outcome.receipt.status !== 'APPLIED' && outcome.receipt.status !== 'DUPLICATE') {
      setEditorError(`Native editor for ${name} unavailable: ${outcome.receipt.status}`);
    }
  };

  const removeDevice = async (instanceId: string, name: string) => {
    setEditorError(null);
    const outcome = await sendWithStaleRetry(getClient(), removeInsertOp(instanceId), {
      transactionId: crypto.randomUUID(),
      refresh: () => loadViewPage(studioStore, getClient(), 'PLUGIN_LIST'),
    });
    if (outcome.receipt.status !== 'APPLIED' && outcome.receipt.status !== 'DUPLICATE') {
      setEditorError(`Remove ${name} rejected: ${outcome.receipt.status}`);
    }
  };

  const engineStatus = !attached ? 'disconnected' : 'ready';
  const frameMap = (meters ?? {}) as Record<string, MeterFrame>;

  const stripEl = (strip: MixerStripModel, wide: boolean) => {
    const db = strip.gainLinear !== undefined ? gainToDb(strip.gainLinear) : 0;
    const busy = Boolean(strip.pending);
    const fill = meterFill(strip.trackId, frameMap, meterAt, nowMs);
    const sel = selected?.trackId === strip.trackId;
    return (
      <div
        key={strip.trackId}
        role="group"
        aria-label={`Channel ${strip.name}`}
        onClick={() => setSelectedId(strip.trackId)}
        style={{
          flex: '0 0 auto',
          width: wide ? 104 : 96,
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'stretch',
          gap: tokens.space8,
          padding: tokens.space8,
          background: sel ? tokens.raised : tokens.surface,
          border: `1px solid ${sel ? tokens.accent : tokens.line}`,
          borderRadius: tokens.radius8,
          cursor: 'pointer',
        }}
      >
        <div style={{ fontSize: 11, fontWeight: 600, color: tokens.text, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {strip.name}
        </div>
        <div style={{ fontSize: 10, color: tokens.subtle, fontFamily: tokens.fontNumeric }}>
          → {strip.kind === 'BUS' ? 'stereo out' : 'master'}
        </div>
        {wide ? (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            {deviceChainFromItems(pluginItems, strip.trackId).map((d, i) => (
              <DeviceSlot key={d.instanceId} index={i} name={d.name} category={d.format} powered={d.powered} failed={Boolean(d.failed)} />
            ))}
            {deviceChainFromItems(pluginItems, strip.trackId).length === 0 ? (
              <span style={{ fontSize: 10, color: tokens.subtle }}>no devices</span>
            ) : null}
          </div>
        ) : null}
        <ParameterKnob
          value={strip.pan ?? 0}
          min={-1}
          max={1}
          defaultValue={0}
          label="Pan"
          format={formatPan}
          disabled={!attached || busy}
          onCommit={(c) => void setPan(mixerBinding(), strip.trackId, c.to)}
          aria-label={`${strip.name} pan`}
        />
        <div style={{ display: 'flex', justifyContent: 'center' }}>
          <ChannelFader
            value={db}
            min={-60}
            max={6}
            defaultValue={0}
            unit="dB"
            label={formatDb(db)}
            height={180}
            width={32}
            meter={fill}
            disabled={!attached || busy}
            onCommit={(c) => void setGainDb(mixerBinding(), strip.trackId, c.to)}
            aria-label={`${strip.name} gain`}
          />
        </div>
        <div style={{ display: 'flex', gap: tokens.space4 }}>
          <ActionButton
            variant={strip.muted ? 'primary' : 'ghost'}
            disabled={!attached || busy}
            onClick={() => void setMuted(mixerBinding(), strip.trackId, !strip.muted)}
            aria-label={`Mute ${strip.name}`}
            style={{ flex: 1 }}
          >
            M
          </ActionButton>
          <ActionButton
            variant={strip.soloed ? 'primary' : 'ghost'}
            disabled={!attached || busy}
            onClick={() => void setSoloed(mixerBinding(), strip.trackId, !strip.soloed)}
            aria-label={`Solo ${strip.name}`}
            style={{ flex: 1 }}
          >
            S
          </ActionButton>
        </div>
      </div>
    );
  };

  return (
    <div style={{ display: 'flex', flexDirection: 'column', height: '100%', fontFamily: tokens.sans, color: tokens.text, background: tokens.ink }}>
      <div style={{ display: 'flex', alignItems: 'center', gap: tokens.space8, padding: `${tokens.space8} ${tokens.space12}`, borderBottom: `1px solid ${tokens.line}` }}>
        {compact ? (
          <>
            <IconButton icon="☰" aria-label="Toggle mix groups rail" onClick={() => setRailOpen(!railOpen)} />
            <IconButton icon="◧" aria-label="Toggle master output" onClick={() => setDetailOpen(!detailOpen)} />
          </>
        ) : null}
        <span style={{ fontSize: 12, fontWeight: 600 }}>Channels</span>
        <StatusBadge status={engineStatus} />
        <span style={{ fontSize: 11, color: tokens.subtle }}>
          {items.length === 0 && attached ? 'no channels reported' : `${stripList.length} channel(s)`}
        </span>
        <div style={{ flex: 1 }} />
        {writeArmed ? <StatusBadge status="recording" /> : null}
        <ActionButton
          variant={writeArmed ? 'danger' : 'ghost'}
          disabled={!attached}
          onClick={() => mixAutomation.getState().actions.setWriteArmed(!writeArmed)}
          aria-label="Arm automation write"
        >
          Write automation
        </ActionButton>
      </div>

      <div style={{ display: 'flex', flex: 1, minHeight: 0 }}>
        {railOpen ? (
          <aside style={{ width: 200, flex: '0 0 auto', borderRight: `1px solid ${tokens.line}`, padding: tokens.space12, display: 'flex', flexDirection: 'column', gap: tokens.space8, overflowY: 'auto' }}>
            <span style={{ fontSize: 10, fontWeight: 700, letterSpacing: 1, color: tokens.subtle }}>MIXER</span>
            {route.buses.length === 0 ? (
              <span style={{ fontSize: 11, color: tokens.subtle }}>No mix groups — channels feed the master implicitly.</span>
            ) : (
              route.buses.map((busId) => (
                <ActionButton key={busId} variant="ghost" onClick={() => setSelectedId(busId)} style={{ justifyContent: 'flex-start' }}>
                  {strips.find((x) => x.trackId === busId)?.name ?? busId}
                </ActionButton>
              ))
            )}
            <span style={{ fontSize: 10, color: tokens.subtle }}>
              Send/aux routing is not expressible on the wire yet — routes shown are real bus destinations.
            </span>
            <div style={{ borderTop: `1px solid ${tokens.line}`, paddingTop: tokens.space8 }}>
              <span style={{ fontSize: 10, fontWeight: 700, letterSpacing: 1, color: tokens.subtle }}>SELECTED CHANNEL</span>
              <div style={{ fontSize: 12, color: tokens.text, marginTop: 4 }}>{selected?.name ?? '—'}</div>
              <span style={{ fontSize: 10, color: tokens.subtle }}>Signal flows left to right. All controls target this channel.</span>
            </div>
            {selected ? (
              <div style={{ borderTop: `1px solid ${tokens.line}`, paddingTop: tokens.space8, display: 'flex', flexDirection: 'column', gap: 4 }}>
                <span style={{ fontSize: 10, fontWeight: 700, letterSpacing: 1, color: tokens.subtle }}>AUTOMATION</span>
                {([
                  { id: gainLaneId, label: 'Gain', param: 'trackGain' as const, lane: gainLane },
                  { id: panLaneId, label: 'Pan', param: 'trackPan' as const, lane: panLane },
                ]).map((row) => (
                  <div key={row.id} style={{ display: 'flex', alignItems: 'center', gap: tokens.space4 }}>
                    <ActionButton
                      variant={visibleLanes.includes(row.id) ? 'primary' : 'ghost'}
                      onClick={() => toggleLane(row.id, row.param)}
                      aria-label={`Toggle ${row.label} automation lane`}
                      style={{ flex: 1, justifyContent: 'flex-start' }}
                    >
                      {row.label}
                    </ActionButton>
                    {row.lane ? (
                      <select
                        aria-label={`${row.label} automation mode`}
                        value={row.lane.mode}
                        onChange={(e) => mixAutomation.getState().actions.setMode(row.id, e.target.value as AutomationMode)}
                        style={{ background: tokens.raised, color: tokens.text, border: `1px solid ${tokens.line}`, borderRadius: tokens.radius4, fontSize: 10, fontFamily: tokens.sans }}
                      >
                        {AUTOMATION_MODES.map((m) => (
                          <option key={m} value={m}>{m}</option>
                        ))}
                      </select>
                    ) : null}
                  </div>
                ))}
                <span style={{ fontSize: 10, color: tokens.subtle }}>Lanes are view state — the protocol carries no automation ops yet.</span>
              </div>
            ) : null}
          </aside>
        ) : null}

        <main style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
          <div style={{ flex: 1, display: 'flex', gap: tokens.space8, padding: tokens.space12, overflowX: 'auto', alignItems: 'stretch' }}>
            {stripList.length === 0 ? (
              <div style={{ padding: tokens.space24, color: tokens.subtle, fontSize: 12 }}>
                {attached ? 'No channels in TRACK_LIST yet.' : 'Engine detached — channels appear after reconnect.'}
              </div>
            ) : (
              stripList.map((s) => stripEl(s, !compact))
            )}
          </div>

          {selected ? (
            <section style={{ borderTop: `1px solid ${tokens.line}`, padding: tokens.space12, display: 'flex', gap: tokens.space16, alignItems: 'flex-start', overflowX: 'auto' }}>
              <div style={{ minWidth: 160 }}>
                <span style={{ fontSize: 10, fontWeight: 700, letterSpacing: 1, color: tokens.subtle }}>{selected.name.toUpperCase()}</span>
                <div style={{ fontSize: 11, color: tokens.muted, marginTop: 4 }}>
                  {(chain.length > 0 ? chain.map((d) => d.name).join(' → ') + ' → ' : '') + 'master'}
                </div>
                <div style={{ fontSize: 10, color: tokens.subtle, marginTop: 4 }}>
                  Delay compensation: not reported by the engine.
                </div>
              </div>
              <div style={{ display: 'flex', flexDirection: 'column', gap: 4, minWidth: 220 }}>
                <span style={{ fontSize: 10, fontWeight: 700, letterSpacing: 1, color: tokens.subtle }}>DEVICE CHAIN</span>
                {chain.map((d, i) => (
                  <div key={d.instanceId} style={{ display: 'flex', gap: tokens.space4, alignItems: 'center' }}>
                    <DeviceSlot
                      index={i}
                      name={d.name}
                      category={d.format}
                      powered={d.powered}
                      failed={Boolean(d.failed)}
                      onRemove={() => void removeDevice(d.instanceId, d.name)}
                    />
                    <ActionButton
                      variant="ghost"
                      disabled={!attached}
                      onClick={() => void openEditor(d.instanceId, d.name)}
                      aria-label={`Open ${d.name} editor`}
                    >
                      Open editor
                    </ActionButton>
                  </div>
                ))}
                {chain.length === 0 ? (
                  <span style={{ fontSize: 10, color: tokens.subtle }}>No insert devices on this channel.</span>
                ) : null}
                {editorError ? <span role="alert" style={{ fontSize: 10, color: tokens.danger }}>{editorError}</span> : null}
              </div>
            </section>
          ) : null}
        </main>

        {detailOpen ? (
          <aside style={{ width: 232, flex: '0 0 auto', borderLeft: `1px solid ${tokens.line}`, padding: tokens.space12, display: 'flex', flexDirection: 'column', gap: tokens.space8, alignItems: 'center' }}>
            <span style={{ fontSize: 10, fontWeight: 700, letterSpacing: 1, color: tokens.subtle }}>STEREO OUT</span>
            {master ? (
              <>
                <span style={{ fontSize: 11, color: tokens.text }}>{master.name}</span>
                <ChannelFader
                  value={master.gainLinear !== undefined ? gainToDb(master.gainLinear) : 0}
                  min={-60}
                  max={6}
                  defaultValue={0}
                  unit="dB"
                  label={master.gainLinear !== undefined ? formatDb(gainToDb(master.gainLinear)) : '—'}
                  height={200}
                  width={48}
                  meter={meterFill(master.trackId, frameMap, meterAt, nowMs)}
                  disabled={!attached || Boolean(master.pending)}
                  onCommit={(c) => void setGainDb(mixerBinding(), master.trackId, c.to)}
                  aria-label="Master gain"
                />
              </>
            ) : (
              <span style={{ fontSize: 11, color: tokens.subtle, textAlign: 'center' }}>
                No master bus reported — output is implicit until the engine reports a bus track.
              </span>
            )}
            <div style={{ width: '100%', borderTop: `1px solid ${tokens.line}`, paddingTop: tokens.space8, fontSize: 10, color: tokens.subtle, display: 'flex', flexDirection: 'column', gap: 2 }}>
              <span>Peak: {peakText(frameMap, meterAt, nowMs)}</span>
              <span>Integrated loudness: not measured</span>
              <span>True peak: not measured</span>
              <span>No loudness target is applied automatically.</span>
            </div>
          </aside>
        ) : null}
      </div>
    </div>
  );

  function peakText(frames: Record<string, MeterFrame>, at: Record<string, number>, now: number): string {
    let peak = 0;
    let fresh = false;
    for (const [id, f] of Object.entries(frames)) {
      const p = Math.max(f.peak_l ?? 0, f.peak_r ?? 0);
      if (typeof at[id] === 'number' && now - at[id] < 250 && p > 0) { peak = Math.max(peak, p); fresh = true; }
    }
    return fresh ? `${peak.toFixed(2)}` : 'no live frames';
  }
}
