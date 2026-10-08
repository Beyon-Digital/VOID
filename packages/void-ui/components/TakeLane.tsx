import React from 'react';
import { focusClass, injectVoidStyles, tokens } from './styles';

export interface TakeLaneProps {
  takeId: string;
  /** Lane index inside the take folder (0 = bottom). */
  laneIndex: number;
  name?: string;
  /** Lane geometry in px (pre-computed by the editor). */
  top: number;
  height: number;
  /** Take coverage inside the comp region, as 0..1 fractions. */
  startFrac: number;
  widthFrac: number;
  /** Incomplete pass (recording interrupted) renders hatched/dimmed. */
  incomplete?: boolean;
  /** The take currently picked by a comp segment covering `atTicks`. */
  comped?: boolean;
  /** Take under the cursor for cycle-alternatives. */
  underCursor?: boolean;
  onPick?: (takeId: string) => void;
  onCycle?: (takeId: string, dir: 1 | -1) => void;
  'aria-label'?: string;
}

/**
 * One take lane in a comp folder. Presentational only — the store
 * owns which take is comped; keyboard: Enter picks, ↑/↓ cycles
 * alternatives at this lane.
 */
export const TakeLane: React.FC<TakeLaneProps> = React.memo(
  ({
    takeId,
    laneIndex,
    name,
    top,
    height,
    startFrac,
    widthFrac,
    incomplete = false,
    comped = false,
    underCursor = false,
    onPick,
    onCycle,
    'aria-label': ariaLabel,
  }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const onKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
      if (e.key === 'Enter' || e.key === ' ') {
        e.preventDefault();
        onPick?.(takeId);
      } else if (e.key === 'ArrowUp' || e.key === 'ArrowRight') {
        e.preventDefault();
        onCycle?.(takeId, 1);
      } else if (e.key === 'ArrowDown' || e.key === 'ArrowLeft') {
        e.preventDefault();
        onCycle?.(takeId, -1);
      }
    };
    return (
      <div
        role="button"
        tabIndex={0}
        aria-label={ariaLabel ?? `take ${name ?? takeId} lane ${laneIndex}`}
        aria-pressed={comped}
        className={focusClass}
        onClick={() => onPick?.(takeId)}
        onKeyDown={onKeyDown}
        style={{
          position: 'absolute',
          top,
          height,
          left: `${startFrac * 100}%`,
          width: `${widthFrac * 100}%`,
          background: comped ? tokens.accent : tokens.surface,
          border: `1px solid ${underCursor ? tokens.text : tokens.border}`,
          borderRadius: 2,
          opacity: incomplete ? 0.55 : 1,
          backgroundImage: incomplete
            ? `repeating-linear-gradient(45deg, transparent 0 4px, ${tokens.border} 4px 5px)`
            : undefined,
          cursor: 'pointer',
          overflow: 'hidden',
          whiteSpace: 'nowrap',
          fontSize: 10,
          color: comped ? tokens.bg : tokens.text,
          paddingLeft: 4,
          lineHeight: `${height}px`,
          boxSizing: 'border-box',
        }}
      >
        {name ?? takeId}
        {incomplete ? ' (partial)' : ''}
      </div>
    );
  },
);
