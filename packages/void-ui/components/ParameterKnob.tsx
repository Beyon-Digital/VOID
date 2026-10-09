import React from 'react';
import {
  applyValueKey,
  clampToDomain,
  formatNumericValue,
  type ValueDomain,
} from './editing';
import { usePointerGesture } from './usePointerGesture';
import { ValueEntry } from './ValueEntry';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';

export interface ParameterKnobProps {
  value: number;
  min?: number;
  max?: number;
  /** Arrow-key step (default range/50). */
  step?: number;
  /** Fine step for Shift+arrow and Shift+drag (default step/10). */
  fineStep?: number;
  /** Reset target for double-click / Delete (default min). */
  defaultValue?: number;
  unit?: string;
  label?: string;
  size?: number;
  disabled?: boolean;
  format?: (value: number) => string;
  /** Live preview while a gesture runs (does NOT create undo steps). */
  onPreview?: (value: number) => void;
  /**
   * Single commit per completed gesture — {from,to} is exactly one undo
   * transaction. Also fires for keyboard nudges and numeric entry.
   */
  onCommit?: (commit: { from: number; to: number }) => void;
  'aria-label'?: string;
}

const ARC_START = 135;
const ARC_SWEEP = 270;
const DRAG_RANGE_PX = 150;

/**
 * Canonical continuous parameter control (S01 "AMOUNT" control family).
 * Contracts: pointer-capture drag, Shift = fine, wheel nudge, arrows +
 * Home/End, double-click reset, Enter opens numeric entry, Escape cancels
 * gesture/entry first, one gesture = one commit = one undo transaction.
 */
export const ParameterKnob: React.FC<ParameterKnobProps> = ({
  value,
  min = 0,
  max = 1,
  step,
  fineStep,
  defaultValue,
  unit,
  label,
  size = 40,
  disabled = false,
  format,
  onPreview,
  onCommit,
  'aria-label': ariaLabel,
}) => {
  React.useEffect(() => injectVoidStyles(), []);
  const domain: ValueDomain = React.useMemo(
    () => ({
      min,
      max,
      step: step ?? ((max - min) / 50 || 0.01),
      fineStep: fineStep ?? (step ?? ((max - min) / 50 || 0.01)) / 10,
    }),
    [min, max, step, fineStep],
  );
  const [editing, setEditing] = React.useState(false);
  const [preview, setPreview] = React.useState<number | null>(null);
  const dragStart = React.useRef<{ y: number; value: number } | null>(null);
  const shown = preview ?? value;
  const resetTo = defaultValue ?? min;

  const gesture = usePointerGesture({
    value,
    disabled: disabled || editing,
    delta: (e) => {
      const start = dragStart.current;
      if (!start) return value;
      const scale = e.shiftKey ? domain.fineStep / domain.step : 1;
      const range = DRAG_RANGE_PX / scale;
      return clampToDomain(start.value + ((start.y - e.clientY) / range) * (max - min), domain);
    },
    onBegin: () => setPreview(value),
    onPreview: (v) => {
      setPreview(v);
      onPreview?.(v);
    },
    onCommit: (tx) => {
      setPreview(null);
      onCommit?.(tx);
    },
    onCancel: () => setPreview(null),
  });

  const commitDirect = (to: number) => {
    const clamped = clampToDomain(to, domain);
    if (clamped !== value) onCommit?.({ from: value, to: clamped });
  };

  const onKeyDown = (e: React.KeyboardEvent<HTMLElement>) => {
    if (editing) return;
    gesture.onKeyDown(e);
    if (e.defaultPrevented) return;
    if (e.key === 'Enter') {
      e.preventDefault();
      setEditing(true);
      return;
    }
    if (e.key === 'Delete' || e.key === 'Backspace') {
      e.preventDefault();
      commitDirect(resetTo);
      return;
    }
    const next = applyValueKey(value, e.key, e, domain);
    if (next !== null) {
      e.preventDefault();
      onPreview?.(next);
      onCommit?.({ from: value, to: next });
    }
  };

  const onWheel = (e: React.WheelEvent<HTMLElement>) => {
    if (disabled || editing) return;
    const dir = e.deltaY < 0 ? 'ArrowUp' : 'ArrowDown';
    const next = applyValueKey(value, dir, e, domain);
    if (next !== null) {
      e.preventDefault();
      onCommit?.({ from: value, to: next });
    }
  };

  const frac = (shown - min) / (max - min || 1);
  const angle = ARC_START + frac * ARC_SWEEP;
  const r = size / 2 - 3;
  const cx = size / 2;
  const cy = size / 2;
  const polar = (deg: number) => {
    const rad = ((deg - 90) * Math.PI) / 180;
    return { x: cx + r * Math.cos(rad), y: cy + r * Math.sin(rad) };
  };
  const a0 = polar(ARC_START);
  const a1 = polar(angle);
  const large = ARC_SWEEP * frac > 180 ? 1 : 0;
  const fmt = format ?? ((v: number) => formatNumericValue(v));

  return (
    <div
      role="slider"
      aria-label={ariaLabel ?? label ?? 'parameter'}
      aria-valuemin={min}
      aria-valuemax={max}
      aria-valuenow={Number(shown.toFixed(4))}
      aria-valuetext={`${fmt(shown)}${unit ? ` ${unit}` : ''}`}
      aria-disabled={disabled || undefined}
      tabIndex={disabled ? -1 : 0}
      className={[focusClass, animatedClass].join(' ')}
      onPointerDown={(e) => {
        dragStart.current = { y: e.clientY, value };
        gesture.onPointerDown(e);
      }}
      onPointerMove={gesture.onPointerMove}
      onPointerUp={gesture.onPointerUp}
      onPointerCancel={gesture.onPointerCancel}
      onLostPointerCapture={gesture.onLostPointerCapture}
      onKeyDown={onKeyDown}
      onWheel={onWheel}
      onDoubleClick={() => {
        if (!disabled) commitDirect(resetTo);
      }}
      style={{
        display: 'inline-flex',
        flexDirection: 'column',
        alignItems: 'center',
        gap: 2,
        cursor: disabled ? 'default' : 'ns-resize',
        opacity: disabled ? 0.5 : 1,
        userSelect: 'none',
        touchAction: 'none',
        fontFamily: tokens.sans,
        outline: 'none',
      }}
    >
      <svg width={size} height={size} aria-hidden>
        <circle cx={cx} cy={cy} r={r} fill={tokens.raised} stroke={tokens.line} strokeWidth={1} />
        {frac > 0.001 ? (
          <path
            d={`M ${a0.x} ${a0.y} A ${r} ${r} 0 ${large} 1 ${a1.x} ${a1.y}`}
            fill="none"
            stroke={tokens.accent}
            strokeWidth={2}
            strokeLinecap="round"
          />
        ) : null}
        <line
          x1={cx}
          y1={cy}
          x2={cx + (r - 5) * Math.cos(((angle - 90) * Math.PI) / 180)}
          y2={cy + (r - 5) * Math.sin(((angle - 90) * Math.PI) / 180)}
          stroke={tokens.text}
          strokeWidth={1.5}
          strokeLinecap="round"
        />
      </svg>
      {editing ? (
        <ValueEntry
          aria-label={`${ariaLabel ?? label ?? 'parameter'} value`}
          initialValue={value}
          domain={domain}
          unit={unit}
          onCommit={(v) => {
            setEditing(false);
            commitDirect(v);
          }}
          onCancel={() => setEditing(false)}
          width={Math.max(48, size)}
        />
      ) : (
        <span
          className="void-type-micro"
          style={{ color: tokens.muted, fontFamily: tokens.fontNumeric, fontSize: 10 }}
          title={unit ? `${fmt(shown)} ${unit}` : fmt(shown)}
        >
          {fmt(shown)}
          {unit ? ` ${unit}` : ''}
        </span>
      )}
      {label ? (
        <span className="void-type-micro" style={{ color: tokens.subtle, fontSize: 10 }}>
          {label}
        </span>
      ) : null}
    </div>
  );
};
