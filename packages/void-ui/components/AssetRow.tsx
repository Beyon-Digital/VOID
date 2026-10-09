import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';

export interface AssetRowProps {
  assetId: string;
  name: string;
  /** Type chip text — AUDIO / MIDI / PRESET (real file kind). */
  kind?: string;
  /** Right-aligned metadata — duration, format, size… (real data only). */
  meta?: string;
  selected?: boolean;
  /** File missing on disk — truthful relink state, not a style. */
  missing?: boolean;
  onSelect?: (assetId: string, additive: boolean) => void;
  onActivate?: (assetId: string) => void;
  'aria-label'?: string;
}

/**
 * Library/browser asset row (S01 Sound Library). Selection is additive-aware
 * (shift/ctrl), double-click or Enter activates (preview/insert upstream).
 */
export const AssetRow: React.FC<AssetRowProps> = React.memo(
  ({
    assetId,
    name,
    kind,
    meta,
    selected = false,
    missing = false,
    onSelect,
    onActivate,
    'aria-label': ariaLabel,
  }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const [hover, setHover] = React.useState(false);
    return (
      <div
        role="option"
        aria-selected={selected}
        tabIndex={0}
        data-asset-id={assetId}
        aria-label={
          ariaLabel ??
          `Asset ${name}${kind ? ` (${kind})` : ''}${missing ? ', file missing' : ''}${selected ? ', selected' : ''}`
        }
        className={[focusClass, animatedClass].join(' ')}
        onClick={(e) => onSelect?.(assetId, e.shiftKey || e.metaKey || e.ctrlKey)}
        onDoubleClick={() => onActivate?.(assetId)}
        onKeyDown={(e) => {
          if (e.key === 'Enter') {
            e.preventDefault();
            onActivate?.(assetId);
          }
        }}
        onMouseEnter={() => setHover(true)}
        onMouseLeave={() => setHover(false)}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: tokens.space8,
          padding: `6px ${tokens.space8}`,
          background: selected ? tokens.accentSoft : hover ? tokens.hover : 'transparent',
          borderRadius: tokens.radius8,
          cursor: 'pointer',
          fontFamily: tokens.sans,
          opacity: missing ? 0.65 : 1,
        }}
      >
        {kind ? (
          <span
            aria-hidden
            style={{
              fontSize: 9,
              fontWeight: 700,
              fontFamily: tokens.fontNumeric,
              color: missing ? tokens.danger : tokens.subtle,
              border: `1px solid ${missing ? tokens.danger : tokens.line}`,
              borderRadius: tokens.radius4,
              padding: '1px 4px',
              flexShrink: 0,
            }}
          >
            {kind}
          </span>
        ) : null}
        <span
          style={{
            flex: 1,
            minWidth: 0,
            fontSize: 12,
            color: missing ? tokens.danger : tokens.text,
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {name}
        </span>
        {missing ? (
          <span role="status" style={{ fontSize: 10, color: tokens.danger, flexShrink: 0 }}>
            missing
          </span>
        ) : meta ? (
          <span
            style={{
              fontSize: 10,
              color: tokens.subtle,
              fontFamily: tokens.fontNumeric,
              flexShrink: 0,
            }}
          >
            {meta}
          </span>
        ) : null}
      </div>
    );
  },
);
AssetRow.displayName = 'AssetRow';
