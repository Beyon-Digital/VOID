import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';

export interface FaderProps {
  value: number;
  min?: number;
  max?: number;
  onChange?: (value: number) => void;
  height?: number;
  label?: string;
  /** Ticks per arrow press (default (max-min)/50). */
  step?: number;
  formatValue?: (value: number) => string;
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

export const Fader: React.FC<FaderProps> = React.memo(
  ({
    value,
    min = 0,
    max = 1,
    onChange,
    height = 120,
    label = 'fader',
    step,
    formatValue = (v: number) => v.toFixed(2),
  }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const range = max - min;
    const coarse = step ?? range / 50;
    const v = clamp(value, min, max);
    const percentage = ((v - min) / range) * 100;
    const interactive = !!onChange;
    const trackRef = React.useRef<HTMLDivElement | null>(null);
    const draggingRef = React.useRef(false);

    const emit = (next: number) => onChange?.(clamp(next, min, max));

    const valueAtY = (clientY: number): number => {
      const rect = trackRef.current?.getBoundingClientRect();
      if (!rect || rect.height <= 0) return v;
      const frac = clamp(1 - (clientY - rect.top) / rect.height, 0, 1);
      return min + frac * range;
    };

    return (
      <div
        ref={trackRef}
        role="slider"
        tabIndex={interactive ? 0 : -1}
        className={`${focusClass} ${animatedClass}`}
        aria-label={label}
        aria-orientation="vertical"
        aria-valuemin={min}
        aria-valuemax={max}
        aria-valuenow={Number(v.toFixed(6))}
        aria-valuetext={formatValue(v)}
        aria-disabled={!interactive || undefined}
        onKeyDown={(e) => {
          if (!interactive) return;
          const fine = e.shiftKey ? 0.1 : 1;
          let next: number | null = null;
          if (e.key === 'ArrowUp' || e.key === 'ArrowRight') next = v + coarse * fine;
          else if (e.key === 'ArrowDown' || e.key === 'ArrowLeft') next = v - coarse * fine;
          else if (e.key === 'PageUp') next = v + coarse * 10 * fine;
          else if (e.key === 'PageDown') next = v - coarse * 10 * fine;
          else if (e.key === 'Home') next = min;
          else if (e.key === 'End') next = max;
          if (next === null) return;
          e.preventDefault();
          emit(next);
        }}
        onPointerDown={(e) => {
          if (!interactive) return;
          draggingRef.current = true;
          e.currentTarget.setPointerCapture(e.pointerId);
          emit(valueAtY(e.clientY));
        }}
        onPointerMove={(e) => {
          if (draggingRef.current) emit(valueAtY(e.clientY));
        }}
        onPointerUp={(e) => {
          draggingRef.current = false;
          e.currentTarget.releasePointerCapture(e.pointerId);
        }}
        onDoubleClick={() => interactive && emit((min + max) / 2)}
        style={{
          height,
          width: 24,
          backgroundColor: tokens.bg,
          border: `1px solid ${tokens.border}`,
          borderRadius: tokens.radius,
          position: 'relative',
          display: 'flex',
          justifyContent: 'center',
          cursor: interactive ? 'ns-resize' : 'default',
          touchAction: 'none',
        }}
      >
        <div
          aria-hidden
          style={{
            position: 'absolute',
            bottom: `${percentage}%`,
            width: '100%',
            height: 12,
            backgroundColor: tokens.surface,
            borderTop: `2px solid ${tokens.accent}`,
            borderBottom: `2px solid ${tokens.border}`,
            transform: 'translateY(50%)',
            pointerEvents: 'none',
          }}
        />
      </div>
    );
  },
);

Fader.displayName = 'Fader';
