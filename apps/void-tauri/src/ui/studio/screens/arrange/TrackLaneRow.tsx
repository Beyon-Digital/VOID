// TrackLaneRow — one S01 track row: TrackHeader (left column) + clip lane
// (right). Clips render the canonical TimelineClip component; all edits go
// through ClipEditor/sendWithStaleRetry with shared transaction ids — same
// command surface as the W09 editor this evolves from.

import * as React from 'react';
import { TimelineClip, TrackHeader, injectVoidStyles, tokens } from 'void-ui';
import {
  beginClipDrag,
  editorStore,
  layoutClips,
  previewClipDrag,
  pxToTicks,
  studioStore,
  ticksToPx,
  useStudio,
  type TickViewport,
  type Zoom,
} from 'void-studio';
import { parseI64 } from 'void-client';
import { getClient } from '../../../client';
import { useEditor } from '../../useStudioData';
import { clipEd, newGesture, useTrackClips, type TrackRow } from './data';

export const TRACK_ROW_H = 76;
export const CLIP_H = 62;

interface LaneDrag {
  trackId: string;
  pointerId: number;
  model: ReturnType<typeof beginClipDrag>;
}

const RANGE_PAD_BARS = 8;

export function TrackLaneRow(props: {
  track: TrackRow;
  viewport: TickViewport;
  zoom: Zoom;
  selectedTrackId: string | null;
  splitArmedTrack: string | null;
  onArmSplit: (trackId: string | null) => void;
}) {
  const { track, viewport, zoom, selectedTrackId, splitArmedTrack, onArmSplit } =
    props;
  React.useEffect(() => injectVoidStyles(), []);
  const attached = useStudio((s) => s.engine.attached);
  const clipSel = useEditor((s) => s.clipSelection);
  const drag = useEditor((s) => s.drag);
  const snap = useEditor((s) => s.snap);
  const setEditError = useEditor((s) => s.actions.setEditError);
  const setDrag = useEditor((s) => s.actions.setDrag);
  const laneRef = React.useRef<HTMLDivElement>(null);
  const dragRef = React.useRef<LaneDrag | null>(null);
  const lastPointer = React.useRef('0');

  // Bounded slice: read the viewport range plus a pad in bars (T30 — the
  // lane never holds the whole arrangement, the engine bounds the page).
  const barTicks = (BigInt(Math.max(1, snap.beatsPerBar)) * 960000n).toString(10);
  const padTicks = (BigInt(RANGE_PAD_BARS) * BigInt(barTicks)).toString(10);
  const sliceStart = (parseI64(viewport.startTicks) - parseI64(padTicks))
    .toString(10);
  const sliceEnd = (parseI64(viewport.endTicks) + parseI64(padTicks)).toString(
    10,
  );
  const { clips } = useTrackClips(track.id, sliceStart, sliceEnd);

  const laidOut = React.useMemo(() => {
    const all = clips ? layoutClips(clips, viewport, zoom) : [];
    // Cull clips entirely outside the drawn range — bounded render slice.
    return all.filter(({ x, w }) => x + w > -64 && x < 4096);
  }, [clips, viewport, zoom]);

  const reloadClips = React.useCallback(() => {
    if (!attached) return Promise.resolve();
    return clipEd()
      .loadTrackClips(studioStore, track.id, {
        startTicks: sliceStart,
        endTicks: sliceEnd,
      })
      .then(() => undefined)
      .catch((e) => setEditError(String(e)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [attached, track.id, sliceStart, sliceEnd, setEditError]);

  const ticksAt = (e: React.PointerEvent): string => {
    const rect = laneRef.current?.getBoundingClientRect();
    const x = rect ? e.clientX - rect.left : 0;
    return pxToTicks(x, viewport, zoom);
  };

  const commitDragGesture = async (lane: LaneDrag) => {
    const preview = previewClipDrag(lane.model, lastPointer.current, snap);
    const res = await clipEd().commitDrag(lane.model, preview, () => reloadClips());
    setEditError(res.errorText || null);
    setDrag(null);
    // Truth comes back from the view, not the preview.
    await reloadClips();
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
    void commitDragGesture(lane);
  };

  const beginGesture = (
    zone: 'body' | 'trim-start' | 'trim-end',
    clipId: string,
    e: React.PointerEvent<HTMLDivElement>,
  ) => {
    const clip = clips?.find((c) => c.clipId === clipId);
    if (!clip || !attached) return;
    const grabTicks = ticksAt(e);
    const mode =
      splitArmedTrack === track.id
        ? ('split' as const)
        : zone === 'body'
          ? e.altKey
            ? 'duplicate'
            : 'move'
          : zone;
    const model = beginClipDrag(clip, mode, grabTicks, newGesture(), track.id);
    lastPointer.current = grabTicks;
    if (mode === 'split') {
      const preview = previewClipDrag(model, grabTicks, snap);
      onArmSplit(null);
      void clipEd()
        .commitDrag(model, preview, () => reloadClips())
        .then((res) => {
          setEditError(res.errorText || null);
          return reloadClips();
        });
      return;
    }
    dragRef.current = { trackId: track.id, pointerId: e.pointerId, model };
    laneRef.current?.setPointerCapture?.(e.pointerId);
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
        const res = await clipEd().removeClip(clipId, tx, () => reloadClips());
        if (res.errorText) {
          setEditError(res.errorText);
          break;
        }
      }
      await reloadClips();
    })();
  };

  const splitArmed = splitArmedTrack === track.id;

  return (
    <div
      role="row"
      aria-label={`Track row ${track.name}`}
      style={{
        display: 'flex',
        height: TRACK_ROW_H,
        borderBottom: `1px solid ${tokens.line}`,
        flexShrink: 0,
      }}
    >
      <TrackHeader
        trackId={track.id}
        name={track.name}
        kind={track.kind}
        selected={selectedTrackId === track.id}
        muted={track.muted}
        soloed={track.soloed}
        onSelect={(id) => studioStore.getState().actions.selectTrack(id)}
        onToggleMute={(id, on) =>
          void getClient()
            .setTrackMute(id, on, newGesture())
            .catch((e) => setEditError(String(e)))
        }
        onToggleSolo={(id, on) =>
          void getClient()
            .setTrackSolo(id, on, newGesture())
            .catch((e) => setEditError(String(e)))
        }
      />
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
            onArmSplit(splitArmed ? null : track.id);
          }
        }}
        style={{
          position: 'relative',
          flex: 1,
          minWidth: 0,
          background: splitArmed ? tokens.accentSoft : 'transparent',
          cursor: splitArmed ? 'crosshair' : 'default',
          overflow: 'hidden',
        }}
      >
        {clips === null ? (
          <span
            className="void-type-micro"
            style={{ position: 'absolute', left: 8, top: 30, color: tokens.subtle }}
          >
            loading lane…
          </span>
        ) : (
          laidOut.map(({ clip, x, w }) => {
            const isPreview = drag?.clipId === clip.clipId;
            const pv = isPreview
              ? {
                  x: ticksToPx(drag!.startTicks, viewport, zoom),
                  w: Math.max(
                    2,
                    Number(parseI64(drag!.lengthTicks)) / zoom.ticksPerPixel,
                  ),
                }
              : { x, w };
            return (
              <TimelineClip
                key={clip.clipId}
                clipId={clip.clipId}
                name={clip.name}
                variant={
                  clip.kind === 'MIDI' ? 'midi' : clip.kind === 'AUDIO' ? 'audio' : 'audio'
                }
                kind={clip.kind}
                x={pv.x}
                w={pv.w}
                top={(TRACK_ROW_H - CLIP_H) / 2}
                height={CLIP_H}
                selected={
                  clipSel.trackId === track.id &&
                  clipSel.clipIds.includes(clip.clipId)
                }
                pending={isPreview}
                color={clip.color}
                onPointerZone={beginGesture}
                onSelect={(clipId, additive) => {
                  const a = editorStore.getState().actions;
                  if (additive) a.toggleClip(track.id, clipId);
                  else a.selectClips(track.id, [clipId]);
                  studioStore.getState().actions.selectClip(track.id, clipId);
                  studioStore.getState().actions.selectTrack(track.id);
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
    </div>
  );
}
