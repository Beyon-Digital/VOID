import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';

export interface TrackHeaderProps {
  trackId: string;
  name: string;
  kind?: string;
  selected?: boolean;
  muted?: boolean;
  soloed?: boolean;
  /** 0..1, from SetTrackGainOp applied through the engine — display only. */
  gain?: number;
  /** -1..1 */
  pan?: number;
  disabled?: boolean;
  onSelect?: (trackId: string, additive: boolean) => void;
  onToggleMute?: (trackId: string, muted: boolean) => void;
  onToggleSolo?: (trackId: string, soloed: boolean) => void;
}

/**
 * Track-list header row: select via click/keyboard, M/S toggles real buttons.
 * The header only shows view state; authoritative values come from read views.
 */
export const TrackHeader: React.FC<TrackHeaderProps> = React.memo(
  ({
    trackId,
    name,
    kind,
    selected = false,
    muted = false,
    soloed = false,
    gain,
    pan,
    disabled = false,
    onSelect,
    onToggleMute,
    onToggleSolo,
  }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const parts = [
      name,
      kind ? `kind ${kind}` : null,
      muted ? 'muted' : null,
      soloed ? 'soloed' : null,
      gain !== undefined ? `gain ${gain.toFixed(2)}` : null,
      pan !== undefined ? `pan ${pan.toFixed(2)}` : null,
      disabled ? 'disabled' : null,
    ].filter(Boolean);

    const toggleBtn = (
      id: string,
      text: string,
      on: boolean,
      color: string,
      onClick?: (trackId: string, on: boolean) => void,
    ) => (
      <button
        key={id}
        type="button"
        aria-pressed={on}
        aria-label={`${text === 'M' ? 'Mute' : 'Solo'} track ${name}`}
        onClick={(e) => {
          e.stopPropagation();
          onClick?.(trackId, !on);
        }}
        className={`${focusClass} ${animatedClass}`}
        style={{
          width: 22,
          height: 18,
          fontSize: 9,
          fontFamily: tokens.mono,
          fontWeight: 700,
          borderRadius: 3,
          border: `1px solid ${on ? color : tokens.border}`,
          background: on ? color : 'transparent',
          color: on ? '#0b0d14' : tokens.textSecondary,
          cursor: 'pointer',
          padding: 0,
        }}
      >
        {text}
      </button>
    );

    return (
      <div
        role="button"
        tabIndex={0}
        aria-pressed={selected}
        aria-label={`Track ${parts.join(', ')}`}
        data-track-id={trackId}
        className={`${focusClass} ${animatedClass}`}
        onClick={(e) => onSelect?.(trackId, e.shiftKey || e.metaKey || e.ctrlKey)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            onSelect?.(trackId, e.shiftKey);
          }
        }}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: 8,
          padding: '6px 10px',
          minHeight: 44,
          background: selected ? tokens.accentSoft : tokens.surface,
          borderLeft: `3px solid ${selected ? tokens.accent : 'transparent'}`,
          borderBottom: `1px solid ${tokens.border}`,
          color: disabled ? tokens.textMuted : tokens.text,
          cursor: 'pointer',
          fontFamily: tokens.sans,
        }}
      >
        <div style={{ flex: 1, minWidth: 0 }}>
          <div
            style={{
              fontSize: 12,
              fontWeight: 600,
              whiteSpace: 'nowrap',
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              textDecoration: disabled ? 'line-through' : 'none',
            }}
          >
            {name}
          </div>
          {kind ? (
            <div style={{ fontSize: 10, fontFamily: tokens.mono, color: tokens.textMuted }}>{kind}</div>
          ) : null}
        </div>
        {toggleBtn('m', 'M', muted, tokens.warn, onToggleMute)}
        {toggleBtn('s', 'S', soloed, tokens.accent, onToggleSolo)}
      </div>
    );
  },
);
TrackHeader.displayName = 'TrackHeader';
