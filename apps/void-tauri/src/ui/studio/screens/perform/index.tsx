// Perform screen — S07 (clips + queued scenes) and S28 (next scene
// playing; same surface, different launch state).
//
// Wired to real state only:
//   - track columns come from the TRACK_LIST read view;
//   - clip content for cells comes from CLIP_LIST (per track);
//   - the launch queue is the scenes feature store: a queued scene stays
//     `pending` until the NATIVE clock crosses the quantized boundary —
//     never settled on assumption (UI-T21);
//   - stop-all / all-notes-off is PANIC on the transport channel: it is
//     acked by send, never queued behind edits (UI-T22);
//   - region-bound scenes map to SEEK + SET_CYCLE at the boundary
//     (sceneTransportOps) — real transport ops;
//   - rev-2 (protocol minor 1): LaunchSceneOp / StopSceneOp /
//     LaunchClipOp are on the wire and are sent for every launch — a
//     REJECTED receipt (e.g. a scene the coordinator does not know) is
//     surfaced, never painted as a queued launch, unless the scene is
//     region-bound and the real SEEK+SET_CYCLE mapping still applies.
//   - SCENE_LIST is a rev-2 read view — launch-state rows fold in when
//     the coordinator serves them; the local machine stays the paint.

import * as React from 'react';
import {
  ActionButton,
  IconButton,
  SceneCell,
  StatusBadge,
  tokens,
  injectVoidStyles,
} from 'void-ui';
import {
  TICKS_PER_QUARTER,
  createSceneStore,
  parseClipItem,
  regionOf,
  sceneTransportOps,
  slotKey,
  useStore,
  useStudio,
  type ClipView,
  type LaunchQuantize,
  type Scene,
} from 'void-studio';
import { getClient } from '../../../client';
import { useProjectSummary, useStudioCompactContext } from '../../useStudioData';
import { loadAllPages, parseTrackRow, type TrackRow } from '../shared/views';

const sceneStore = createSceneStore();
const useScenes = <T,>(sel: (s: ReturnType<typeof sceneStore.getState>) => T): T =>
  useStore(sceneStore, sel);

function quantizeLabel(q: LaunchQuantize): string {
  if (q === 'immediate') return 'now';
  if (q === 'bar') return '1 bar';
  if (q === 'beat') return '1 beat';
  return `${q.ticks} ticks`;
}

/**
 * Transport position in ticks, derived from the native clock's
 * timeline_sample + project bpm. Honest only while the engine streams
 * clock telemetry; callers treat a missing clock as "unknown".
 */
function useNowTicks(): string | null {
  const clock = useStudio((s) => s.telemetry.clock);
  const summary = useProjectSummary();
  return React.useMemo(() => {
    if (!clock || !summary.bpm || summary.bpm <= 0) return null;
    const samples = BigInt(clock.timeline_sample);
    const rate = BigInt(Math.max(1, Math.round(clock.sample_rate)));
    const tpq = BigInt(TICKS_PER_QUARTER);
    const bpmNum = Math.round(summary.bpm * 1000);
    const ticks = (samples * tpq * BigInt(bpmNum)) / (rate * 60n * 1000n);
    return ticks.toString();
  }, [clock, summary.bpm]);
}

export default function PerformScreen() {
  React.useEffect(() => injectVoidStyles(), []);
  const compact = useStudioCompactContext();
  const projectId = useStudio((s) => s.projectId);
  const revision = useStudio((s) => s.revision);
  const engineAttached = useStudio((s) => s.engine.attached);
  const clock = useStudio((s) => s.telemetry.clock);
  const grid = useScenes((s) => s.grid);
  const launch = useScenes((s) => s.launch);
  const defaultQuantize = useScenes((s) => s.defaultQuantize);
  const actions = useScenes((s) => s.actions);
  const nowTicks = useNowTicks();

  const [tracks, setTracks] = React.useState<TrackRow[]>([]);
  const [clipsByTrack, setClipsByTrack] = React.useState<Record<string, ClipView[]>>({});
  const [loadError, setLoadError] = React.useState('');
  const [launchError, setLaunchError] = React.useState('');
  const [assigning, setAssigning] = React.useState<{ sceneId: string; trackId: string } | null>(
    null,
  );
  const [railOpen, setRailOpen] = React.useState(false);

  // Pull real track columns + clip content. Re-reads when the project or
  // revision moves so the grid never shows stale content.
  React.useEffect(() => {
    if (!projectId || !engineAttached) {
      setTracks([]);
      setClipsByTrack({});
      return;
    }
    let cancelled = false;
    (async () => {
      try {
        const t = await loadAllPages('TRACK_LIST');
        if (cancelled) return;
        const rows = t.items
          .map(parseTrackRow)
          .filter((r): r is TrackRow => r !== null)
          .sort((a, b) => (a.index ?? 0) - (b.index ?? 0));
        setTracks(rows);
        const clipPages = await loadAllPages('CLIP_LIST');
        if (cancelled) return;
        const byTrack: Record<string, ClipView[]> = {};
        for (const it of clipPages.items) {
          const c = parseClipItem(it);
          if (!c) continue;
          (byTrack[c.trackId] ??= []).push(c);
        }
        setClipsByTrack(byTrack);
        setLoadError('');
      } catch (e) {
        if (!cancelled) setLoadError(String(e));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [projectId, engineAttached, revision]);

  // Keep grid columns in lockstep with the real track list.
  React.useEffect(() => {
    const ids = tracks.map((t) => t.trackId);
    const g = sceneStore.getState().grid;
    if (ids.join() !== g.trackIds.join()) {
      sceneStore.getState().actions.setGrid({ ...g, trackIds: ids });
    }
  }, [tracks]);

  // UI-T21: settle queued launches only when the native clock crosses the
  // scheduled boundary. Region-bound scenes then send their SEEK+SET_CYCLE.
  React.useEffect(() => {
    if (nowTicks === null) return;
    const now = BigInt(nowTicks);
    const st = sceneStore.getState();
    const due = new Set<string>();
    for (const slot of Object.values(st.launch.slots)) {
      if (
        (slot.phase === 'pending' || slot.phase === 'stopping') &&
        BigInt(slot.atTicks) <= now
      ) {
        due.add(slot.atTicks);
      }
    }
    for (const at of due) {
      st.actions.settle(at);
      const playingId = st.launch.playingSceneId;
      if (playingId) {
        const scene = st.grid.scenes.find((s) => s.sceneId === playingId);
        if (scene?.region) {
          for (const cmd of sceneTransportOps(scene)) {
            void sendSceneCommand(cmd).catch(() => undefined);
          }
        }
      }
    }
  }, [nowTicks]);

  /** Map the local quantize choice onto the wire enum (rev-2). */
  const wireQuantize = (): { quantize: 'IMMEDIATE' | 'BAR' | 'BEAT' | 'CUSTOM'; quantize_ticks?: string } => {
    if (defaultQuantize === 'immediate') return { quantize: 'IMMEDIATE' };
    if (defaultQuantize === 'beat') return { quantize: 'BEAT' };
    if (typeof defaultQuantize === 'object' && defaultQuantize !== null && 'ticks' in defaultQuantize)
      return { quantize: 'CUSTOM', quantize_ticks: String(defaultQuantize.ticks) };
    return { quantize: 'BAR' };
  };

  const launchScene = (sceneId: string) => {
    setLaunchError('');
    const scene = grid.scenes.find((s) => s.sceneId === sceneId);
    // rev-2 LaunchSceneOp — the receipt decides whether the engine owns
    // the launch; a rejection is surfaced, never painted as queued.
    void getClient()
      .sendCommand({ LaunchSceneOp: { scene_id: sceneId, ...wireQuantize() } })
      .then((r) => {
        if (r.status === 'REJECTED') {
          if (!scene?.region) {
            setLaunchError(
              `scene launch rejected: ${r.message || r.error || 'no detail'}`,
            );
            return;
          }
          // Region-bound scenes still get the real SEEK+SET_CYCLE mapping
          // at the boundary even when the coordinator doesn't track the
          // scene entity.
        }
        try {
          // nowTicks null (engine detached / no clock) queues from tick 0 —
          // the slot stays visibly pending until real clock telemetry lands.
          actions.launch(sceneId, nowTicks ?? '0', defaultQuantize);
        } catch (e) {
          setLaunchError(String(e));
        }
      })
      .catch((e) => setLaunchError(String(e instanceof Error ? e.message : e)));
  };

  /** Per-slot clip launch (rev-2 LaunchClipOp) — the coordinator may not
   * know a UI-minted slot id; the receipt is surfaced honestly. */
  const launchClip = (slotId: string) => {
    void getClient()
      .sendCommand({ LaunchClipOp: { slot_id: slotId, ...wireQuantize() } })
      .then((r) => {
        if (r.status === 'REJECTED')
          setLaunchError(
            `clip launch rejected: ${r.message || r.error || 'no detail'}`,
          );
      })
      .catch((e) => setLaunchError(String(e instanceof Error ? e.message : e)));
  };

  const panicAll = async () => {
    setLaunchError('');
    try {
      // Immediate quantize + PANIC: never queued, acked by send (UI-T22).
      actions.stopAll(nowTicks ?? '0', 'immediate');
      // rev-2: StopSceneOp{scene_id:''} = stop every playing slot —
      // best-effort alongside the transport PANIC.
      void getClient()
        .sendCommand({ StopSceneOp: { scene_id: '', quantize: 'IMMEDIATE' } })
        .catch(() => undefined);
      await getClient().panic();
      const st = sceneStore.getState();
      const ats = new Set(
        Object.values(st.launch.slots)
          .filter((s) => s.phase === 'stopping')
          .map((s) => s.atTicks),
      );
      for (const at of ats) st.actions.settle(at);
    } catch (e) {
      setLaunchError(String(e));
    }
  };

  const stopTransport = async () => {
    try {
      await getClient().stop();
    } catch (e) {
      setLaunchError(String(e));
    }
  };

  const addScene = () => {
    const g = sceneStore.getState().grid;
    const n = g.scenes.length + 1;
    const scene: Scene = {
      sceneId: crypto.randomUUID(),
      name: `Scene ${String(n).padStart(2, '0')}`,
      // Region binds to the live cycle range only — never invented.
      region:
        clock && BigInt(clock.loop_end_ticks) > BigInt(clock.loop_start_ticks)
          ? regionOf(
              clock.loop_start_ticks,
              (BigInt(clock.loop_end_ticks) - BigInt(clock.loop_start_ticks)).toString(),
            )
          : undefined,
    };
    actions.setGrid({ ...g, scenes: [...g.scenes, scene] });
  };

  const assignClip = (sceneId: string, trackId: string, clipId: string | null) => {
    actions.setSlot({
      slotId: crypto.randomUUID(),
      sceneId,
      trackId,
      content: clipId ? { kind: 'clip', clipId } : { kind: 'empty' },
    });
    setAssigning(null);
  };

  const playingScene = grid.scenes.find((s) => s.sceneId === launch.playingSceneId);
  // launch.slots is keyed by SceneSlot.slotId — map back through the grid
  // to learn which scenes are queued.
  const queuedSceneIds = new Set(
    Object.keys(launch.slots)
      .filter((slotId) => launch.slots[slotId].phase === 'pending')
      .map((slotId) => Object.values(grid.slots).find((s) => s.slotId === slotId)?.sceneId)
      .filter((id): id is string => id !== undefined),
  );
  const queuedScene = grid.scenes.find((s) => queuedSceneIds.has(s.sceneId));
  const clipById = React.useMemo(() => {
    const m = new Map<string, ClipView>();
    for (const clips of Object.values(clipsByTrack)) for (const c of clips) m.set(c.clipId, c);
    return m;
  }, [clipsByTrack]);

  if (!projectId || !engineAttached) {
    return (
      <Centered>
        <p className="void-type-body" style={{ color: tokens.subtle, maxWidth: 420, textAlign: 'center' }}>
          {!projectId
            ? 'no project open — perform needs a loaded project to read tracks and clips'
            : 'engine detached — the launch grid is read-only until the engine reconnects'}
        </p>
      </Centered>
    );
  }

  const rail = (
    <aside
      aria-label="Performance safety"
      style={{
        width: compact ? '100%' : 280,
        flexShrink: 0,
        borderLeft: compact ? 'none' : `1px solid ${tokens.line}`,
        borderTop: compact ? `1px solid ${tokens.line}` : 'none',
        padding: tokens.space16,
        display: 'flex',
        flexDirection: 'column',
        gap: tokens.space16,
        overflowY: 'auto',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', justifyContent: 'space-between' }}>
        <span className="void-type-title" style={{ color: tokens.text }}>
          Performance safety
        </span>
        {compact ? (
          <IconButton icon="✕" aria-label="Close safety panel" onClick={() => setRailOpen(false)} />
        ) : null}
      </div>

      <div style={{ display: 'flex', alignItems: 'center', gap: tokens.space8 }}>
        <span className="void-type-small" style={{ color: tokens.subtle }}>
          Next scene
        </span>
        {queuedScene ? (
          <StatusBadge status="queued" label={queuedScene.name} />
        ) : (
          <StatusBadge status="ready" label="No scene queued" />
        )}
      </div>
      <p className="void-type-small" style={{ color: tokens.subtle, margin: 0 }}>
        {queuedScene
          ? 'Queued for the next boundary. The native clock owns the switch — the badge flips only when it lands.'
          : 'Nothing is queued. Pick a scene launch below.'}
      </p>

      <label
        className="void-type-small"
        style={{ color: tokens.subtle, display: 'flex', flexDirection: 'column', gap: 4 }}
      >
        Launch grid
        <select
          value={defaultQuantize === 'immediate' || defaultQuantize === 'bar' || defaultQuantize === 'beat' ? defaultQuantize : 'bar'}
          onChange={(e) =>
            actions.setDefaultQuantize(e.target.value as 'immediate' | 'bar' | 'beat')
          }
          style={{
            background: tokens.raised,
            color: tokens.text,
            border: `1px solid ${tokens.line}`,
            borderRadius: tokens.radius8,
            padding: '6px 8px',
            font: 'inherit',
          }}
        >
          <option value="immediate">immediate</option>
          <option value="bar">1 bar</option>
          <option value="beat">1 beat</option>
        </select>
      </label>

      <div style={{ display: 'flex', flexDirection: 'column', gap: tokens.space8 }}>
        <ActionButton variant="danger" onClick={() => void panicAll()}>
          Panic — all notes off
        </ActionButton>
        <ActionButton variant="secondary" onClick={() => void stopTransport()}>
          Stop transport
        </ActionButton>
      </div>
      <p className="void-type-small" style={{ color: tokens.subtle, margin: 0 }}>
        Panic is acknowledged by send — never queued behind edits.
      </p>
    </aside>
  );

  return (
    <div style={{ display: 'flex', flex: 1, minWidth: 0, minHeight: 0, flexDirection: compact ? 'column' : 'row' }}>
      <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', overflow: 'auto' }}>
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: tokens.space12,
            padding: `${tokens.space12} ${tokens.space16}`,
            borderBottom: `1px solid ${tokens.line}`,
          }}
        >
          <span className="void-type-title" style={{ color: tokens.text }}>
            Scenes
          </span>
          {clock ? (
            <StatusBadge
              status={clock.transport === 'PLAYING' ? 'playing' : 'ready'}
              label={clock.transport === 'PLAYING' ? 'Playing' : clock.transport === 'RECORDING' ? 'Recording' : 'Stopped'}
            />
          ) : (
            <StatusBadge status="unavailable" label="No clock telemetry" />
          )}
          <span style={{ flex: 1 }} />
          {compact ? (
            <IconButton
              icon="⚠"
              aria-label="Open safety panel"
              pressed={railOpen}
              onClick={() => setRailOpen((v) => !v)}
            />
          ) : null}
          <ActionButton variant="secondary" size="sm" onClick={addScene}>
            New scene
          </ActionButton>
        </div>

        {grid.scenes.length === 0 ? (
          <div style={{ padding: tokens.space24 }}>
            <p className="void-type-body" style={{ color: tokens.subtle, maxWidth: 480 }}>
              No scenes defined for this session. Scenes are session view-state — there
              is no scene-list wire view in protocol major.1 — so nothing is fabricated:
              create one to start a launch grid. Slot content binds to real clips.
            </p>
          </div>
        ) : (
          <div style={{ display: 'flex', flexDirection: 'column' }}>
            {/* header row: track names */}
            <div
              style={{
                display: 'grid',
                gridTemplateColumns: `160px repeat(${Math.max(1, tracks.length)}, minmax(96px, 1fr))`,
                borderBottom: `1px solid ${tokens.line}`,
              }}
            >
              <div style={{ padding: tokens.space8 }} />
              {tracks.map((t) => (
                <div
                  key={t.trackId}
                  className="void-type-small"
                  style={{ padding: tokens.space8, color: tokens.subtle, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
                >
                  {t.name ?? t.trackId}
                </div>
              ))}
            </div>

            {grid.scenes.map((scene, i) => {
              const rowQueued = queuedSceneIds.has(scene.sceneId);
              const rowPlaying = launch.playingSceneId === scene.sceneId;
              return (
                <div
                  key={scene.sceneId}
                  style={{
                    display: 'grid',
                    gridTemplateColumns: `160px repeat(${Math.max(1, tracks.length)}, minmax(96px, 1fr))`,
                    borderBottom: `1px solid ${tokens.line}`,
                    alignItems: 'center',
                  }}
                >
                  <div style={{ padding: tokens.space8, display: 'flex', flexDirection: 'column', gap: 4 }}>
                    <span className="void-type-body" style={{ color: tokens.text }}>
                      {String(i + 1).padStart(2, '0')} {scene.name}
                    </span>
                    <div style={{ display: 'flex', gap: tokens.space8, alignItems: 'center' }}>
                      <ActionButton
                        variant={rowQueued ? 'subtle' : 'secondary'}
                        size="sm"
                        disabled={rowQueued}
                        onClick={() => launchScene(scene.sceneId)}
                        aria-label={`Launch ${scene.name}`}
                      >
                        {rowQueued ? 'Queued' : rowPlaying ? 'Playing' : 'Launch'}
                      </ActionButton>
                      {rowQueued ? <StatusBadge status="queued" label={quantizeLabel(defaultQuantize)} hideDot /> : null}
                    </div>
                  </div>
                  {grid.trackIds.map((trackId) => {
                    const key = slotKey(scene.sceneId, trackId);
                    const slot = grid.slots[key];
                    const slotState = slot ? launch.slots[slot.slotId] : undefined;
                    const phase =
                      slotState?.phase ?? (rowPlaying && slot ? 'playing' : 'stopped');
                    const clip =
                      slot?.content.kind === 'clip' ? clipById.get(slot.content.clipId) : undefined;
                    const isAssigning =
                      assigning?.sceneId === scene.sceneId && assigning.trackId === trackId;
                    return (
                      <div key={key} style={{ padding: 2, position: 'relative' }}>
                        <SceneCell
                          slotId={slot?.slotId ?? key}
                          sceneId={scene.sceneId}
                          trackId={trackId}
                          contentKind={slot?.content.kind ?? 'empty'}
                          label={clip?.name}
                          color={scene.color}
                          phase={phase === 'pending' || phase === 'playing' || phase === 'stopping' ? phase : 'stopped'}
                          quantizeLabel={quantizeLabel(defaultQuantize)}
                          onLaunch={() => {
                            if (slot?.content.kind === 'clip') {
                              launchClip(slot.slotId);
                            } else {
                              launchScene(scene.sceneId);
                            }
                          }}
                          onStop={() => void panicAll()}
                        />
                        <button
                          type="button"
                          aria-label={`Assign clip to ${scene.name}`}
                          onClick={() => setAssigning({ sceneId: scene.sceneId, trackId })}
                          style={{
                            position: 'absolute',
                            top: 2,
                            right: 2,
                            background: 'transparent',
                            border: 'none',
                            color: tokens.subtle,
                            cursor: 'pointer',
                            fontSize: 10,
                            lineHeight: 1,
                            padding: 4,
                          }}
                        >
                          ⋯
                        </button>
                        {isAssigning ? (
                          <div
                            role="menu"
                            aria-label="Assign clip"
                            style={{
                              position: 'absolute',
                              top: '100%',
                              left: 0,
                              zIndex: 30,
                              background: tokens.raised,
                              border: `1px solid ${tokens.line}`,
                              borderRadius: tokens.radius8,
                              minWidth: 180,
                              padding: 4,
                              boxShadow: '0 8px 24px rgba(0,0,0,.4)',
                            }}
                          >
                            <button
                              type="button"
                              onClick={() => assignClip(scene.sceneId, trackId, null)}
                              style={menuItemStyle}
                            >
                              empty
                            </button>
                            {(clipsByTrack[trackId] ?? []).map((c) => (
                              <button
                                key={c.clipId}
                                type="button"
                                onClick={() => assignClip(scene.sceneId, trackId, c.clipId)}
                                style={menuItemStyle}
                              >
                                {c.name ?? c.clipId}
                              </button>
                            ))}
                            {(clipsByTrack[trackId] ?? []).length === 0 ? (
                              <div className="void-type-small" style={{ color: tokens.subtle, padding: 6 }}>
                                no clips on this track
                              </div>
                            ) : null}
                          </div>
                        ) : null}
                      </div>
                    );
                  })}
                </div>
              );
            })}
          </div>
        )}

        <footer
          style={{
            padding: `${tokens.space8} ${tokens.space16}`,
            borderTop: `1px solid ${tokens.line}`,
            display: 'flex',
            gap: tokens.space16,
            alignItems: 'center',
          }}
        >
          <span className="void-type-small" style={{ color: tokens.subtle }}>
            {playingScene ? `NOW ${playingScene.name}` : 'NOW —'}
            {' → '}
            {queuedScene ? `NEXT ${queuedScene.name}` : 'NEXT —'}
          </span>
          <span className="void-type-small" style={{ color: tokens.subtle }}>
            {queuedScene
              ? `Launch at next ${quantizeLabel(defaultQuantize)} boundary. Native clock owns timing; visuals follow.`
              : playingScene
                ? `${playingScene.name} playing. No next scene is queued.`
                : 'Nothing playing.'}
          </span>
          {(loadError || launchError) && (
            <span className="void-type-small" style={{ color: tokens.danger }}>
              {loadError || launchError}
            </span>
          )}
        </footer>
      </div>
      {compact ? (railOpen ? rail : null) : rail}
    </div>
  );
}

async function sendSceneCommand(cmd: {
  op: 'SEEK' | 'SET_CYCLE' | 'PLAY' | 'STOP';
  position_ticks?: string;
  cycle_start_ticks?: string;
  cycle_end_ticks?: string;
}) {
  const c = getClient();
  if (cmd.op === 'SEEK') await c.seek(cmd.position_ticks!);
  else if (cmd.op === 'SET_CYCLE') await c.setCycle(cmd.cycle_start_ticks!, cmd.cycle_end_ticks!);
  else if (cmd.op === 'PLAY') await c.play();
  else await c.stop();
}

const menuItemStyle: React.CSSProperties = {
  display: 'block',
  width: '100%',
  textAlign: 'left',
  background: 'transparent',
  border: 'none',
  color: 'inherit',
  padding: '6px 8px',
  cursor: 'pointer',
  font: 'inherit',
  fontSize: 12,
};

const Centered: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <div
    style={{
      flex: 1,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      padding: tokens.space24,
    }}
  >
    {children}
  </div>
);
