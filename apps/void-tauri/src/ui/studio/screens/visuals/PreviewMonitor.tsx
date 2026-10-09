// S09 preview monitor — the preview channel's compositing projection.
//
// The webview never holds frame pixels (they live on the native side and
// no readback bridge exists in this build). What the monitor renders is
// the REAL layer stack: order, blend, opacity, visibility — plus the last
// produced frame's identity when the engine reports one. Labeled as a
// projection so nobody mistakes it for output pixels.

import * as React from 'react';
import { tokens } from 'void-ui';
import {
  dropCountersLine,
  frameMetaLine,
  layerKindLabel,
  orderedChannelLayers,
  REASON_NO_SNAPSHOT,
} from './model';
import { useVisuals, visualsStore } from './stores';

const layerColor: Record<string, string> = {
  image: tokens.blue,
  video: tokens.violet,
  generator: tokens.mint,
};

export const PreviewMonitor: React.FC = () => {
  const layers = useVisuals((s) => s.layers);
  const order = useVisuals((s) => s.order);
  const lastFrame = useVisuals((s) => s.lastFrame.preview);
  const counters = useVisuals((s) => s.dropCounters);
  const focused = useVisuals((s) => s.focusedChannel === 'preview');
  const transition = useVisuals((s) => s.transitions.preview);

  const stack = React.useMemo(
    () => orderedChannelLayers({ layers, order }, 'preview'),
    [layers, order],
  );

  return (
    <section
      aria-label="Preview monitor"
      aria-current={focused || undefined}
      onClick={() => visualsStore.getState().actions.focusChannel('preview')}
      style={{
        flex: '1 1 0',
        minWidth: 0,
        display: 'flex',
        flexDirection: 'column',
        border: `1px solid ${focused ? tokens.accent : tokens.line}`,
        borderRadius: tokens.radius12,
        background: tokens.surface,
        overflow: 'hidden',
        cursor: 'pointer',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: tokens.space8, padding: `${tokens.space8} ${tokens.space12}` }}>
        <span style={{ fontFamily: tokens.mono, fontSize: 10, letterSpacing: '0.14em', color: tokens.textMuted }}>
          PREVIEW
        </span>
        <span style={{ fontFamily: tokens.mono, fontSize: 10, color: tokens.textMuted }}>
          {stack.length === 0 ? 'empty stack' : `${stack.length} layer${stack.length === 1 ? '' : 's'}`}
        </span>
        {transition?.inFlight ? (
          <span style={{ fontFamily: tokens.mono, fontSize: 10, color: tokens.orange }}>
            {transition.kind} {Math.round(transition.progress * 100)}%
          </span>
        ) : null}
      </div>

      <div
        role="img"
        aria-label="Preview layer stack projection"
        style={{
          flex: 1,
          minHeight: 200,
          margin: `0 ${tokens.space12}`,
          borderRadius: tokens.radius8,
          border: `1px solid ${tokens.line}`,
          background: '#000',
          position: 'relative',
          display: 'flex',
          flexDirection: 'column-reverse',
          justifyContent: 'flex-start',
          gap: 4,
          padding: tokens.space8,
          overflow: 'auto',
        }}
      >
        {stack.length === 0 ? (
          <p
            style={{
              margin: 'auto',
              alignSelf: 'center',
              textAlign: 'center',
              fontFamily: tokens.sans,
              fontSize: 12,
              color: tokens.textMuted,
              maxWidth: 340,
            }}
          >
            Preview channel is empty — no visual layers.
            <br />
            <span style={{ fontFamily: tokens.mono, fontSize: 10 }}>
              {REASON_NO_SNAPSHOT}
            </span>
          </p>
        ) : (
          stack.map((l) => (
            <div
              key={l.layerId}
              style={{
                display: 'flex',
                alignItems: 'center',
                gap: tokens.space8,
                padding: `4px ${tokens.space8}`,
                borderRadius: tokens.radius4,
                border: `1px solid ${layerColor[l.kind] ?? tokens.line}`,
                background: tokens.raised,
                opacity: l.visible ? Math.max(0.25, l.opacity) : 0.3,
              }}
            >
              <span
                style={{
                  fontFamily: tokens.mono,
                  fontSize: 9,
                  padding: '1px 5px',
                  borderRadius: tokens.radius4,
                  background: tokens.surface,
                  color: layerColor[l.kind] ?? tokens.text,
                }}
              >
                {layerKindLabel(l.kind)}
              </span>
              <span style={{ flex: 1, fontFamily: tokens.sans, fontSize: 12, color: tokens.text }}>
                {l.name}
              </span>
              <span style={{ fontFamily: tokens.mono, fontSize: 10, color: tokens.textMuted }}>
                {l.blend} · {Math.round(l.opacity * 100)}%{l.visible ? '' : ' · hidden'}
              </span>
            </div>
          ))
        )}
      </div>

      <div style={{ padding: `${tokens.space8} ${tokens.space12}`, display: 'flex', flexDirection: 'column', gap: 2 }}>
        <span style={{ fontFamily: tokens.mono, fontSize: 10, color: tokens.textMuted }}>
          {lastFrame ? frameMetaLine(lastFrame) : 'no frames rendered — native renderer not bridged to this view'}
        </span>
        <span style={{ fontFamily: tokens.mono, fontSize: 10, color: counters.droppedFrames > 0 || counters.droppedClocks > 0 ? tokens.orange : tokens.textMuted }}>
          {dropCountersLine(counters)}
        </span>
      </div>
    </section>
  );
};
