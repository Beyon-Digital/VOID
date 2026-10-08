import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';
import { Button } from './Button';

export interface TapPadProps {
  armed: boolean;
  /** Number of taps captured so far (view state). */
  tapCount?: number;
  /** Last inter-onset ms for display (optional). */
  lastIntervalMs?: number;
  onArm?: () => void;
  onDisarm?: () => void;
  /** A tap landed — the timestamp comes from the event time (ms). */
  onTap?: (tMs: number) => void;
  'aria-label'?: string;
}

/**
 * Tap-rhythm capture pad. Presentational: emits tap timestamps; the
 * gestures/rhythm layer converts them to raw onsets (timing retained
 * exactly — quantization is a reversible transform applied on top).
 * Enter/Space taps too (GEST-06 keyboard parity), so the pad is a real
 * button with focus styles. Requires ARM first.
 */
export const TapPad: React.FC<TapPadProps> = React.memo(
  ({
    armed,
    tapCount = 0,
    lastIntervalMs,
    onArm,
    onDisarm,
    onTap,
    'aria-label': ariaLabel = 'Tap rhythm pad',
  }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const origin = React.useRef<number | null>(null);
    const [flash, setFlash] = React.useState(false);

    const tap = () => {
      if (!armed) return;
      if (origin.current === null) origin.current = performance.now();
      onTap?.(Math.max(0, performance.now() - origin.current));
      setFlash(true);
      window.setTimeout(() => setFlash(false), 90);
    };

    return (
      <div>
        <div style={{ display: 'flex', gap: 8, marginBottom: 6, alignItems: 'center' }}>
          <Button
            variant={armed ? 'danger' : 'primary'}
            onClick={() => {
              if (armed) onDisarm?.();
              else {
                origin.current = null;
                onArm?.();
              }
            }}
            aria-pressed={armed}
            aria-label={armed ? 'Disarm tap pad' : 'Arm tap pad'}
          >
            {armed ? 'DISARM' : 'ARM'}
          </Button>
          <span
            aria-live="polite"
            style={{ fontFamily: tokens.mono, fontSize: 11, color: tokens.textSecondary }}
          >
            {armed
              ? `${tapCount} tap${tapCount === 1 ? '' : 's'}${
                  lastIntervalMs !== undefined ? ` · ${Math.round(lastIntervalMs)}ms` : ''
                }`
              : 'disarmed'}
          </span>
        </div>
        <button
          type="button"
          aria-label={ariaLabel}
          aria-disabled={!armed}
          disabled={!armed}
          onClick={tap}
          onKeyDown={(e) => {
            if (e.key === 'Enter' || e.key === ' ') {
              e.preventDefault();
              tap();
            }
          }}
          className={`${focusClass} ${animatedClass}`}
          style={{
            width: '100%',
            height: 120,
            borderRadius: tokens.radius,
            border: `2px solid ${armed ? tokens.accent : tokens.border}`,
            background: flash ? tokens.accentSoft : tokens.surface,
            color: tokens.text,
            fontFamily: tokens.mono,
            fontSize: 14,
            cursor: armed ? 'pointer' : 'not-allowed',
            transition: 'background 90ms ease',
          }}
        >
          {armed ? 'TAP' : 'ARM TO TAP'}
        </button>
      </div>
    );
  },
);
