import React from 'react';
import { Button } from './Button';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';

export interface TransportControlsProps {
  /** Engine transport state, or '—' when detached/no snapshot yet. */
  transport: 'STOPPED' | 'PLAYING' | 'RECORDING' | 'PAUSED' | '—';
  /** Playhead position text as shipped by the engine (samples, honest). */
  positionText?: string;
  /** Cycle/loop indicator from ClockSnapshot loop bounds. */
  cycle?: { active: boolean; label?: string };
  /** A command is in flight — buttons stay enabled but show busy. */
  busy?: boolean;
  disabled?: boolean;
  onPlay?: () => void;
  onStop?: () => void;
  onPanic?: () => void;
  onToggleCycle?: () => void;
  'aria-label'?: string;
}

/**
 * The one transport bar shared by all workspaces: PLAY/STOP/PANIC issue
 * send_transport calls (the parent wires the real client); the cycle
 * indicator is display-only, driven by ClockSnapshot — SET_CYCLE and
 * SetLoopRangeOp are issued by the shell, not this component.
 */
export const TransportControls: React.FC<TransportControlsProps> = React.memo(
  ({
    transport,
    positionText,
    cycle,
    busy = false,
    disabled = false,
    onPlay,
    onStop,
    onPanic,
    onToggleCycle,
    'aria-label': ariaLabel = 'Transport',
  }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const stateColor =
      transport === 'PLAYING'
        ? tokens.ok
        : transport === 'RECORDING'
          ? tokens.danger
          : transport === '—'
            ? tokens.textMuted
            : tokens.text;
    return (
      <div
        role="toolbar"
        aria-label={ariaLabel}
        aria-busy={busy || undefined}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 8,
          padding: '6px 10px',
          background: tokens.surface,
          border: `1px solid ${tokens.border}`,
          borderRadius: tokens.radius,
          fontFamily: tokens.mono,
        }}
      >
        <Button
          variant="primary"
          disabled={disabled}
          onClick={onPlay}
          aria-label="Play"
          className={`${focusClass} ${animatedClass}`}
        >
          ▶ Play
        </Button>
        <Button
          disabled={disabled}
          onClick={onStop}
          aria-label="Stop"
          className={`${focusClass} ${animatedClass}`}
        >
          ■ Stop
        </Button>
        <Button
          variant="danger"
          disabled={disabled}
          onClick={onPanic}
          aria-label="Panic — all notes off"
          className={`${focusClass} ${animatedClass}`}
        >
          PANIC
        </Button>
        {onToggleCycle ? (
          <Button
            variant={cycle?.active ? 'primary' : 'ghost'}
            disabled={disabled}
            onClick={onToggleCycle}
            aria-pressed={cycle?.active ?? false}
            aria-label={`Loop/cycle${cycle?.active ? ' on' : ' off'}${cycle?.label ? ` ${cycle.label}` : ''}`}
          >
            ⟲ cycle
          </Button>
        ) : null}
        <span
          role="status"
          aria-live="polite"
          aria-label={`Transport state ${transport}`}
          style={{
            marginLeft: 'auto',
            fontSize: 12,
            fontWeight: 700,
            color: stateColor,
          }}
        >
          {transport}
        </span>
        {positionText !== undefined ? (
          <span
            aria-label="Playhead position"
            style={{ fontSize: 11, color: tokens.textSecondary }}
          >
            {positionText}
          </span>
        ) : null}
      </div>
    );
  },
);
TransportControls.displayName = 'TransportControls';
