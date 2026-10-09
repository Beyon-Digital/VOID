// S09 visual timeline — layer windows on the same tick ruler as the
// arrangement. Seeking is real: the click rides send_transport/SEEK like
// every other timeline. Layers without a trim span the whole ruler.

import * as React from 'react';
import { TimelineClip, TimelineRuler, tokens } from 'void-ui';
import { useStudio } from 'void-studio';
import { getClient } from '../../../client';
import { layerWindowTicks, orderedChannelLayers, timelineSpanTicks } from './model';
import { useVisuals, visualsStore } from './stores';

const ROW_H = 40;
const HEADER_W = 132;

const layerColor: Record<string, string> = {
  image: tokens.blue,
  video: tokens.violet,
  generator: tokens.mint,
};

export const VisualTimelinePane: React.FC = () => {
  const attached = useStudio((s) => s.engine.attached);
  const layers = useVisuals((s) => s.layers);
  const order = useVisuals((s) => s.order);
  const selected = useVisuals((s) => s.selectedLayerIds);

  const stack = React.useMemo(
    () => orderedChannelLayers({ layers, order }, 'preview'),
    [layers, order],
  );
  const spanTicks = React.useMemo(() => timelineSpanTicks(stack), [stack]);

  // Track the pane width so clip x/w lands in real pixels.
  const trackRef = React.useRef<HTMLDivElement>(null);
  const [trackW, setTrackW] = React.useState(0);
  React.useEffect(() => {
    const el = trackRef.current;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      for (const e of entries) setTrackW(e.contentRect.width);
    });
    ro.observe(el);
    setTrackW(el.clientWidth);
    return () => ro.disconnect();
  }, []);

  const ticksPerPx = trackW > 0 ? spanTicks / trackW : spanTicks / 600;
  const toPx = (ticks: number) => (trackW > 0 ? (ticks / spanTicks) * trackW : 0);

  return (
    <section
      aria-label="Visual timeline"
      style={{
        display: 'flex',
        flexDirection: 'column',
        border: `1px solid ${tokens.line}`,
        borderRadius: tokens.radius12,
        background: tokens.surface,
        overflow: 'hidden',
        flex: '0 0 auto',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center' }}>
        <div
          style={{
            width: HEADER_W,
            flex: '0 0 auto',
            padding: `0 ${tokens.space8}`,
            fontFamily: tokens.mono,
            fontSize: 10,
            letterSpacing: '0.14em',
            color: tokens.textMuted,
          }}
        >
          LAYERS
        </div>
        <div style={{ flex: 1, minWidth: 0 }}>
          <TimelineRuler
            startTicks={0}
            ticksPerPx={ticksPerPx}
            widthPx={trackW}
            aria-label="Visual timeline ruler"
            onSeek={(ticks) => {
              if (!attached) return;
              void getClient()
                .seek(Number(ticks))
                .catch(() => undefined);
            }}
          />
        </div>
      </div>

      <div ref={trackRef} style={{ position: 'relative', minHeight: 0 }}>
        {stack.length === 0 ? (
          <p
            role="status"
            style={{
              margin: 0,
              padding: `${tokens.space12} ${tokens.space12} ${tokens.space12} ${HEADER_W + 8}px`,
              fontFamily: tokens.sans,
              fontSize: 11,
              color: tokens.textMuted,
            }}
          >
            no preview layers — the visual timeline is empty
          </p>
        ) : (
          stack.map((l) => {
            const w = layerWindowTicks(l, spanTicks);
            const x = toPx(w.startTicks);
            const width = Math.max(12, toPx(w.endTicks) - x);
            return (
              <div
                key={l.layerId}
                style={{ display: 'flex', alignItems: 'center', height: ROW_H, borderTop: `1px solid ${tokens.line}` }}
              >
                <div
                  style={{
                    width: HEADER_W,
                    flex: '0 0 auto',
                    padding: `0 ${tokens.space8}`,
                    fontFamily: tokens.sans,
                    fontSize: 11,
                    color: l.visible ? tokens.text : tokens.textMuted,
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                    whiteSpace: 'nowrap',
                  }}
                >
                  {String(l.index + 1).padStart(2, '0')} · {l.name}
                </div>
                <div style={{ flex: 1, position: 'relative', height: '100%' }}>
                  <TimelineClip
                    clipId={l.layerId}
                    name={l.name}
                    variant="midi"
                    kind={l.kind}
                    x={x}
                    w={width}
                    top={6}
                    height={ROW_H - 12}
                    selected={selected.includes(l.layerId)}
                    color={layerColor[l.kind]}
                    onSelect={(id, additive) => {
                      const cur = visualsStore.getState().selectedLayerIds;
                      visualsStore
                        .getState()
                        .actions.selectLayers(additive ? [...cur, id] : [id]);
                    }}
                    aria-label={`Layer ${l.name} (${l.kind}${l.visible ? '' : ', hidden'})`}
                  />
                </div>
              </div>
            );
          })
        )}
      </div>
    </section>
  );
};
