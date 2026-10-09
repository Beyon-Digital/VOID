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

export interface ChannelFaderProps {
  /** Continuous level — never quantized to steps. */
  value: number;
  min?: number;
  max?: number;
  /** Arrow-key step (default range/50 — for dB domains ≈0.6 dB). */
  step?: number;
  fineStep?: number;
  /** Reset target — double-click (unity gain for mixer faders). */
  defaultValue?: number;
  unit?: string;
  label?: string;
  height?: number;
  width?: number;
  disabled?: boolean;
  /** Optional live meter fill 0..1 rendered behind the track. */
  meter?: number;
  format?: (value: number) => string;
  onPreview?: (value: number) => void;
  onCommit?: (commit: { from: number; to: number }) => void;
  'aria-label'?: string;
}

const THUMB_H = 12;
const TRAVEL_PAD = 8;

/**
 * Mixer channel fader — a continuous vertical slider (S04). Same contracts
 * as ParameterKnob: pointer capture + lost-pointer handling, Shift=fine,
 * arrows/PageUp/Down/Home/End, wheel, numeric entry on Enter, double-click
 * reset to defaultValue, Escape cancels gesture/entry first, one gesture =
 * one undo transaction.
 */
export const ChannelFader: React.FC<ChannelFaderProps> = ({
  value,
  min = -60,
  max = 6,
  step,
  fineStep,
  defaultValue = 0,
  unit = 'dB',
  label,
  height = 160,
  width = 36,
  disabled = false,
  meter,
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
      step: step ?? (max - min) / 50,
      fineStep: fineStep ?? (step ?? (max - min) / 50) / 10,
    }),
    [min, max, step, fineStep],
  );
  const [editing, setEditing] = React.useState(false);
  const [preview, setPreview] = React.useState<number | null>(null);
  const trackRef = React.useRef<HTMLDivElement>(null);
  const shown = preview ?? value;
  const travel = height - THUMB_H - TRAVEL_PAD * 2;

  const gesture = usePointerGesture({
    value,
    disabled: disabled || editing,
    delta: (e, s) => {
      const track = trackRef.current;
      if (!track) return s.currentValue;
      const rect = track.getBoundingClientRect();
      const frac = 1 - (e.clientY - rect.top - TRAVEL_PAD - THUMB_H / 2) / travel;
      return clampToDomain(min + frac * (max - min), domain);
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
      commitDirect(defaultValue);
      return;
    }
    const next = applyValueKey(value, e.key, e, domain);
    if (next !== null) {
      e.preventDefault();
      onCommit?.({ from: value, to: next });
    }
  };

  const onWheel = (e: React.WheelEvent<HTMLElement>) => {
    if (disabled || editing) return;
    const next = applyValueKey(value, e.deltaY < 0 ? 'ArrowUp' : 'ArrowDown', e, domain);
    if (next !== null) {
      e.preventDefault();
      onCommit?.({ from: value, to: next });
    }
  };

  const frac = (shown - min) / (max - min || 1);
  const thumbTop = TRAVEL_PAD + (1 - frac) * travel;
  const fmt = format ?? ((v: number) => formatNumericValue(v));

  return (
    <div
      style={{
        display: 'inline-flex',
        flexDirection: 'column',
        alignItems: 'center',
        gap: tokens.space4,
        fontFamily: tokens.sans,
        opacity: disabled ? 0.5 : 1,
      }}
    >
      {editing ? (
        <ValueEntry
          aria-label={`${ariaLabel ?? label ?? 'fader'} value`}
          initialValue={value}
          domain={domain}
          unit={unit}
          onCommit={(v) => {
            setEditing(false);
            commitDirect(v);
          }}
          onCancel={() => setEditing(false)}
        />
      ) : (
        <button
          type="button"
          className={focusClass}
          aria-label={`enter numeric value for ${ariaLabel ?? label ?? 'fader'}`}
          onClick={() => !disabled && setEditing(true)}
          style={{
            background: 'transparent',
            border: 'none',
            color: tokens.muted,
            fontFamily: tokens.fontNumeric,
            fontSize: 10,
            cursor: disabled ? 'default' : 'text',
            padding: 0,
          }}
        >
          {fmt(shown)}
        </button>
      )}
      <div
        ref={trackRef}
        role="slider"
        aria-label={ariaLabel ?? label ?? 'channel level'}
        aria-orientation="vertical"
        aria-valuemin={min}
        aria-valuemax={max}
        aria-valuenow={Number(shown.toFixed(4))}
        aria-valuetext={`${fmt(shown)} ${unit}`}
        aria-disabled={disabled || undefined}
        tabIndex={disabled ? -1 : 0}
        className={[focusClass, animatedClass].join(' ')}
        onPointerDown={gesture.onPointerDown}
        onPointerMove={gesture.onPointerMove}
        onPointerUp={gesture.onPointerUp}
        onPointerCancel={gesture.onPointerCancel}
        onLostPointerCapture={gesture.onLostPointerCapture}
        onKeyDown={onKeyDown}
        onWheel={onWheel}
        onDoubleClick={() => !disabled && commitDirect(defaultValue)}
        style={{
          position: 'relative',
          width,
          height,
          background: tokens.raised,
          border: `1px solid ${tokens.line}`,
          borderRadius: tokens.radius8,
          cursor: disabled ? 'default' : 'ns-resize',
          touchAction: 'none',
          outline: 'none',
        }}
      >
        {meter !== undefined ? (
          <div
            aria-hidden
            style={{
              position: 'absolute',
              left: 3,
              right: 3,
              bottom: TRAVEL_PAD,
              height: Math.max(0, Math.min(1, meter)) * travel,
              background: tokens.mintSoft,
              borderRadius: tokens.radius4,
            }}
          />
        ) : null}
        <div
          aria-hidden
          style={{
            position: 'absolute',
            left: '50%',
            top: '10%',
            bottom: '10%',
            width: 1,
            transform: 'translateX(-0.5px)',
            background: tokens.line,
          }}
        />
        <div
          aria-hidden
          style={{
            position: 'absolute',
            left: 4,
            right: 4,
            top: thumbTop,
            height: THUMB_H,
            background: gesture.active ? tokens.accent : tokens.text,
            borderRadius: tokens.radius4,
            boxShadow: `0 1px 3px ${tokens.ink}`,
          }}
        />
      </div>
      {label ? (
        <span className="void-type-micro" style={{ color: tokens.subtle, fontSize: 10 }}>
          {label}
        </span>
      ) : null}
    </div>
  );
};
