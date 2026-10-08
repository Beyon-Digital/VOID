import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';
import { Button } from './Button';

export interface GesturePoint {
  /** ms since capture start. */
  tMs: number;
  /** Normalized pad coordinates 0..1 (x right, y UP = higher pitch). */
  x: number;
  y: number;
}

export interface GesturePadProps {
  /** Whether the pad is armed for capture — gestures never fire unarmed. */
  armed: boolean;
  /** True while a stroke is in progress (view state for the caller). */
  capturing?: boolean;
  /** Preview path to render beneath the stroke (normalized 0..1). */
  preview?: GesturePoint[];
  onArm?: () => void;
  onDisarm?: () => void;
  /** Stroke finished: raw normalized points, pointer-order preserved. */
  onCapture?: (points: GesturePoint[]) => void;
  /** Fired per point during a stroke (for live ghost rendering). */
  onPoint?: (point: GesturePoint) => void;
  'aria-label'?: string;
}

/**
 * Contour-drawing surface: pointer path → normalized point stream.
 * Presentational only — the gestures/session store owns quantization,
 * preview, and commit. Requires explicit ARM before capture so drawing
 * can never fire from a stray touch (gestures never steal navigation).
 * Keyboard parity lives in the step-entry editor (GEST-06).
 */
export const GesturePad: React.FC<GesturePadProps> = React.memo(
  ({
    armed,
    capturing = false,
    preview,
    onArm,
    onDisarm,
    onCapture,
    onPoint,
    'aria-label': ariaLabel = 'Melodic contour pad',
  }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const ref = React.useRef<HTMLDivElement | null>(null);
    const startRef = React.useRef(0);
    const ptsRef = React.useRef<GesturePoint[]>([]);

    const toNorm = (e: React.PointerEvent): GesturePoint | null => {
      const el = ref.current;
      if (!el) return null;
      const r = el.getBoundingClientRect();
      if (r.width <= 0 || r.height <= 0) return null;
      const x = (e.clientX - r.left) / r.width;
      const yTop = (e.clientY - r.top) / r.height;
      return {
        tMs: Math.max(0, performance.now() - startRef.current),
        x: Math.min(1, Math.max(0, x)),
        y: Math.min(1, Math.max(0, 1 - yTop)),
      };
    };

    const down = (e: React.PointerEvent<HTMLDivElement>) => {
      if (!armed) return;
      startRef.current = performance.now();
      ptsRef.current = [];
      e.currentTarget.setPointerCapture(e.pointerId);
      const p = toNorm(e);
      if (p) {
        ptsRef.current.push(p);
        onPoint?.(p);
      }
    };
    const move = (e: React.PointerEvent<HTMLDivElement>) => {
      if (!armed || !e.currentTarget.hasPointerCapture(e.pointerId)) return;
      const p = toNorm(e);
      if (p) {
        ptsRef.current.push(p);
        onPoint?.(p);
      }
    };
    const up = (e: React.PointerEvent<HTMLDivElement>) => {
      if (!armed || !e.currentTarget.hasPointerCapture(e.pointerId)) return;
      e.currentTarget.releasePointerCapture(e.pointerId);
      const points = ptsRef.current;
      ptsRef.current = [];
      if (points.length > 0) onCapture?.(points);
    };

    const path = (pts: GesturePoint[]) =>
      pts
        .map((p, i) => `${i === 0 ? 'M' : 'L'}${(p.x * 100).toFixed(2)},${((1 - p.y) * 100).toFixed(2)}`)
        .join(' ');

    return (
      <div>
        <div style={{ display: 'flex', gap: 8, marginBottom: 6 }}>
          <Button
            variant={armed ? 'danger' : 'primary'}
            onClick={armed ? onDisarm : onArm}
            aria-pressed={armed}
            aria-label={armed ? 'Disarm contour pad' : 'Arm contour pad'}
          >
            {armed ? 'DISARM' : 'ARM'}
          </Button>
          <span
            aria-live="polite"
            style={{
              fontFamily: tokens.mono,
              fontSize: 11,
              color: tokens.textSecondary,
              alignSelf: 'center',
            }}
          >
            {armed ? (capturing ? 'drawing…' : 'armed — draw a contour') : 'disarmed'}
          </span>
        </div>
        <div
          ref={ref}
          role="application"
          aria-label={ariaLabel}
          aria-disabled={!armed}
          onPointerDown={down}
          onPointerMove={move}
          onPointerUp={up}
          onPointerCancel={up}
          style={{
            position: 'relative',
            width: '100%',
            height: 180,
            background: tokens.surface,
            border: `1px solid ${armed ? tokens.accent : tokens.border}`,
            borderRadius: tokens.radius,
            touchAction: armed ? 'none' : 'auto',
            cursor: armed ? 'crosshair' : 'default',
            overflow: 'hidden',
          }}
        >
          <svg
            viewBox="0 0 100 100"
            preserveAspectRatio="none"
            style={{ position: 'absolute', inset: 0, width: '100%', height: '100%' }}
            aria-hidden="true"
          >
            {preview && preview.length > 1 && (
              <path
                d={path(preview)}
                fill="none"
                stroke={tokens.accent}
                strokeWidth={0.8}
                strokeDasharray="2 1.5"
                vectorEffect="non-scaling-stroke"
              />
            )}
          </svg>
        </div>
      </div>
    );
  },
);
