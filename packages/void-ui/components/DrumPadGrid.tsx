import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';

export interface DrumPadView {
  padId: string;
  label: string;
  /** GM pitch number shown in the corner. */
  pitch?: number;
  /** Slice/asset bound to this pad (for the badge). */
  bound?: boolean;
  /** Choke group id — pads in a group visually cluster. */
  chokeGroup?: number;
}

export interface DrumPadGridProps {
  pads: DrumPadView[];
  columns?: number;
  /** Currently flashing pad ids (recent hits). */
  active?: ReadonlySet<string>;
  onHit?: (padId: string, velocity?: number) => void;
  'aria-label'?: string;
}

/**
 * Drum pad bank — presentational. Pointer velocity is approximated by
 * how far down the pad the pointer lands (top = hard); keyboard hits
 * (Enter/Space) report velocity 100 — parity without pretending keys
 * have pressure (GEST-06).
 */
export const DrumPadGrid: React.FC<DrumPadGridProps> = React.memo(
  ({ pads, columns = 4, active, onHit, 'aria-label': ariaLabel = 'Drum pads' }) => {
    React.useEffect(() => injectVoidStyles(), []);
    return (
      <div
        role="group"
        aria-label={ariaLabel}
        style={{
          display: 'grid',
          gridTemplateColumns: `repeat(${columns}, 1fr)`,
          gap: 6,
        }}
      >
        {pads.map((pad) => {
          const isActive = active?.has(pad.padId) ?? false;
          return (
            <button
              key={pad.padId}
              type="button"
              aria-label={`Pad ${pad.label}`}
              aria-pressed={isActive}
              onPointerDown={(e) => {
                const r = e.currentTarget.getBoundingClientRect();
                const rel = r.height > 0 ? (e.clientY - r.top) / r.height : 0.5;
                const velocity = Math.round(127 * (1 - Math.min(1, Math.max(0, rel))));
                onHit?.(pad.padId, velocity);
              }}
              onKeyDown={(e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  onHit?.(pad.padId, 100);
                }
              }}
              className={`${focusClass} ${animatedClass}`}
              style={{
                height: 64,
                borderRadius: tokens.radius,
                border: `1px solid ${isActive ? tokens.accent : tokens.border}`,
                background: isActive ? tokens.accentSoft : tokens.surface,
                color: tokens.text,
                fontFamily: tokens.mono,
                fontSize: 11,
                cursor: 'pointer',
                position: 'relative',
                transition: 'background 80ms ease, border-color 80ms ease',
              }}
            >
              <span>{pad.label}</span>
              {pad.pitch !== undefined && (
                <span
                  aria-hidden="true"
                  style={{
                    position: 'absolute',
                    top: 4,
                    right: 6,
                    fontSize: 8,
                    color: tokens.textMuted,
                  }}
                >
                  {pad.pitch}
                </span>
              )}
              {pad.bound && (
                <span
                  aria-hidden="true"
                  title="Slice bound"
                  style={{
                    position: 'absolute',
                    bottom: 4,
                    right: 6,
                    width: 6,
                    height: 6,
                    borderRadius: 3,
                    background: tokens.ok,
                  }}
                />
              )}
              {pad.chokeGroup !== undefined && (
                <span
                  aria-hidden="true"
                  style={{
                    position: 'absolute',
                    bottom: 4,
                    left: 6,
                    fontSize: 8,
                    color: tokens.warn,
                  }}
                >
                  C{pad.chokeGroup}
                </span>
              )}
            </button>
          );
        })}
      </div>
    );
  },
);
