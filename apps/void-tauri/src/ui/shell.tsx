// VOID Studio workspace shell — the W09 editor surface.
//
// Layout: one shared transport bar on top, workspace tabs (Compose /
// Arrange / Mix) below, then the workspace's regions. The existing dev
// dashboard (engine spawn, project create, read inspector, meters) stays
// reachable under a collapsed "Developer surface" section — nothing here
// fakes data; every editor pane renders read-view projections only and
// every edit goes through the real void-client command surface.
//
// Ownership rule (CONTRACTS.md): React state and the stores hold view
// state only — clips/notes are re-parsed from the bounded view cache for
// display; a committed gesture always ends in a fresh re-read of the
// touched view.

import React from 'react';
import {
  Button,
  ClipBlock,
  NoteBlock,
  Panel,
  TimelineRuler,
  TrackHeader,
  TransportControls,
  WorkspaceTabs,
  formatBarBeat,
  injectVoidStyles,
  tokens,
} from 'void-ui';
import {
  ClipEditor,
  NoteEditor,
  WORKSPACES,
  WORKSPACE_ORDER,
  assertEditorViewState,
  beginClipDrag,
  clipEndTicks,
  describeReceiptError,
  editorStore,
  gridStepTicks,
  isBlackKey,
  layoutClips,
  loadViewPage,
  makeViewKey,
  noteEditIntent,
  noteRect,
  parseClipItem,
  parseNoteItem,
  pitchForKey,
  pitchName,
  previewClipDrag,
  pxToTicks,
  receiptFailed,
  setLiveCycle,
  setLoopRange,
  snapTicks,
  studioStore,
  ticksToPx,
  transportBarState,
  useStudio,
  velocityAtPy,
  velocityBars,
  velocityHit,
  workspaceForShortcut,
} from 'void-studio';
import type {
  ClipView,
  EditorViewState,
  EditorActions,
  NoteView,
  TickViewport,
  Zoom,
} from 'void-studio';
import type { ReadItem } from 'void-client';
import { parseI64 } from 'void-client';
import { ensureClientStarted, getClient } from './client';
import { Dashboard } from './dashboard';
import { HelpButton, LauncherPanel, NotesPanel, RelinkPanel, SupportOverlays } from './support';

// Lazily constructed: getClient() binds the Tauri transport on first use so
// a plain-browser preview never touches IPC at import time.
let _clipEditor: ClipEditor | null = null;
let _noteEditor: NoteEditor | null = null;
const clipEd = () =>
  (_clipEditor ??= new ClipEditor(getClient(), () => crypto.randomUUID()));
const noteEd = () =>
  (_noteEditor ??= new NoteEditor(getClient(), () => crypto.randomUUID()));
const newGesture = () => crypto.randomUUID();

const mono: React.CSSProperties = { fontFamily: tokens.mono };
const label: React.CSSProperties = {
  fontSize: 10,
  color: tokens.textMuted,
  ...mono,
};

type EditorState = EditorViewState & { actions: EditorActions };

function useEditor<T>(selector: (s: EditorState) => T): T {
  return React.useSyncExternalStore(
    (onChange) => editorStore.subscribe(onChange),
    () => selector(editorStore.getState()),
    () => selector(editorStore.getState()),
  );
}

// ---------------------------------------------------------------------------
// shared transport bar — one bar, every workspace
// ---------------------------------------------------------------------------

function SharedTransportBar() {
  const clock = useStudio((s) => s.telemetry.clock);
  const attached = useStudio((s) => s.engine.attached);
  const viewport = useStudio((s) => s.viewport);
  const setEditError = useEditor((s) => s.actions.setEditError);
  const lastEditError = useEditor((s) => s.lastEditError);
  const bar = transportBarState(clock, attached);
  const [busy, setBusy] = React.useState(false);

  const run = (fn: () => Promise<unknown>) => {
    setBusy(true);
    return fn()
      .catch((e) => setEditError(String(e)))
      .finally(() => setBusy(false));
  };

  const cycleLabel = bar.cycle.active
    ? `${formatBarBeat(bar.cycle.startTicks)} → ${formatBarBeat(bar.cycle.endTicks)}`
    : 'off';

  const toggleCycle = () =>
    run(async () => {
      if (bar.cycle.active) {
        // Disable the loop range — a document edit (enabled=false keeps bounds).
        const out = await setLoopRange(
          getClient(),
          {
            startTicks: bar.cycle.startTicks,
            endTicks: bar.cycle.endTicks,
            enabled: false,
          },
          { transactionId: newGesture() },
        );
        if (receiptFailed(out.receipt)) {
          setEditError(describeReceiptError(out.receipt));
        }
      } else {
        // Loop the current viewport: live cycle + persistent range.
        const tx = newGesture();
        await setLiveCycle(getClient(), viewport.startTicks, viewport.endTicks);
        const out = await setLoopRange(
          getClient(),
          {
            startTicks: viewport.startTicks,
            endTicks: viewport.endTicks,
            enabled: true,
          },
          { transactionId: tx },
        );
        if (receiptFailed(out.receipt)) {
          setEditError(describeReceiptError(out.receipt));
        }
      }
    });

  return (
    <div>
      <TransportControls
        transport={bar.transport}
        positionText={
          bar.timelineSample !== null ? `pos ${bar.timelineSample} samples` : 'no clock'
        }
        cycle={{ active: bar.cycle.active, label: cycleLabel }}
        busy={busy}
        disabled={!attached}
        onPlay={() => run(() => getClient().play())}
        onStop={() => run(() => getClient().stop())}
        onPanic={() => run(() => getClient().panic())}
        onToggleCycle={toggleCycle}
      />
      {bar.cycle.active ? (
        <div style={{ ...label, marginTop: 4 }} role="status">
          loop {cycleLabel} · tempo map rev {bar.tempoMapRevision ?? '—'}
        </div>
      ) : null}
      {lastEditError ? (
        <p role="alert" style={{ color: tokens.danger, fontSize: 11, ...mono, margin: '4px 0 0' }}>
          {lastEditError}
        </p>
      ) : null}
    </div>
  );
}

// ---------------------------------------------------------------------------
// track list column (left side of every workspace)
// ---------------------------------------------------------------------------

interface TrackRow {
  objectId: string;
  id: string;
  name: string;
  kind?: string;
  muted: boolean;
  soloed: boolean;
}

function useTrackRows(): { tracks: TrackRow[]; loaded: boolean } {
  const views = useStudio((s) => s.views);
  const entry = views[makeViewKey('TRACK_LIST')];
  return React.useMemo(() => {
    if (!entry) return { tracks: [], loaded: false };
    const tracks = entry.items.map((it: ReadItem) => {
      try {
        const v = JSON.parse(it.summary_json) as Record<string, unknown>;
        return {
          objectId: it.object_id,
          id: String(v.track_id ?? v.id ?? it.object_id),
          name: String(v.name ?? v.track_id ?? it.object_id),
          kind: typeof v.kind === 'string' ? v.kind : undefined,
          muted: v.muted === true,
          soloed: v.soloed === true,
        };
      } catch {
        return {
          objectId: it.object_id,
          id: it.object_id,
          name: it.object_id,
          muted: false,
          soloed: false,
        };
      }
    });
    return { tracks, loaded: true };
  }, [entry]);
}

function TrackListColumn() {
  const { tracks, loaded } = useTrackRows();
  const selection = useStudio((s) => s.selection);
  const attached = useStudio((s) => s.engine.attached);
  const setEditError = useEditor((s) => s.actions.setEditError);
  const [busy, setBusy] = React.useState(false);

  const load = () => {
    setBusy(true);
    loadViewPage(studioStore, getClient(), 'TRACK_LIST')
      .catch((e) => setEditError(String(e)))
      .finally(() => setBusy(false));
  };

  return (
    <div
      role="region"
      aria-label="Track list"
      style={{ borderRight: `1px solid ${tokens.border}`, minWidth: 180 }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 6, padding: '6px 8px' }}>
        <span style={label}>tracks</span>
        <Button
          loading={busy}
          disabled={!attached}
          onClick={load}
          aria-label="Read track list"
          style={{ marginLeft: 'auto', padding: '2px 8px', fontSize: 10 }}
        >
          {loaded ? 're-read' : 'read'}
        </Button>
      </div>
      {!loaded ? (
        <p style={{ ...label, padding: '0 8px' }}>
          no TRACK_LIST page yet — tracks render read-view data only
        </p>
      ) : tracks.length === 0 ? (
        <p style={{ ...label, padding: '0 8px' }}>TRACK_LIST: 0 items (empty project)</p>
      ) : (
        <div role="list" aria-label="Tracks">
          {tracks.map((t) => (
            <TrackHeader
              key={t.objectId}
              trackId={t.id}
              name={t.name}
              kind={t.kind}
              muted={t.muted}
              soloed={t.soloed}
              selected={selection.trackId === t.id}
              onSelect={(id) => studioStore.getState().actions.selectTrack(id)}
              onToggleMute={(id, on) =>
                getClient()
                  .setTrackMute(id, on)
                  .catch((e) => setEditError(String(e)))
              }
              onToggleSolo={(id, on) =>
                getClient()
                  .setTrackSolo(id, on)
                  .catch((e) => setEditError(String(e)))
              }
            />
          ))}
        </div>
      )}
    </div>
  );
}

// ---------------------------------------------------------------------------
// timeline pane — track rows + clip blocks from CLIP_LIST views
// ---------------------------------------------------------------------------

const ROW_H = 48;

interface LaneDrag {
  trackId: string;
  pointerId: number;
  model: ReturnType<typeof beginClipDrag>;
}

function TrackLane(props: { track: TrackRow; viewport: TickViewport; zoom: Zoom }) {
  const { track, viewport, zoom } = props;
  const views = useStudio((s) => s.views);
  const attached = useStudio((s) => s.engine.attached);
  const clipSel = useEditor((s) => s.clipSelection);
  const drag = useEditor((s) => s.drag);
  const snap = useEditor((s) => s.snap);
  const setEditError = useEditor((s) => s.actions.setEditError);
  const setDrag = useEditor((s) => s.actions.setDrag);
  const [splitArmed, setSplitArmed] = React.useState(false);
  const laneRef = React.useRef<HTMLDivElement>(null);
  const dragRef = React.useRef<LaneDrag | null>(null);
  const lastPointer = React.useRef('0');

  const key = makeViewKey('CLIP_LIST', track.id);
  const entry = views[key];

  const clips = React.useMemo(() => {
    if (!entry) return null;
    const out: ClipView[] = [];
    for (const it of entry.items) {
      const c = parseClipItem(it);
      if (c) out.push(c);
    }
    return out;
  }, [entry]);

  const laidOut = React.useMemo(
    () => (clips ? layoutClips(clips, viewport, zoom) : []),
    [clips, viewport, zoom],
  );

  const loadClips = React.useCallback(() => {
    if (!attached) return Promise.resolve();
    return clipEd()
      .loadTrackClips(studioStore, track.id)
      .then(() => undefined)
      .catch((e) => setEditError(String(e)));
  }, [attached, track.id, setEditError]);

  const ticksAt = (e: React.PointerEvent): string => {
    const rect = laneRef.current?.getBoundingClientRect();
    const x = rect ? e.clientX - rect.left : 0;
    return pxToTicks(x, viewport, zoom);
  };

  const commitDrag = async (lane: LaneDrag) => {
    const preview = previewClipDrag(lane.model, lastPointer.current, snap);
    const res = await clipEd().commitDrag(lane.model, preview, () => loadClips());
    setEditError(res.errorText || null);
    setDrag(null);
    // Whether the op applied or failed, re-read the lane — the committed
    // truth (or the revert) always comes from the view, not the preview.
    await loadClips();
  };

  const onLanePointerMove = (e: React.PointerEvent) => {
    const lane = dragRef.current;
    if (!lane) return;
    lastPointer.current = ticksAt(e);
    const preview = previewClipDrag(lane.model, lastPointer.current, snap);
    setDrag({
      mode: lane.model.mode,
      clipId: lane.model.clipId,
      trackId: track.id,
      transactionId: lane.model.transactionId,
      startTicks: preview.startTicks,
      lengthTicks: preview.lengthTicks,
      offsetTicks: preview.offsetTicks,
      atTicks: preview.atTicks,
      targetTrackId: preview.trackId,
    });
  };

  const onLanePointerUp = (e: React.PointerEvent) => {
    const lane = dragRef.current;
    if (!lane) return;
    dragRef.current = null;
    laneRef.current?.releasePointerCapture?.(e.pointerId);
    void commitDrag(lane);
  };

  const beginGesture = (
    zone: 'body' | 'trim-start' | 'trim-end',
    clipId: string,
    e: React.PointerEvent<HTMLDivElement>,
  ) => {
    const clip = clips?.find((c) => c.clipId === clipId);
    if (!clip || !attached) return;
    const grabTicks = ticksAt(e);
    const mode = splitArmed
      ? ('split' as const)
      : zone === 'body'
        ? e.altKey
          ? 'duplicate'
          : 'move'
        : zone;
    const model = beginClipDrag(clip, mode, grabTicks, newGesture(), track.id);
    lastPointer.current = grabTicks;
    if (mode === 'split') {
      // Click-to-split: no drag — commit at the snapped click position.
      const preview = previewClipDrag(model, grabTicks, snap);
      setSplitArmed(false);
      void clipEd()
        .commitDrag(model, preview, () => loadClips())
        .then((res) => {
          setEditError(res.errorText || null);
          return loadClips();
        });
      return;
    }
    dragRef.current = { trackId: track.id, pointerId: e.pointerId, model };
    laneRef.current?.setPointerCapture?.(e.pointerId);
    // Seed the optimistic preview at the grab point.
    const preview = previewClipDrag(model, grabTicks, snap);
    setDrag({
      mode: model.mode,
      clipId: model.clipId,
      trackId: track.id,
      transactionId: model.transactionId,
      startTicks: preview.startTicks,
      lengthTicks: preview.lengthTicks,
      offsetTicks: preview.offsetTicks,
      targetTrackId: preview.trackId,
    });
  };

  const removeSelected = () => {
    const ids = clipSel.trackId === track.id ? clipSel.clipIds : [];
    if (ids.length === 0 || !attached) return;
    const tx = newGesture();
    void (async () => {
      for (const clipId of ids) {
        const res = await clipEd().removeClip(clipId, tx, () => loadClips());
        if (res.errorText) {
          setEditError(res.errorText);
          break;
        }
      }
      await loadClips();
    })();
  };

  return (
    <div
      aria-label={`Lane for track ${track.name}`}
      style={{ display: 'flex', borderBottom: `1px solid ${tokens.border}`, minHeight: ROW_H }}
    >
      <div
        ref={laneRef}
        role="application"
        aria-label={`Clips on ${track.name} — drag to move, edges trim, Alt-drag duplicates${splitArmed ? ', split armed' : ''}`}
        tabIndex={0}
        onPointerMove={onLanePointerMove}
        onPointerUp={onLanePointerUp}
        onKeyDown={(e) => {
          if (e.key === 'Delete' || e.key === 'Backspace') {
            e.preventDefault();
            removeSelected();
          } else if (e.key.toLowerCase() === 's') {
            e.preventDefault();
            setSplitArmed((v) => !v);
          }
        }}
        style={{
          position: 'relative',
          flex: 1,
          height: ROW_H,
          background: splitArmed ? 'rgba(106,141,255,0.06)' : 'transparent',
          cursor: splitArmed ? 'crosshair' : 'default',
          overflow: 'hidden',
        }}
      >
        {clips === null ? (
          <span style={{ ...label, position: 'absolute', left: 8, top: 16 }}>
            lane not read — press “read” on the track list
          </span>
        ) : laidOut.length === 0 ? (
          <span style={{ ...label, position: 'absolute', left: 8, top: 16 }}>
            no clips in view (CLIP_LIST empty for this range)
          </span>
        ) : (
          laidOut.map(({ clip, x, w }) => {
            const isPreview = drag?.clipId === clip.clipId;
            const pv = isPreview
              ? {
                  x: ticksToPx(drag!.startTicks, viewport, zoom),
                  w: Math.max(2, Number(parseI64(drag!.lengthTicks)) / zoom.ticksPerPixel),
                }
              : { x, w };
            return (
              <ClipBlock
                key={clip.clipId}
                clipId={clip.clipId}
                name={clip.name}
                kind={clip.kind}
                x={pv.x}
                w={pv.w}
                top={4}
                height={ROW_H - 8}
                selected={
                  clipSel.trackId === track.id && clipSel.clipIds.includes(clip.clipId)
                }
                pending={isPreview}
                color={clip.color}
                onPointerZone={beginGesture}
                onSelect={(clipId, additive) => {
                  const a = editorStore.getState().actions;
                  if (additive) a.toggleClip(track.id, clipId);
                  else a.selectClips(track.id, [clipId]);
                  studioStore.getState().actions.selectClip(track.id, clipId);
                }}
              />
            );
          })
        )}
        {drag?.mode === 'split' && drag.trackId === track.id && drag.atTicks ? (
          <div
            aria-hidden
            style={{
              position: 'absolute',
              left: ticksToPx(drag.atTicks, viewport, zoom),
              top: 0,
              bottom: 0,
              width: 1,
              background: tokens.warn,
            }}
          />
        ) : null}
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: 2, padding: 4 }}>
        <Button
          aria-label={`Split clips on ${track.name} at click`}
          aria-pressed={splitArmed}
          onClick={() => setSplitArmed((v) => !v)}
          disabled={!attached}
          style={{ fontSize: 9, padding: '2px 6px' }}
        >
          split
        </Button>
        <Button
          aria-label={`Delete selected clips on ${track.name}`}
          onClick={removeSelected}
          disabled={
            !attached || clipSel.trackId !== track.id || clipSel.clipIds.length === 0
          }
          style={{ fontSize: 9, padding: '2px 6px' }}
        >
          del
        </Button>
      </div>
    </div>
  );
}

function TimelinePane() {
  const { tracks, loaded } = useTrackRows();
  const viewport = useStudio((s) => s.viewport);
  const zoom = useStudio((s) => s.zoom);
  const attached = useStudio((s) => s.engine.attached);
  const snap = useEditor((s) => s.snap);
  const actions = useStudio((s) => s.actions);
  const [widthPx, setWidthPx] = React.useState(760);
  const containerRef = React.useRef<HTMLDivElement>(null);

  React.useEffect(() => {
    const el = containerRef.current;
    if (!el || typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver((rs) =>
      setWidthPx(Math.max(200, rs[0].contentRect.width)),
    );
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Snap grid revision follows the engine's tempo_map_revision.
  const clock = useStudio((s) => s.telemetry.clock);
  React.useEffect(() => {
    if (
      clock?.tempo_map_revision &&
      clock.tempo_map_revision !== snap.tempoMapRevision
    ) {
      editorStore
        .getState()
        .actions.setSnap({ tempoMapRevision: clock.tempo_map_revision });
    }
    // snap.tempoMapRevision is read for comparison only
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [clock?.tempo_map_revision]);

  return (
    <section
      role="region"
      aria-label="Timeline — arrangement editor"
      ref={containerRef}
      style={{ flex: 1, minWidth: 0 }}
    >
      <div
        style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '4px 8px' }}
      >
        <TimelineRuler
          startTicks={viewport.startTicks}
          ticksPerPx={zoom.ticksPerPixel}
          widthPx={widthPx}
          beatsPerBar={snap.beatsPerBar}
          onSeek={attached ? (t) => void getClient().seek(t) : undefined}
          aria-label="Timeline ruler — click or arrow keys to seek"
        />
      </div>
      <div
        style={{ display: 'flex', alignItems: 'center', gap: 8, padding: '0 8px 6px' }}
      >
        <label style={label}>
          snap
          <select
            aria-label="Snap division"
            value={snap.division}
            onChange={(e) =>
              editorStore
                .getState()
                .actions.setSnap({
                  division: e.target.value as typeof snap.division,
                })
            }
            style={{
              marginLeft: 4,
              background: tokens.bg,
              color: tokens.text,
              border: `1px solid ${tokens.border}`,
              borderRadius: 3,
              fontFamily: tokens.mono,
              fontSize: 10,
            }}
          >
            {['bar', 'beat', '1/2', '1/4', '1/8', '1/16', '1/64'].map((d) => (
              <option key={d} value={d}>
                {d}
              </option>
            ))}
          </select>
        </label>
        <label style={label}>
          <input
            type="checkbox"
            aria-label="Snap enabled"
            checked={snap.enabled}
            onChange={(e) =>
              editorStore.getState().actions.setSnap({ enabled: e.target.checked })
            }
          />{' '}
          on
        </label>
        <label style={label}>
          zoom
          <input
            type="range"
            aria-label="Timeline zoom"
            min={-4}
            max={4}
            step={0.25}
            defaultValue={0}
            onChange={(e) =>
              actions.setTicksPerPixel(48000 * Math.pow(2, -Number(e.target.value)))
            }
            style={{ width: 90 }}
          />
        </label>
        <span style={{ ...label, marginLeft: 'auto' }}>
          view {formatBarBeat(viewport.startTicks)} → {formatBarBeat(viewport.endTicks)}
          {attached ? '' : ' · engine detached'}
        </span>
      </div>
      {!loaded ? (
        <p style={{ ...label, padding: '4px 8px' }}>
          read TRACK_LIST first — lanes render read-view clips only
        </p>
      ) : (
        tracks.map((t) => (
          <TrackLane key={t.objectId} track={t} viewport={viewport} zoom={zoom} />
        ))
      )}
    </section>
  );
}

// ---------------------------------------------------------------------------
// piano-roll pane — NOTE_RANGE for the selected clip
// ---------------------------------------------------------------------------

const NOTE_ROW_H = 14;
const VEL_LANE_H = 56;
const GRID_W = 760;

function PianoRollPane() {
  const clipSel = useEditor((s) => s.clipSelection);
  const noteSel = useEditor((s) => s.noteSelection);
  const snap = useEditor((s) => s.snap);
  const setEditError = useEditor((s) => s.actions.setEditError);
  const views = useStudio((s) => s.views);
  const attached = useStudio((s) => s.engine.attached);
  const viewport = useStudio((s) => s.viewport);
  const zoom = useStudio((s) => s.zoom);
  const gridRef = React.useRef<HTMLDivElement>(null);
  const [pitchBase, setPitchBase] = React.useState(60);
  const [insertVel, setInsertVel] = React.useState(100);

  // The selected clip's ClipView, re-projected from the view cache.
  const clip = React.useMemo((): ClipView | null => {
    if (!clipSel.trackId || clipSel.clipIds.length === 0) return null;
    const entry = views[makeViewKey('CLIP_LIST', clipSel.trackId)];
    if (!entry) return null;
    for (const it of entry.items) {
      const c = parseClipItem(it);
      if (c && c.clipId === clipSel.clipIds[0]) return c;
    }
    return null;
  }, [clipSel, views]);

  const clipIdKey = clip
    ? `${clip.trackId}|${clip.clipId}|${clip.startTicks}|${clip.lengthTicks}`
    : '';

  const noteKey = clip
    ? makeViewKey('NOTE_RANGE', clip.trackId, clip.startTicks, clipEndTicks(clip))
    : '';
  const noteEntry = noteKey ? views[noteKey] : undefined;

  const notes = React.useMemo((): NoteView[] => {
    if (!clip || !noteEntry) return [];
    const out: NoteView[] = [];
    for (const it of noteEntry.items) {
      const n = parseNoteItem(it);
      if (n && n.clipId === clip.clipId) out.push(n);
    }
    return out;
  }, [clip, noteEntry]);

  // Load when a different clip is selected (or the engine attaches) — not
  // on every view-cache update, which would loop (a load mutates views).
  const clipRef = React.useRef<ClipView | null>(null);
  clipRef.current = clip;
  React.useEffect(() => {
    const c = clipRef.current;
    if (!c || !attached) return;
    void noteEd()
      .loadClipNotes(studioStore, c)
      .catch((e) =>
        editorStore.getState().actions.setEditError(String(e)),
      );
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [clipIdKey, attached]);

  const loadNotes = React.useCallback(() => {
    const c = clipRef.current;
    if (!c || !attached) return Promise.resolve();
    return noteEd()
      .loadClipNotes(studioStore, c)
      .then(() => undefined)
      .catch((e) => setEditError(String(e)));
  }, [attached, setEditError]);

  const selectedNotes = notes.filter(
    (n) => noteSel.clipId === clip?.clipId && noteSel.noteIds.includes(n.noteId),
  );

  const runIntent = (intent: ReturnType<typeof noteEditIntent>) => {
    if (!clip || !attached || intent.type === 'noop' || selectedNotes.length === 0)
      return;
    const tx = newGesture();
    void noteEd()
      .applyIntent(clip.clipId, selectedNotes, intent, tx, () => loadNotes())
      .then(async (results) => {
        const failed = results.find((r) => r.errorText);
        setEditError(failed ? failed.errorText : null);
        await loadNotes();
      });
  };

  const onGridDoubleClick = (e: React.MouseEvent) => {
    if (!clip || !attached) return;
    const rect = gridRef.current?.getBoundingClientRect();
    if (!rect) return;
    const px = e.clientX - rect.left;
    const row = Math.floor((e.clientY - rect.top) / NOTE_ROW_H);
    const pitch = 127 - row;
    const startTicks = snapTicks(pxToTicks(px, viewport, zoom), snap);
    const stepTicks = snap.enabled ? gridStepTicks(snap).toString(10) : '960000';
    void noteEd()
      .insertNote(
        clip.clipId,
        pitch,
        insertVel,
        startTicks,
        stepTicks,
        newGesture(),
        () => loadNotes(),
      )
      .then((r) => {
        setEditError(r.errorText || null);
        return loadNotes();
      });
  };

  const onVelocityPointer = (e: React.PointerEvent<HTMLDivElement>) => {
    if (!clip || !attached) return;
    if (e.type !== 'pointerdown' && e.buttons !== 1) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const px = e.clientX - rect.left;
    const py = e.clientY - rect.top;
    const rects = notes.map((note) => {
      const r = noteRect(note, viewport, zoom, NOTE_ROW_H);
      return { note, x: r.x, w: r.w };
    });
    const hit = velocityHit(rects, px);
    if (!hit) return;
    if (e.type === 'pointerdown') e.currentTarget.setPointerCapture(e.pointerId);
    void noteEd()
      .setVelocity(
        clip.clipId,
        hit.noteId,
        velocityAtPy(py, VEL_LANE_H),
        newGesture(),
        () => loadNotes(),
      )
      .then((r) => setEditError(r.errorText || null));
  };

  return (
    <section
      role="region"
      aria-label="Piano roll — MIDI note editor"
      style={{ flex: 1, minWidth: 0 }}
      onKeyDown={(e) => {
        const intent = noteEditIntent(
          e.key,
          { shift: e.shiftKey, alt: e.altKey },
          snap,
        );
        if (intent.type !== 'noop') {
          e.preventDefault();
          runIntent(intent);
          return;
        }
        const pitch = pitchForKey(e.key, pitchBase);
        if (pitch !== null && clip && attached) {
          e.preventDefault();
          const startTicks = snapTicks(viewport.startTicks, snap);
          const stepTicks = snap.enabled
            ? gridStepTicks(snap).toString(10)
            : '960000';
          void noteEd()
            .insertNote(
              clip.clipId,
              pitch,
              insertVel,
              startTicks,
              stepTicks,
              newGesture(),
              () => loadNotes(),
            )
            .then((r) => {
              setEditError(r.errorText || null);
              return loadNotes();
            });
        }
      }}
    >
      <div style={{ display: 'flex', gap: 8, alignItems: 'center', padding: '4px 8px' }}>
        <span style={label}>
          {clip
            ? `clip ${clip.name ?? clip.clipId} · ${notes.length} notes`
            : 'select a clip in Arrange to edit notes'}
        </span>
        <label style={label}>
          base pitch
          <input
            aria-label="Keyboard base pitch"
            type="number"
            value={pitchBase}
            min={0}
            max={115}
            onChange={(e) =>
              setPitchBase(Math.max(0, Math.min(115, Number(e.target.value) || 0)))
            }
            style={{
              width: 48,
              marginLeft: 4,
              background: tokens.bg,
              color: tokens.text,
              border: `1px solid ${tokens.border}`,
              borderRadius: 3,
              fontFamily: tokens.mono,
              fontSize: 10,
            }}
          />
        </label>
        <label style={label}>
          velocity
          <input
            aria-label="Insert velocity"
            type="number"
            value={insertVel}
            min={1}
            max={127}
            onChange={(e) =>
              setInsertVel(Math.max(1, Math.min(127, Number(e.target.value) || 1)))
            }
            style={{
              width: 44,
              marginLeft: 4,
              background: tokens.bg,
              color: tokens.text,
              border: `1px solid ${tokens.border}`,
              borderRadius: 3,
              fontFamily: tokens.mono,
              fontSize: 10,
            }}
          />
        </label>
        <Button
          disabled={!clip || !attached}
          onClick={loadNotes}
          style={{ fontSize: 10, padding: '2px 8px' }}
        >
          re-read notes
        </Button>
        <span style={{ ...label, marginLeft: 'auto' }}>
          z–m keys enter notes · arrows move · +/− velocity · del deletes
        </span>
      </div>
      {!clip ? (
        <p style={{ ...label, padding: '0 8px' }}>
          piano roll shows notes for one selected MIDI clip — empty state, nothing
          fabricated
        </p>
      ) : (
        <>
          <div
            ref={gridRef}
            role="application"
            aria-label={`Note grid for clip ${clip.name ?? clip.clipId} — double-click inserts a note`}
            tabIndex={0}
            onDoubleClick={onGridDoubleClick}
            style={{
              position: 'relative',
              height: Math.min(420, 128 * NOTE_ROW_H),
              overflow: 'auto',
              background: tokens.bg,
              border: `1px solid ${tokens.border}`,
              borderRadius: tokens.radius,
            }}
          >
            <div
              style={{ position: 'relative', width: GRID_W, height: 128 * NOTE_ROW_H }}
            >
              {Array.from({ length: 128 }, (_, i) => {
                const pitch = 127 - i;
                return (
                  <div
                    key={pitch}
                    aria-hidden
                    style={{
                      position: 'absolute',
                      top: i * NOTE_ROW_H,
                      left: 0,
                      right: 0,
                      height: NOTE_ROW_H,
                      background: isBlackKey(pitch)
                        ? 'rgba(255,255,255,0.03)'
                        : 'transparent',
                      borderBottom: `1px solid ${tokens.border}`,
                    }}
                  />
                );
              })}
              {notes.map((n) => {
                const r = noteRect(n, viewport, zoom, NOTE_ROW_H);
                return (
                  <NoteBlock
                    key={n.noteId}
                    noteId={n.noteId}
                    x={r.x}
                    y={r.y}
                    w={r.w}
                    h={r.h}
                    velocity={n.velocity}
                    pitchLabel={pitchName(n.pitch)}
                    selected={noteSel.noteIds.includes(n.noteId)}
                    onSelect={(noteId, additive) => {
                      const a = editorStore.getState().actions;
                      if (additive) a.toggleNote(clip.clipId, noteId);
                      else a.selectNotes(clip.clipId, [noteId]);
                      studioStore
                        .getState()
                        .actions.selectNote(clip.trackId, clip.clipId, noteId);
                    }}
                  />
                );
              })}
              {notes.length === 0 && noteEntry ? (
                <span style={{ ...label, position: 'absolute', left: 8, top: 8 }}>
                  NOTE_RANGE empty for this clip — double-click to insert a note
                </span>
              ) : null}
            </div>
          </div>
          <div
            role="region"
            aria-label="Velocity lane — click or drag a bar to set velocity"
            onPointerDown={onVelocityPointer}
            onPointerMove={onVelocityPointer}
            style={{
              position: 'relative',
              height: VEL_LANE_H,
              marginTop: 4,
              background: tokens.surface,
              border: `1px solid ${tokens.border}`,
              borderRadius: tokens.radius,
              cursor: notes.length ? 'ns-resize' : 'default',
            }}
          >
            {velocityBars(notes, VEL_LANE_H).map(({ note, heightPx, velocity }) => {
              const r = noteRect(note, viewport, zoom, NOTE_ROW_H);
              return (
                <div
                  key={note.noteId}
                  aria-hidden
                  style={{
                    position: 'absolute',
                    left: r.x,
                    bottom: 0,
                    width: Math.max(2, r.w),
                    height: heightPx,
                    background: noteSel.noteIds.includes(note.noteId)
                      ? tokens.accent
                      : tokens.warn,
                  }}
                  title={`vel ${velocity}`}
                />
              );
            })}
            {notes.length === 0 ? (
              <span style={{ ...label, position: 'absolute', left: 8, top: 8 }}>
                velocity lane — no notes
              </span>
            ) : null}
          </div>
        </>
      )}
    </section>
  );
}

// ---------------------------------------------------------------------------
// shell
// ---------------------------------------------------------------------------

export function StudioShell() {
  const workspace = useEditor((s) => s.workspace);
  const projectId = useStudio((s) => s.projectId);
  const [showDev, setShowDev] = React.useState(false);
  const [showSupport, setShowSupport] = React.useState(false);

  React.useEffect(() => {
    injectVoidStyles();
    void ensureClientStarted();
  }, []);

  React.useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && !e.shiftKey && !e.altKey) {
        const ws = workspaceForShortcut(e.key);
        if (ws) {
          e.preventDefault();
          editorStore.getState().actions.setWorkspace(ws);
        }
      }
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, []);

  // Dev-time sanity: the editor store must stay view-state-only.
  React.useEffect(
    () => editorStore.subscribe((s) => assertEditorViewState(s)),
    [],
  );

  const def = WORKSPACES[workspace];

  return (
    <div
      style={{
        fontFamily: tokens.sans,
        background: tokens.bg,
        color: tokens.text,
        minHeight: '100vh',
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      <header
        style={{ display: 'flex', alignItems: 'center', gap: 16, padding: '8px 12px 0' }}
      >
        <h1 style={{ fontSize: 15, margin: 0, fontFamily: tokens.mono }}>VOID Studio</h1>
        <WorkspaceTabs
          workspaces={WORKSPACE_ORDER.map((id) => ({
            id,
            title: WORKSPACES[id].title,
            ariaLabel: WORKSPACES[id].ariaLabel,
            shortcut: `⌃${WORKSPACES[id].shortcut}`,
          }))}
          active={workspace}
          onSelect={(id) =>
            editorStore.getState().actions.setWorkspace(id as typeof workspace)
          }
        />
        <Button
          onClick={() => setShowSupport((v) => !v)}
          aria-pressed={showSupport}
          aria-label="Toggle notes and relink panels"
          style={{ marginLeft: 'auto', fontSize: 10 }}
        >
          notes·assets
        </Button>
        <HelpButton />
        <Button
          onClick={() => setShowDev((v) => !v)}
          aria-pressed={showDev}
          aria-label="Toggle developer panels"
          style={{ fontSize: 10 }}
        >
          dev
        </Button>
      </header>

      <div style={{ padding: '8px 12px' }}>
        <SharedTransportBar />
      </div>

      <main
        role="tabpanel"
        id={`workspace-panel-${workspace}`}
        aria-labelledby={`workspace-tab-${workspace}`}
        aria-label={def.ariaLabel}
        style={{
          display: 'flex',
          flex: 1,
          gap: 0,
          borderTop: `1px solid ${tokens.border}`,
        }}
      >
        {!projectId ? (
          <LauncherPanel />
        ) : (
          <>
            <TrackListColumn />
            {workspace === 'compose' ? (
              <PianoRollPane />
            ) : workspace === 'arrange' ? (
              <TimelinePane />
            ) : (
              <Panel title="Mix" style={{ flex: 1, margin: 12 }}>
                <p style={label}>
                  mixer region — channel strips and meters land with the mix workspace; the
                  developer surface below carries gain/pan/meters today
                </p>
              </Panel>
            )}
          </>
        )}
      </main>

      {projectId && showSupport ? (
        <section
          aria-label="Notes and asset recovery"
          style={{ display: 'flex', gap: 12, padding: '0 12px 12px' }}
        >
          <NotesPanel />
          <RelinkPanel />
        </section>
      ) : null}

      <SupportOverlays />

      {showDev ? (
        <section
          aria-label="Developer surface"
          style={{ borderTop: `1px solid ${tokens.border}` }}
        >
          <Dashboard />
        </section>
      ) : null}
    </div>
  );
}
