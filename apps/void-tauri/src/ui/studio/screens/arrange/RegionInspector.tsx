// RegionInspector — S01 right rail (w264). Shows the selected clip's real
// properties: start/length editable through MoveClipOp/TrimClipOp, quantize
// mirrors the live snap setting; rows the wire can't answer stay '—' with
// the reason on the title attribute. "Continue phrase" hops to the compose
// workspace with the selection intact (UI-T02 continuity).

import * as React from 'react';
import { ActionButton, StatusBadge, ValueEntry, injectVoidStyles, tokens } from 'void-ui';
import {
  TICKS_PER_QUARTER,
  clipEndTicks,
  describeReceiptError,
  editorStore,
  receiptFailed,
  studioStore,
  useStudio,
} from 'void-studio';
import { parseI64 } from 'void-client';
import { useEditor } from '../../useStudioData';
import {
  clipEd,
  newGesture,
  useSelectedClip,
  useTrackPlugins,
  useTrackRows,
} from './data';

const barTicks = (beatsPerBar: number) => TICKS_PER_QUARTER * BigInt(Math.max(1, beatsPerBar));

function barOf(ticksStr: string, beatsPerBar: number): number {
  return Number(parseI64(ticksStr) / barTicks(beatsPerBar)) + 1;
}

function Row(props: { label: string; children: React.ReactNode; title?: string }) {
  return (
    <div
      title={props.title}
      style={{
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'space-between',
        gap: tokens.space8,
        padding: `4px 0`,
        borderBottom: `1px solid ${tokens.line}`,
      }}
    >
      <span className="void-type-micro" style={{ color: tokens.subtle }}>
        {props.label}
      </span>
      <span className="void-type-numeric" style={{ color: tokens.text }}>
        {props.children}
      </span>
    </div>
  );
}

const NOT_EXPOSED = 'not exposed by the protocol for this region type';

export const RegionInspector: React.FC = () => {
  React.useEffect(() => injectVoidStyles(), []);
  const clip = useSelectedClip();
  const clipSel = useEditor((s) => s.clipSelection);
  const attached = useStudio((s) => s.engine.attached);
  const snap = useEditor((s) => s.snap);
  const { tracks } = useTrackRows();
  const { slots } = useTrackPlugins(clip?.trackId ?? null);
  const track = tracks.find((t) => t.id === clip?.trackId) ?? null;
  const instrument = slots.find((s) => s.slotIndex === 0) ?? slots[0] ?? null;

  const startBar = clip ? barOf(clip.startTicks, snap.beatsPerBar) : null;
  const endBar = clip ? barOf(clipEndTicks(clip), snap.beatsPerBar) : null;
  const lengthBars = clip
    ? Number(parseI64(clip.lengthTicks)) / Number(barTicks(snap.beatsPerBar))
    : null;

  const commitBars = (kind: 'start' | 'length', bars: number) => {
    if (!clip || !attached || !Number.isFinite(bars) || bars <= 0) return;
    const tx = newGesture();
    const reload = () =>
      clipEd().loadTrackClips(studioStore, clip.trackId).then(() => undefined);
    const run =
      kind === 'start'
        ? clipEd().moveClip(
            clip.clipId,
            clip.trackId,
            (BigInt(Math.round(bars - 1)) * barTicks(snap.beatsPerBar)).toString(10),
            tx,
            reload,
          )
        : clipEd().commitDrag(
            // trim keeps the clip's start + offset, changes length
            {
              mode: 'trim-end',
              clipId: clip.clipId,
              trackId: clip.trackId,
              transactionId: tx,
              originStartTicks: clip.startTicks,
              originLengthTicks: clip.lengthTicks,
              originOffsetTicks: clip.offsetTicks,
              grabTicks: clipEndTicks(clip),
              assetId: clip.assetId,
              kind: clip.kind,
              name: clip.name,
            },
            {
              startTicks: clip.startTicks,
              lengthTicks: (
                BigInt(Math.max(1, Math.round(bars * Number(barTicks(snap.beatsPerBar)))))
              ).toString(10),
              offsetTicks: clip.offsetTicks,
              trackId: clip.trackId,
            },
            reload,
          );
    void run
      .then((res) => {
        editorStore.getState().actions.setEditError(res.errorText || null);
        return reload();
      })
      .catch((e) => editorStore.getState().actions.setEditError(String(e)));
  };

  return (
    <aside
      role="complementary"
      aria-label="Region inspector"
      style={{
        width: 264,
        flexShrink: 0,
        display: 'flex',
        flexDirection: 'column',
        gap: tokens.space12,
        padding: tokens.space16,
        background: tokens.surface,
        borderLeft: `1px solid ${tokens.line}`,
        overflowY: 'auto',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: tokens.space8 }}>
        <span className="void-type-micro" style={{ color: tokens.subtle }}>
          REGION
        </span>
        {clip ? (
          <StatusBadge status="ready" label={clip.kind ?? 'CLIP'} />
        ) : (
          <StatusBadge status="unavailable" label="none" />
        )}
      </div>

      {clip ? (
        <>
          <span
            className="void-type-title"
            style={{ color: tokens.text, wordBreak: 'break-word' }}
          >
            {clip.name ?? clip.clipId}
          </span>
          <span
            className="void-type-micro"
            style={{
              color: tokens.accent,
              background: tokens.accentSoft,
              borderRadius: 4,
              padding: '2px 6px',
              alignSelf: 'flex-start',
            }}
          >
            Bars {startBar}–{endBar}
            {track ? ` · ${track.name}` : ''}
          </span>

          <div className="void-type-micro" style={{ color: tokens.subtle }}>
            TIMING &amp; FEEL
          </div>
          <Row label="Start">
            {attached ? (
              <ValueEntry
                initialValue={startBar ?? 1}
                domain={{ min: 1, max: 9999, step: 1, fineStep: 0.25 }}
                unit="bar"
                aria-label="Region start (bars)"
                width={64}
                onCommit={(v) => commitBars('start', v)}
                onCancel={() => undefined}
              />
            ) : (
              `bar ${startBar}`
            )}
          </Row>
          <Row label="Length">
            {attached ? (
              <ValueEntry
                initialValue={lengthBars ?? 1}
                domain={{ min: 0.25, max: 9999, step: 1, fineStep: 0.25 }}
                unit="bars"
                aria-label="Region length (bars)"
                width={64}
                onCommit={(v) => commitBars('length', v)}
                onCancel={() => undefined}
              />
            ) : (
              `${lengthBars} bars`
            )}
          </Row>
          <Row
            label="Quantize"
            title="Mirrors the live snap grid — change it in the toolbar"
          >
            {snap.enabled ? snap.division : 'off'}
          </Row>
          <Row label="Strength" title={NOT_EXPOSED}>
            —
          </Row>
          <Row label="Swing" title={NOT_EXPOSED}>
            —
          </Row>

          <div className="void-type-micro" style={{ color: tokens.subtle }}>
            PITCH
          </div>
          <Row label="Transpose" title={NOT_EXPOSED}>
            —
          </Row>
          <Row label="Scale" title={NOT_EXPOSED}>
            —
          </Row>
          <Row label="Velocity" title={NOT_EXPOSED}>
            —
          </Row>

          <div className="void-type-micro" style={{ color: tokens.subtle }}>
            SIGNAL
          </div>
          <Row label="Instrument">
            {instrument?.name ?? '—'}
          </Row>
          <Row label="Output" title={NOT_EXPOSED}>
            —
          </Row>

          <div
            style={{
              marginTop: 'auto',
              padding: tokens.space12,
              background: tokens.accentSoft,
              borderRadius: tokens.radius8,
              display: 'flex',
              flexDirection: 'column',
              gap: tokens.space8,
            }}
          >
            <span className="void-type-strong" style={{ color: tokens.text }}>
              Continue phrase
            </span>
            <span className="void-type-micro" style={{ color: tokens.subtle }}>
              Opens this region in the compose workspace — the selection
              follows you.
            </span>
            <ActionButton
              size="sm"
              variant="secondary"
              onClick={() => {
                window.location.hash = '#/compose';
              }}
            >
              Open in compose
            </ActionButton>
          </div>
        </>
      ) : (
        <span className="void-type-small" style={{ color: tokens.subtle }}>
          {clipSel.clipIds.length === 0
            ? 'Select a region on the timeline.'
            : 'Region data is outside the loaded view slice.'}
        </span>
      )}
    </aside>
  );
};
