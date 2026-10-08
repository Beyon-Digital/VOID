import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';

export interface StepCell {
  on: boolean;
  /** 1..127 when on — drives fill intensity. */
  velocity?: number;
  /** Probability 0..1 badge (probabilityPpm mapped). */
  probability?: number;
  /** Repeats>1 = ratchet marker. */
  repeats?: number;
  /** Tie to next step. */
  tie?: boolean;
}

export interface StepGridRow {
  rowId: string;
  label: string;
  cells: StepCell[];
}

export interface StepGridProps {
  rows: StepGridRow[];
  /** Focused cell for the roving-highlight editor. */
  focus?: { row: number; step: number } | null;
  onToggle?: (row: number, step: number) => void;
  onFocus?: (row: number, step: number) => void;
  'aria-label'?: string;
}

/**
 * Drum step grid — presentational. Each cell is a real toggle button
 * (aria-pressed) so the whole grid is keyboard/screen-reader operable
 * (GEST-06). Editing semantics (row-scoped steps, tie, ratchet,
 * deterministic probability) live in void-studio patterns/stepPattern.
 */
export const StepGrid: React.FC<StepGridProps> = React.memo(
  ({ rows, focus, onToggle, onFocus, 'aria-label': ariaLabel = 'Step pattern grid' }) => {
    React.useEffect(() => injectVoidStyles(), []);
    return (
      <div role="grid" aria-label={ariaLabel} style={{ display: 'inline-block' }}>
        {rows.map((row, ri) => (
          <div
            key={row.rowId}
            role="row"
            style={{ display: 'flex', gap: 2, marginBottom: 2, alignItems: 'center' }}
          >
            <span
              style={{
                width: 64,
                fontFamily: tokens.mono,
                fontSize: 11,
                color: tokens.textSecondary,
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
              }}
            >
              {row.label}
            </span>
            {row.cells.map((cell, si) => {
              const focused = focus?.row === ri && focus?.step === si;
              const intensity = cell.on ? 0.35 + 0.65 * ((cell.velocity ?? 100) / 127) : 0;
              return (
                <button
                  key={si}
                  type="button"
                  role="gridcell"
                  aria-pressed={cell.on}
                  aria-label={`${row.label} step ${si + 1}${cell.on ? ' on' : ' off'}`}
                  tabIndex={focus ? (focused ? 0 : -1) : 0}
                  onClick={() => {
                    onFocus?.(ri, si);
                    onToggle?.(ri, si);
                  }}
                  onFocus={() => onFocus?.(ri, si)}
                  className={`${focusClass} ${animatedClass}`}
                  style={{
                    width: 22,
                    height: 26,
                    padding: 0,
                    borderRadius: 3,
                    border: `1px solid ${
                      focused ? tokens.accent : si % 4 === 0 ? tokens.border : 'transparent'
                    }`,
                    background: cell.on
                      ? `rgba(106,141,255,${intensity.toFixed(2)})`
                      : si % 4 === 0
                        ? tokens.surfaceRaised
                        : tokens.surface,
                    color: tokens.textMuted,
                    fontSize: 8,
                    fontFamily: tokens.mono,
                    cursor: 'pointer',
                    position: 'relative',
                  }}
                >
                  {cell.repeats && cell.repeats > 1 ? `×${cell.repeats}` : ''}
                  {cell.tie && (
                    <span
                      aria-hidden="true"
                      style={{
                        position: 'absolute',
                        right: -3,
                        top: '50%',
                        width: 6,
                        height: 2,
                        background: tokens.accent,
                        transform: 'translateY(-50%)',
                      }}
                    />
                  )}
                </button>
              );
            })}
          </div>
        ))}
      </div>
    );
  },
);
