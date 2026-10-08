import React from 'react';
import { focusClass, injectVoidStyles, tokens } from './styles';

export type SceneCellPhase = 'stopped' | 'pending' | 'playing' | 'stopping';

export interface SceneCellProps {
  slotId: string;
  sceneId: string;
  trackId: string;
  contentKind: 'clip' | 'pattern' | 'empty';
  label?: string;
  color?: string;
  phase: SceneCellPhase;
  /** Quantize for the next launch (display only, e.g. "1 bar"). */
  quantizeLabel?: string;
  onLaunch?: (slotId: string) => void;
  onStop?: (slotId: string) => void;
  'aria-label'?: string;
}

const PHASE_COLOR: Record<SceneCellPhase, string> = {
  stopped: tokens.border,
  pending: tokens.warn,
  playing: tokens.accent,
  stopping: tokens.warn,
};

/**
 * One cell of the scene launch grid (track × scene). Presentational:
 * the launch machine lives in void-studio `scenes/`; the cell shows
 * phase and fires launch/stop intents. Keyboard: Enter launches,
 * Backspace/Delete stops.
 */
export const SceneCell: React.FC<SceneCellProps> = React.memo(
  ({
    slotId,
    sceneId,
    trackId,
    contentKind,
    label,
    color,
    phase,
    quantizeLabel,
    onLaunch,
    onStop,
    'aria-label': ariaLabel,
  }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const empty = contentKind === 'empty';
    const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        onLaunch?.(slotId);
      } else if (e.key === 'Backspace' || e.key === 'Delete') {
        e.preventDefault();
        onStop?.(slotId);
      }
    };
    return (
      <div
        role="button"
        tabIndex={empty ? -1 : 0}
        aria-label={ariaLabel ?? `scene ${sceneId} track ${trackId}`}
        aria-pressed={phase === 'playing'}
        aria-disabled={empty}
        className={focusClass}
        onClick={empty ? undefined : () => onLaunch?.(slotId)}
        onKeyDown={empty ? undefined : onKeyDown}
        style={{
          position: 'relative',
          width: '100%',
          height: '100%',
          minHeight: 28,
          background: empty ? 'transparent' : tokens.surface,
          border: `1px solid ${PHASE_COLOR[phase]}`,
          borderLeft: `4px solid ${color ?? PHASE_COLOR[phase]}`,
          borderRadius: 2,
          cursor: empty ? 'default' : 'pointer',
          opacity: empty ? 0.3 : phase === 'stopped' ? 0.85 : 1,
          boxShadow:
            phase === 'playing' ? `0 0 6px ${tokens.accent}` : undefined,
          transition: 'border-color 120ms, box-shadow 120ms, opacity 120ms',
          color: tokens.text,
          fontSize: 10,
          padding: '2px 4px',
          boxSizing: 'border-box',
          overflow: 'hidden',
          whiteSpace: 'nowrap',
          textOverflow: 'ellipsis',
        }}
      >
        {label ?? (empty ? '' : contentKind)}
        {phase === 'pending' || phase === 'stopping' ? (
          <span style={{ position: 'absolute', right: 4, opacity: 0.7 }}>
            {quantizeLabel ?? '…'}
          </span>
        ) : null}
      </div>
    );
  },
);
