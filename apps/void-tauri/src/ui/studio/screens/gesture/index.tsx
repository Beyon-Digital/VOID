// Gesture screen — draw a phrase / tap a rhythm (S08).
//
// Raw capture stays separate from the snapped interpretation: the
// session keeps `raw` and re-renders `preview` on every transform or
// constraint change — quantize is reversible (UI-T17). Pointer capture
// + window blur release held input (UI-T17); the keyboard/piano-roll
// alternative is always linked (UI-T18). There is no camera modality
// in this build — it is shown as such, never implied (UI-T18/T32).

import * as React from 'react';
import {
  Button,
  GesturePad,
  StatusBadge,
  TapPad,
  tokens,
} from 'void-ui';
import {
  commitGesture,
  makeContourMapping,
  useStudio,
  undoGestureCommit,
} from 'void-studio';
import type { GesturePoint } from 'void-studio';
import { getClient } from '../../../client';
import {
  useProjectSummary,
  useStudioCompactContext,
} from '../../useStudioData';
import {
  Card,
  Divider,
  Eyebrow,
  Mono,
  ReasonNote,
  Row,
  Stack,
} from '../../uip4/chrome';
import {
  gestureSession,
  go,
  useFeatureStores,
  useGestureSession,
  useSelectedTrackClips,
} from '../../uip4/runtime';

const GRID_OPTIONS: { label: string; ticks: string }[] = [
  { label: '1/8', ticks: '480000' },
  { label: '1/16', ticks: '240000' },
  { label: '1/32', ticks: '120000' },
];

function StrengthSlider() {
  const transform = useGestureSession((s) => s.transform);
  const pct = Math.round((transform.quantizeStrengthPpm / 1_000_000) * 100);
  return (
    <label style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
      <Row justify="space-between">
        <ReasonNote>Quantize strength</ReasonNote>
        <Mono>{pct}%</Mono>
      </Row>
      <input
        type="range"
        min={0}
        max={100}
        value={pct}
        onChange={(e) =>
          gestureSession.getState().actions.setTransform({
            quantizeStrengthPpm: Number(e.target.value) * 10_000,
          })
        }
        aria-label="Quantize strength — reversible, re-renders the preview"
      />
      <ReasonNote>Reversible — re-renders the preview, never the raw capture.</ReasonNote>
    </label>
  );
}

function PreviewPane() {
  const phase = useGestureSession((s) => s.phase);
  const preview = useGestureSession((s) => s.preview);
  const points = useGestureSession((s) => s.points);
  const taps = useGestureSession((s) => s.taps);
  const kind = useGestureSession((s) => s.kind);
  const lastError = useGestureSession((s) => s.lastError);
  const lastCommit = useGestureSession((s) => s.lastCommit);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  const maxX = Math.max(...points.map((p) => p.x), 1);
  const maxY = Math.max(...points.map((p) => p.y), 1);
  const path =
    points.length > 1
      ? `M ${points
          .map((p) => `${(p.x / maxX) * 100} ${100 - (p.y / maxY) * 100}`)
          .join(' L ')}`
      : null;

  const commit = async () => {
    setBusy(true);
    setError(null);
    try {
      const plan = await commitGesture(
        gestureSession,
        getClient(),
        () => crypto.randomUUID(),
      );
      if (!plan) setError('nothing to commit');
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Stack gap={10} style={{ flex: 1, minHeight: 0 }}>
      <div
        style={{
          flex: 1,
          minHeight: 180,
          background: tokens.raised,
          border: `1px solid ${tokens.line}`,
          borderRadius: tokens.radius8,
          position: 'relative',
          overflow: 'hidden',
        }}
        aria-label="Gesture preview"
        role="img"
      >
        {path ? (
          <svg
            viewBox="0 0 100 100"
            preserveAspectRatio="none"
            style={{ position: 'absolute', inset: 0, width: '100%', height: '100%' }}
            aria-hidden
          >
            <path
              d={path}
              fill="none"
              stroke={tokens.textMuted}
              strokeWidth={0.6}
              vectorEffect="non-scaling-stroke"
              strokeDasharray="3 3"
            />
          </svg>
        ) : null}
        {preview.map((n) => {
          const left = (Number(BigInt(n.startTicks)) / 3_840_000) * 100;
          const w = Math.max(1.5, (Number(BigInt(n.lengthTicks)) / 3_840_000) * 100);
          const top = 100 - (n.pitch / 127) * 100;
          return (
            <div
              key={n.index}
              title={`note ${n.index + 1} — pitch ${n.pitch}, vel ${n.velocity}`}
              style={{
                position: 'absolute',
                left: `${Math.min(98, left)}%`,
                top: `${Math.max(0, top - 3)}%`,
                width: `${w}%`,
                height: 8,
                borderRadius: 3,
                background: phase === 'committed' ? tokens.accentSoft : 'rgba(180,140,255,0.25)',
                border: `1px solid ${phase === 'committed' ? tokens.accent : '#B48CFF'}`,
              }}
            />
          );
        })}
        {kind === 'rhythm' && taps.length > 0 ? (
          <div
            style={{
              position: 'absolute',
              bottom: 8,
              left: 8,
              fontFamily: tokens.mono,
              fontSize: 11,
              color: tokens.textMuted,
            }}
          >
            {taps.length} taps captured
          </div>
        ) : null}
        {phase === 'preview' ? (
          <div
            style={{
              position: 'absolute',
              top: 8,
              left: 8,
              fontFamily: tokens.sans,
              fontSize: 10,
              color: tokens.violet,
            }}
          >
            Preview only — {preview.length} notes · no changes committed
          </div>
        ) : null}
        {phase === 'committed' ? (
          <div
            style={{
              position: 'absolute',
              top: 8,
              left: 8,
              fontFamily: tokens.sans,
              fontSize: 10,
              color: tokens.accent,
            }}
          >
            Committed — {lastCommit?.noteIds.length ?? 0} notes · one undo reverts
          </div>
        ) : null}
      </div>

      {lastError || error ? (
        <StatusBadge status="error" label={error ?? lastError ?? 'failed'} />
      ) : null}

      <Row gap={6}>
        {phase === 'preview' || phase === 'committed' ? (
          <>
            {phase === 'preview' ? (
              <Button variant="primary" size="sm" loading={busy} onClick={() => void commit()}>
                Commit phrase
              </Button>
            ) : (
              <Button
                variant="secondary"
                size="sm"
                onClick={() => void undoGestureCommit(gestureSession, getClient())}
              >
                Undo commit
              </Button>
            )}
            <Button
              variant="ghost"
              size="sm"
              onClick={() => gestureSession.getState().actions.release('cancel')}
            >
              Discard
            </Button>
          </>
        ) : null}
        <Button variant="ghost" size="sm" onClick={() => go('compose')}>
          Type notes instead →
        </Button>
      </Row>
    </Stack>
  );
}

export default function GestureScreen() {
  const compact = useStudioCompactContext();
  useFeatureStores();
  const phase = useGestureSession((s) => s.phase);
  const kind = useGestureSession((s) => s.kind);
  const armed = phase === 'armed' || phase === 'capturing';
  const capturing = phase === 'capturing';
  const bpm = useProjectSummary().bpm;
  const viewport = useStudio((s) => s.viewport);
  const zoom = useStudio((s) => s.zoom);
  const clips = useSelectedTrackClips();
  const target = clips[0] ?? null;
  const points = useGestureSession((s) => s.points);
  const tapCount = useGestureSession((s) => s.taps.length);
  const [mode, setMode] = React.useState<'contour' | 'rhythm'>('contour');
  const [gridTicks, setGridTicks] = React.useState('240000');
  const padRef = React.useRef<HTMLDivElement>(null);

  // Focus loss / Escape releases the held stroke (UI-T17).
  React.useEffect(() => {
    const onBlur = () => {
      const s = gestureSession.getState();
      if (s.phase === 'capturing' || s.phase === 'armed')
        s.actions.release('focus-lost');
    };
    window.addEventListener('blur', onBlur);
    document.addEventListener('visibilitychange', onBlur);
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape' && (capturing || armed)) {
        e.stopPropagation();
        gestureSession.getState().actions.release('cancel');
      }
    };
    window.addEventListener('keydown', onKey, true);
    return () => {
      window.removeEventListener('blur', onBlur);
      document.removeEventListener('visibilitychange', onBlur);
      window.removeEventListener('keydown', onKey, true);
    };
  }, [capturing, armed]);

  const arm = (k: 'contour' | 'rhythm') => {
    if (!target) return;
    gestureSession
      .getState()
      .actions.arm(k, { trackId: target.trackId, clipId: target.clipId });
  };

  /** First input of an armed session starts capturing. */
  const ensureCapturing = () => {
    const s = gestureSession.getState();
    if (s.phase === 'armed') s.actions.beginCapture();
  };

  const endCapture = () => {
    const heightPx = padRef.current?.clientHeight ?? 1;
    if (mode === 'contour') {
      gestureSession.getState().actions.endCapture({
        mapping: makeContourMapping(viewport, zoom, 36, 96, heightPx),
        contour: { cellTicks: gridTicks, velocity: 100 },
      });
    } else {
      gestureSession.getState().actions.endCapture({
        rhythm: { bpm: bpm ?? 120, originTicks: '0', pitch: 60 },
      });
    }
  };

  const pad = (
    <div
      ref={padRef}
      onPointerCancel={() => {
        const s = gestureSession.getState();
        if (s.phase === 'capturing') s.actions.release('cancel');
      }}
      style={{ flex: 1, minHeight: 200, display: 'flex' }}
    >
      {mode === 'contour' ? (
        <GesturePad
          armed={armed}
          capturing={capturing}
          preview={points}
          onArm={() => arm('contour')}
          onDisarm={() => gestureSession.getState().actions.release('cancel')}
          onPoint={(p: GesturePoint) => {
            ensureCapturing();
            gestureSession.getState().actions.addPoint(p);
          }}
          onCapture={() => endCapture()}
          aria-label="Draw a phrase — contour pad"
        />
      ) : (
        <TapPad
          armed={armed}
          tapCount={tapCount}
          onArm={() => arm('rhythm')}
          onDisarm={() => gestureSession.getState().actions.release('cancel')}
          onTap={(tMs) => {
            ensureCapturing();
            gestureSession.getState().actions.addTap({ tMs });
          }}
          aria-label="Tap a rhythm"
        />
      )}
      {mode === 'rhythm' && capturing ? (
        <div style={{ marginTop: 8 }}>
          <Button variant="secondary" size="sm" onClick={() => endCapture()}>
            Done tapping — show preview
          </Button>
        </div>
      ) : null}
    </div>
  );

  const leftRail = (
    <Stack gap={10}>
      <Eyebrow>Gesture</Eyebrow>
      <Card>
        <Eyebrow>Capture mode</Eyebrow>
        <Row gap={6}>
          <Button
            variant={mode === 'contour' ? 'secondary' : 'ghost'}
            size="sm"
            aria-pressed={mode === 'contour'}
            onClick={() => setMode('contour')}
          >
            Draw a phrase
          </Button>
          <Button
            variant={mode === 'rhythm' ? 'secondary' : 'ghost'}
            size="sm"
            aria-pressed={mode === 'rhythm'}
            onClick={() => setMode('rhythm')}
          >
            Tap a rhythm
          </Button>
        </Row>
        <Button variant="ghost" size="sm" disabled title="No XY-expression modality in this build">
          XY expression
        </Button>
        <ReasonNote>
          Camera hand-shape capture is not in this build — and is always
          optional. Pointer, touch, and the piano roll stay available.
        </ReasonNote>
      </Card>
      <Card>
        <Eyebrow>Target</Eyebrow>
        {target ? (
          <Mono>{target.name ?? target.clipId}</Mono>
        ) : (
          <ReasonNote>Select a MIDI clip on the track to arm a gesture.</ReasonNote>
        )}
      </Card>
      <Card>
        <Eyebrow>Mapping</Eyebrow>
        <Row justify="space-between">
          <ReasonNote>Vertical</ReasonNote>
          <Mono>pitch 36–96</Mono>
        </Row>
        <Row justify="space-between">
          <ReasonNote>Horizontal</ReasonNote>
          <Mono>time</Mono>
        </Row>
        <Row justify="space-between">
          <ReasonNote>Scale</ReasonNote>
          <Mono>chromatic</Mono>
        </Row>
        <Row justify="space-between">
          <ReasonNote>Grid</ReasonNote>
          <Row gap={4}>
            {GRID_OPTIONS.map((g) => (
              <Button
                key={g.ticks}
                variant={gridTicks === g.ticks ? 'secondary' : 'ghost'}
                size="sm"
                onClick={() => {
                  setGridTicks(g.ticks);
                  gestureSession
                    .getState()
                    .actions.setTransform({ gridTicks: g.ticks });
                }}
              >
                {g.label}
              </Button>
            ))}
          </Row>
        </Row>
        <StrengthSlider />
      </Card>
      <ReasonNote>
        Raw capture is kept — snapping is a reversible render on top.
        Escape or focus loss releases the stroke without committing.
      </ReasonNote>
    </Stack>
  );

  const rightRail = (
    <Stack gap={10}>
      <Eyebrow>Interpretation</Eyebrow>
      <Card>
        <Row justify="space-between">
          <ReasonNote>Phase</ReasonNote>
          <StatusBadge
            status={
              phase === 'committed'
                ? 'saved'
                : phase === 'preview'
                  ? 'ready'
                  : capturing
                    ? 'recording'
                    : armed
                      ? 'armed'
                      : 'unavailable'
            }
            label={phase === 'idle' ? 'Not armed' : phase}
          />
        </Row>
        <Divider />
        <GestureNotesList />
      </Card>
      <Card>
        <Eyebrow>Keyboard</Eyebrow>
        <ReasonNote>
          Prefer keys? The same phrase can be typed in the piano roll —
          the pad is an alternative, not a requirement.
        </ReasonNote>
        <Button variant="secondary" size="sm" onClick={() => go('compose')}>
          Open piano roll
        </Button>
      </Card>
    </Stack>
  );

  const center = (
    <div
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: 'flex',
        flexDirection: 'column',
        padding: 12,
        gap: 10,
      }}
    >
      {kind === null && phase === 'idle' ? (
        <ReasonNote>
          Arm a capture mode on the left, then draw or tap. The snapped
          interpretation appears on the right — nothing is committed
          until you say so.
        </ReasonNote>
      ) : null}
      {pad}
      {phase === 'preview' || phase === 'committed' ? <PreviewPane /> : null}
    </div>
  );

  if (compact) {
    return (
      <div
        style={{
          display: 'flex',
          flexDirection: 'column',
          flex: 1,
          minHeight: 0,
          overflowY: 'auto',
          padding: 12,
          gap: 12,
        }}
      >
        {leftRail}
        {center}
        {rightRail}
      </div>
    );
  }

  return (
    <div style={{ display: 'flex', flex: 1, minWidth: 0, minHeight: 0 }}>
      <aside
        aria-label="Gesture setup"
        style={{
          width: 240,
          flexShrink: 0,
          borderRight: `1px solid ${tokens.line}`,
          overflowY: 'auto',
          padding: '12px 10px',
        }}
      >
        {leftRail}
      </aside>
      {center}
      <aside
        aria-label="Interpretation"
        style={{
          width: 260,
          flexShrink: 0,
          borderLeft: `1px solid ${tokens.line}`,
          overflowY: 'auto',
          padding: '12px 10px',
        }}
      >
        {rightRail}
      </aside>
    </div>
  );
}

function GestureNotesList() {
  const preview = useGestureSession((s) => s.preview);
  const raw = useGestureSession((s) => s.raw);
  if (preview.length === 0)
    return <ReasonNote>No preview yet — capture a gesture first.</ReasonNote>;
  return (
    <Stack gap={4}>
      <Row justify="space-between">
        <ReasonNote>Snapped</ReasonNote>
        <ReasonNote>Raw kept</ReasonNote>
      </Row>
      {preview.map((n) => (
        <Row key={n.index} justify="space-between">
          <Mono>
            {n.pitch} · {n.startTicks}
          </Mono>
          <Mono>
            {raw[n.index]?.rawPitch ?? n.rawPitch} · {n.rawStartTicks}
          </Mono>
        </Row>
      ))}
    </Stack>
  );
}
