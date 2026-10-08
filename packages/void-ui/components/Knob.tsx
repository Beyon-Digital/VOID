import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';

export interface KnobProps {
  value: number;
  min?: number;
  max?: number;
  onChange?: (value: number) => void;
  label?: string;
  size?: number;
  /** Ticks added per arrow press (default (max-min)/50). */
  step?: number;
  /** Fine-step multiplier for Shift+arrow (default 0.1). */
  fineStepRatio?: number;
  formatValue?: (value: number) => string;
}

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));
const PX_PER_REV = 150; // pointer distance (px) for a full 270° sweep

export const Knob: React.FC<KnobProps> = React.memo(
  ({
    value,
    min = 0,
    max = 1,
    onChange,
    label,
    size = 40,
    step,
    fineStepRatio = 0.1,
    formatValue = (v: number) => v.toFixed(2),
  }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const range = max - min;
    const coarse = step ?? range / 50;
    const v = clamp(value, min, max);
    const rotation = ((v - min) / range) * 270 - 135;
    const interactive = !!onChange;
    const dragRef = React.useRef<{ startY: number; startValue: number } | null>(null);

    const emit = (next: number) => onChange?.(clamp(next, min, max));

    return (
      <div style={{ display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 4 }}>
        <div
          role="slider"
          tabIndex={interactive ? 0 : -1}
          className={`${focusClass} ${animatedClass}`}
          aria-label={label ?? 'knob'}
          aria-orientation="vertical"
          aria-valuemin={min}
          aria-valuemax={max}
          aria-valuenow={Number(v.toFixed(6))}
          aria-valuetext={formatValue(v)}
          aria-disabled={!interactive || undefined}
          onKeyDown={(e) => {
            if (!interactive) return;
            const fine = e.shiftKey ? fineStepRatio : 1;
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
            e.currentTarget.setPointerCapture(e.pointerId);
            dragRef.current = { startY: e.clientY, startValue: v };
          }}
          onPointerMove={(e) => {
            const drag = dragRef.current;
            if (!drag) return;
            const frac = (drag.startY - e.clientY) / PX_PER_REV;
            emit(drag.startValue + frac * range);
          }}
          onPointerUp={(e) => {
            dragRef.current = null;
            e.currentTarget.releasePointerCapture(e.pointerId);
          }}
          onDoubleClick={() => interactive && emit((min + max) / 2)}
          style={{
            width: size,
            height: size,
            borderRadius: '50%',
            backgroundColor: tokens.surface,
            border: `2px solid ${tokens.border}`,
            position: 'relative',
            cursor: interactive ? 'ns-resize' : 'default',
            touchAction: 'none',
          }}
        >
          <div
            aria-hidden
            style={{
              position: 'absolute',
              top: '4px',
              bottom: '4px',
              left: '50%',
              width: 2,
              backgroundColor: tokens.accent,
              transform: `translateX(-50%) rotate(${rotation}deg)`,
              transformOrigin: 'bottom center',
            }}
          />
        </div>
        {label && (
          <span style={{ fontSize: 10, color: tokens.textSecondary }}>{label}</span>
        )}
      </div>
    );
  },
);

Knob.displayName = 'Knob';
