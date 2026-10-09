// ArrangementPane — S01 centre column: tools row, section strip + time
// ruler, virtualized track lanes, and the contextual device dock.
// Section markers come from the arrangement feature store (view-state —
// the wire has no section ops yet, so the strip shows honest emptiness).

import * as React from 'react';
import {
  ActionButton,
  TimelineRuler,
  formatBarBeat,
  injectVoidStyles,
  tokens,
} from 'void-ui';
import { parseI64 } from 'void-client';
import {
  editorStore,
  panByPx,
  studioStore,
  ticksToPx,
  transportBarState,
  useStudio,
  useStore,
  zoomAtPx,
  type SnapDivision,
} from 'void-studio';
import { useEditor } from '../../useStudioData';
import { getClient } from '../../../client';
import {
  arrangementStore,
  automationStore,
  newGesture,
  useTrackRows,
  type TrackRow,
} from './data';
import { TRACK_ROW_H, TrackLaneRow } from './TrackLaneRow';
import { DeviceDock } from './DeviceDock';

const HEADER_W = 196;
const RULER_H = 56;
const SECTION_H = 22;

const SNAP_ORDER: SnapDivision[] = ['bar', 'beat', '1/2', '1/4', '1/8', '1/16', '1/64'];

function SnapButton() {
  const snap = useEditor((s) => s.snap);
  const next = () => {
    const i = SNAP_ORDER.indexOf(snap.division);
    const division = SNAP_ORDER[(i + 1) % SNAP_ORDER.length];
    editorStore.getState().actions.setSnap({ division });
  };
  return (
    <ActionButton
      variant="secondary"
      size="sm"
      aria-label={`Snap ${snap.enabled ? snap.division : 'off'} — click to cycle division`}
      aria-pressed={snap.enabled}
      onClick={() =>
        editorStore.getState().actions.setSnap({ enabled: !snap.enabled })
      }
      onContextMenu={(e) => {
        e.preventDefault();
        next();
      }}
      title="Click toggles snapping; right-click cycles the division"
    >
      Snap {snap.enabled ? snap.division : 'off'}
    </ActionButton>
  );
}

/** Section strip — real sections from the arrangement feature store. */
function SectionStrip(props: {
  viewport: { startTicks: string; endTicks: string };
  ticksPerPixel: number;
}) {
  const store = arrangementStore();
  const sections = useStore(store, (s) => s.sections);
  const list = React.useMemo(
    () =>
      Object.values(sections).sort(
        (a, b) => Number(parseI64(a.startTicks) - parseI64(b.startTicks)),
      ),
    [sections],
  );
  const view: Parameters<typeof ticksToPx>[1] = props.viewport as never;
  const zoom = { ticksPerPixel: props.ticksPerPixel };
  if (list.length === 0) {
    return (
      <div
        aria-label="Song sections"
        style={{
          height: SECTION_H,
          display: 'flex',
          alignItems: 'center',
          paddingLeft: 4,
          color: tokens.subtle,
          fontSize: 10,
          fontFamily: tokens.mono,
        }}
      >
        no sections — section ops aren’t in the wire yet
      </div>
    );
  }
  const span = parseI64(props.viewport.endTicks) - parseI64(props.viewport.startTicks);
  return (
    <div
      aria-label="Song sections"
      style={{ position: 'relative', height: SECTION_H }}
    >
      {list.map((s) => {
        const x = ticksToPx(s.startTicks, view, zoom);
        const w = Number(parseI64(s.lengthTicks)) / props.ticksPerPixel;
        const end = parseI64(props.viewport.startTicks) + span;
        if (parseI64(s.startTicks) > end || parseI64(s.startTicks) + parseI64(s.lengthTicks) < parseI64(props.viewport.startTicks)) return null;
        return (
          <div
            key={s.sectionId}
            title={s.name}
            style={{
              position: 'absolute',
              left: x,
              width: Math.max(8, w - 2),
              top: 0,
              height: SECTION_H - 4,
              background: tokens.accentSoft,
              border: `1px solid ${tokens.line}`,
              borderRadius: 3,
              fontSize: 10,
              fontFamily: tokens.mono,
              color: tokens.text,
              padding: '0 6px',
              overflow: 'hidden',
              whiteSpace: 'nowrap',
              textOverflow: 'ellipsis',
            }}
          >
            {s.name}
          </div>
        );
      })}
    </div>
  );
}

/** Automation mini-lane — real lanes from the automation store only. */
function AutomationStrip({ trackId, viewport, ticksPerPixel }: {
  trackId: string;
  viewport: { startTicks: string; endTicks: string };
  ticksPerPixel: number;
}) {
  const store = automationStore();
  const lanes = useStore(store, (s) => s.lanes);
  const visible = useStore(store, (s) => s.visibleByTrack[trackId]);
  const list = React.useMemo(
    () =>
      Object.values(lanes).filter(
        (l) => l.param.trackId === trackId && (visible?.includes(l.id) ?? false),
      ),
    [lanes, trackId, visible],
  );
  if (list.length === 0) return null;
  return (
    <div
      aria-label={`Automation on track ${trackId}`}
      style={{
        height: 18,
        borderTop: `1px dashed ${tokens.line}`,
        background: tokens.blueSoft,
        position: 'relative',
        overflow: 'hidden',
      }}
    >
      {list.map((lane) => (
        <div
          key={lane.id}
          title={`${lane.id} — ${lane.base.length} points`}
          style={{
            position: 'absolute',
            left: 0,
            right: 0,
            top: 0,
            bottom: 0,
            fontSize: 9,
            fontFamily: tokens.mono,
            color: tokens.blue,
            padding: '1px 6px',
          }}
        >
          {lane.id} · {lane.base.length} pts
          {/* point dots — real automation data */}
          {lane.base.map((p) => {
            const x = ticksToPx(p.ticks, viewport as never, {
              ticksPerPixel,
            });
            return (
              <span
                key={p.ticks}
                aria-hidden
                style={{
                  position: 'absolute',
                  left: x,
                  top: 8,
                  width: 3,
                  height: 3,
                  borderRadius: 2,
                  background: tokens.blue,
                }}
              />
            );
          })}
        </div>
      ))}
    </div>
  );
}

export function ArrangementPane(props: {
  compact: boolean;
  onOpenLibrary?: () => void;
  onOpenInspector?: () => void;
}) {
  React.useEffect(() => injectVoidStyles(), []);
  const { compact } = props;
  const { tracks, loaded } = useTrackRows();
  const viewport = useStudio((s) => s.viewport);
  const zoom = useStudio((s) => s.zoom);
  const attached = useStudio((s) => s.engine.attached);
  const selection = useStudio((s) => s.selection);
  const snap = useEditor((s) => s.snap);
  const clock = useStudio((s) => s.telemetry.clock);
  const [splitArmedTrack, setSplitArmedTrack] = React.useState<string | null>(null);
  const [undoBusy, setUndoBusy] = React.useState(false);
  const scrollRef = React.useRef<HTMLDivElement>(null);
  const [scrollTop, setScrollTop] = React.useState(0);
  const [viewH, setViewH] = React.useState(400);
  const [laneW, setLaneW] = React.useState(916);

  // Sync snap grid revision with the engine's tempo map (T07).
  React.useEffect(() => {
    if (clock?.tempo_map_revision && clock.tempo_map_revision !== snap.tempoMapRevision) {
      editorStore
        .getState()
        .actions.setSnap({ tempoMapRevision: clock.tempo_map_revision });
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [clock?.tempo_map_revision]);

  // Track lane width for ruler sizing.
  React.useEffect(() => {
    const el = scrollRef.current;
    if (!el || typeof ResizeObserver === 'undefined') return;
    const ro = new ResizeObserver((rs) => {
      setViewH(Math.max(60, rs[0].contentRect.height));
      setLaneW(Math.max(120, rs[0].contentRect.width - HEADER_W));
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Virtualized rows (T30): only the visible window mounts lanes.
  const { startIdx, endIdx, topPad, bottomPad } = React.useMemo(() => {
    const first = Math.max(0, Math.floor(scrollTop / TRACK_ROW_H) - 1);
    const last = Math.min(
      tracks.length,
      Math.ceil((scrollTop + viewH) / TRACK_ROW_H) + 1,
    );
    return {
      startIdx: first,
      endIdx: last,
      topPad: first * TRACK_ROW_H,
      bottomPad: Math.max(0, (tracks.length - last) * TRACK_ROW_H),
    };
  }, [scrollTop, viewH, tracks.length]);

  // Horizontal wheel = musical pan; ctrl+wheel = zoom at cursor (T02/T07 —
  // all through the viewport contract, never pixel scroll state).
  const onWheel = (e: React.WheelEvent) => {
    const dx = e.deltaX !== 0 ? e.deltaX : e.shiftKey ? e.deltaY : 0;
    if (e.ctrlKey || e.metaKey) {
      const rect = scrollRef.current?.getBoundingClientRect();
      const px = (rect ? e.clientX - rect.left : 0) - HEADER_W;
      const z = zoomAtPx(viewport, zoom, px, e.deltaY < 0 ? 1 / 1.2 : 1.2);
      if (z) studioStore.getState().actions.setTicksPerPixel(z.zoom.ticksPerPixel);
      return;
    }
    if (dx !== 0) {
      const v = panByPx(viewport, zoom, dx);
      studioStore.getState().actions.setViewport(v.startTicks, v.endTicks);
    }
  };

  const bar = transportBarState(clock, attached);
  const loopRange = bar.cycle.active
    ? {
        x: ticksToPx(bar.cycle.startTicks, viewport, zoom),
        w:
          Number(
            parseI64(bar.cycle.endTicks) - parseI64(bar.cycle.startTicks),
          ) / zoom.ticksPerPixel,
      }
    : null;

  const addTrack = () => {
    if (!attached) return;
    void getClient()
      .addTrack({
        trackId: crypto.randomUUID(),
        kind: 'MIDI',
        commandId: newGesture(),
      })
      .catch((e) => editorStore.getState().actions.setEditError(String(e)));
  };

  const undo = () => {
    if (!attached) return;
    setUndoBusy(true);
    void getClient()
      .undo(newGesture())
      .catch((e) => editorStore.getState().actions.setEditError(String(e)))
      .finally(() => setUndoBusy(false));
  };

  return (
    <section
      role="region"
      aria-label="Arrangement"
      style={{
        flex: 1,
        minWidth: 0,
        display: 'flex',
        flexDirection: 'column',
        background: tokens.bg,
      }}
    >
      {/* tools row */}
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: tokens.space8,
          height: 44,
          padding: `0 ${tokens.space12}`,
          borderBottom: `1px solid ${tokens.line}`,
          flexShrink: 0,
        }}
      >
        {compact ? (
          <>
            <ActionButton
              variant="secondary"
              size="sm"
              onClick={props.onOpenLibrary}
              aria-label="Open sound library drawer"
            >
              Library
            </ActionButton>
            <ActionButton
              variant="secondary"
              size="sm"
              onClick={props.onOpenInspector}
              aria-label="Open inspector drawer"
            >
              Inspector
            </ActionButton>
          </>
        ) : null}
        <span className="void-type-strong" style={{ color: tokens.text }}>
          Arrangement
        </span>
        <SnapButton />
        <ActionButton
          variant="secondary"
          size="sm"
          aria-pressed={false}
          title="Automation lanes: recorded lanes appear under their track"
          onClick={() => {
            // Toggle visibility for lanes of the selected track (view state).
            const st = automationStore().getState();
            const sel = selection.trackId;
            if (!sel) return;
            const ids = Object.values(st.lanes)
              .filter((l) => l.param.trackId === sel)
              .map((l) => l.id);
            for (const id of ids) {
              const on = st.visibleByTrack[sel]?.includes(id) ?? false;
              st.actions.setLaneVisible(sel, id, !on);
            }
          }}
        >
          Automation
        </ActionButton>
        <ActionButton
          variant="secondary"
          size="sm"
          onClick={undo}
          disabled={!attached || undoBusy}
          loading={undoBusy}
          aria-label="Undo last edit"
        >
          Undo
        </ActionButton>
        <ActionButton
          variant="secondary"
          size="sm"
          onClick={addTrack}
          disabled={!attached}
          aria-label="Add a MIDI track"
        >
          + Add track
        </ActionButton>
        <div style={{ marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: 6 }}>
          <span className="void-type-micro" style={{ color: tokens.subtle }}>
            zoom
          </span>
          <input
            type="range"
            aria-label="Timeline zoom"
            min={4800}
            max={480000}
            step={4800}
            value={zoom.ticksPerPixel}
            onChange={(e) =>
              studioStore
                .getState()
                .actions.setTicksPerPixel(Number(e.target.value))
            }
            style={{ width: 96 }}
          />
        </div>
      </div>

      {/* ruler row: track-list header + sections + ruler */}
      <div
        style={{
          display: 'flex',
          height: RULER_H,
          borderBottom: `1px solid ${tokens.line}`,
          flexShrink: 0,
          background: tokens.surface,
        }}
      >
        <div
          style={{
            width: HEADER_W,
            flexShrink: 0,
            display: 'flex',
            flexDirection: 'column',
            justifyContent: 'center',
            padding: `0 ${tokens.space12}`,
            borderRight: `1px solid ${tokens.line}`,
          }}
        >
          <span className="void-type-micro" style={{ color: tokens.subtle }}>
            TRACKS
          </span>
          <span className="void-type-micro" style={{ color: tokens.subtle }}>
            {loaded ? `${tracks.length} tracks` : '—'}
          </span>
        </div>
        <div style={{ flex: 1, minWidth: 0, position: 'relative' }}>
          <SectionStrip viewport={viewport} ticksPerPixel={zoom.ticksPerPixel} />
          <TimelineRuler
            startTicks={viewport.startTicks}
            ticksPerPx={zoom.ticksPerPixel}
            widthPx={laneW}
            beatsPerBar={snap.beatsPerBar}
            onSeek={attached ? (t) => void getClient().seek(t) : undefined}
            aria-label="Timeline ruler — click or arrow keys to seek"
          />
          {loopRange ? (
            <div
              aria-hidden
              title={`Cycle ${formatBarBeat(bar.cycle.startTicks)} → ${formatBarBeat(bar.cycle.endTicks)}`}
              style={{
                position: 'absolute',
                left: loopRange.x,
                width: Math.max(2, loopRange.w),
                bottom: 0,
                height: 3,
                background: tokens.accent,
              }}
            />
          ) : null}
        </div>
      </div>

      {/* virtualized track rows */}
      <div
        ref={scrollRef}
        role="list"
        aria-label="Track lanes"
        onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}
        onWheel={onWheel}
        style={{ flex: 1, minHeight: 0, overflowY: 'auto', position: 'relative' }}
      >
        {!loaded ? (
          <p
            className="void-type-micro"
            style={{ padding: tokens.space12, color: tokens.subtle }}
          >
            reading TRACK_LIST…
          </p>
        ) : tracks.length === 0 ? (
          <p
            className="void-type-micro"
            style={{ padding: tokens.space12, color: tokens.subtle }}
          >
            empty project — “Add track” creates the first lane
          </p>
        ) : (
          <>
            <div aria-hidden style={{ height: topPad }} />
            {tracks.slice(startIdx, endIdx).map((t: TrackRow) => (
              <React.Fragment key={t.id}>
                <TrackLaneRow
                  track={t}
                  viewport={viewport}
                  zoom={zoom}
                  selectedTrackId={selection.trackId}
                  splitArmedTrack={splitArmedTrack}
                  onArmSplit={setSplitArmedTrack}
                />
                <AutomationStrip
                  trackId={t.id}
                  viewport={viewport}
                  ticksPerPixel={zoom.ticksPerPixel}
                />
              </React.Fragment>
            ))}
            <div aria-hidden style={{ height: bottomPad }} />
          </>
        )}
      </div>

      {/* contextual device dock */}
      <DeviceDock compact={compact} />
    </section>
  );
}
