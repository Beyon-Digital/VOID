import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';
import { Button } from './Button';

export interface ChordRegionView {
  regionId: string;
  /** Rendered position/width in px (caller maps ticks→px). */
  x: number;
  w: number;
  symbol: string;
  voicing?: string;
}

export interface ChordRegionStripProps {
  regions: ChordRegionView[];
  /** Currently applied constraint mode — shown as a live badge. */
  constraintMode?: 'off' | 'chord' | 'scale';
  selectedRegionId?: string | null;
  onSelect?: (regionId: string) => void;
  onRemove?: (regionId: string) => void;
  'aria-label'?: string;
}

/**
 * Chord-track region strip — a lane of labeled blocks above/below the
 * timeline. Presentational: selection and removal callbacks only; the
 * harmony/track store owns overlap resolution and the constraint spec.
 * Constraint mode is a visible badge so applied harmony is never
 * implicit (TIME-05).
 */
export const ChordRegionStrip: React.FC<ChordRegionStripProps> = React.memo(
  ({
    regions,
    constraintMode = 'off',
    selectedRegionId,
    onSelect,
    onRemove,
    'aria-label': ariaLabel = 'Chord track',
  }) => {
    React.useEffect(() => injectVoidStyles(), []);
    return (
      <div
        role="listbox"
        aria-label={ariaLabel}
        aria-activedescendant={selectedRegionId ?? undefined}
        style={{
          position: 'relative',
          height: 36,
          background: tokens.surface,
          border: `1px solid ${tokens.border}`,
          borderRadius: tokens.radius,
          overflow: 'hidden',
        }}
      >
        <span
          aria-live="polite"
          style={{
            position: 'absolute',
            top: 3,
            right: 8,
            fontFamily: tokens.mono,
            fontSize: 9,
            color: constraintMode === 'off' ? tokens.textMuted : tokens.accent,
            zIndex: 2,
          }}
        >
          snap: {constraintMode.toUpperCase()}
        </span>
        {regions.map((r) => {
          const selected = r.regionId === selectedRegionId;
          return (
            <button
              key={r.regionId}
              id={r.regionId}
              role="option"
              aria-selected={selected}
              aria-label={`Chord ${r.symbol}${selected ? ' selected' : ''}`}
              onClick={() => onSelect?.(r.regionId)}
              className={`${focusClass} ${animatedClass}`}
              style={{
                position: 'absolute',
                left: r.x,
                width: Math.max(20, r.w),
                top: 4,
                bottom: 4,
                background: selected ? tokens.accentSoft : tokens.surfaceRaised,
                border: `1px solid ${selected ? tokens.accent : tokens.border}`,
                borderRadius: 3,
                color: tokens.text,
                fontFamily: tokens.mono,
                fontSize: 11,
                cursor: 'pointer',
                overflow: 'hidden',
                textOverflow: 'ellipsis',
                whiteSpace: 'nowrap',
                padding: '0 4px',
              }}
            >
              {r.symbol}
              {selected && onRemove && (
                <Button
                  variant="danger"
                  aria-label={`Remove chord ${r.symbol}`}
                  onClick={(e) => {
                    e.stopPropagation();
                    onRemove(r.regionId);
                  }}
                  style={{ padding: '0 4px', fontSize: 8, marginLeft: 4 }}
                >
                  ×
                </Button>
              )}
            </button>
          );
        })}
      </div>
    );
  },
);
