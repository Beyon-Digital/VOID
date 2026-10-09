import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';
import { IconButton } from './IconButton';

export interface DeviceSlotProps {
  /** Insert position in the chain. */
  index: number;
  /** Device display name — empty slot omits it. */
  name?: string;
  /** Device category hint (instrument / effect / utility). */
  category?: string;
  /** Powered/bypassed state — only meaningful when a device is loaded. */
  powered?: boolean;
  selected?: boolean;
  /** Slot flagged missing/failed by the engine — honest error state. */
  failed?: boolean;
  onSelect?: (index: number) => void;
  onTogglePower?: (index: number) => void;
  onRemove?: (index: number) => void;
  /** Empty slot affordance — opens the device picker upstream. */
  onAdd?: (index: number) => void;
  'aria-label'?: string;
}

/**
 * Device-chain slot (S01 inspector strip). Two states: loaded device
 * (name, power toggle, remove) or empty add-slot — never a fabricated
 * placeholder device.
 */
export const DeviceSlot: React.FC<DeviceSlotProps> = React.memo(
  ({
    index,
    name,
    category,
    powered = true,
    selected = false,
    failed = false,
    onSelect,
    onTogglePower,
    onRemove,
    onAdd,
    'aria-label': ariaLabel,
  }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const [hover, setHover] = React.useState(false);
    const empty = !name;

    if (empty) {
      return (
        <button
          type="button"
          aria-label={ariaLabel ?? `Add device at slot ${index + 1}`}
          className={[focusClass, animatedClass].join(' ')}
          onClick={() => onAdd?.(index)}
          onMouseEnter={() => setHover(true)}
          onMouseLeave={() => setHover(false)}
          style={{
            width: '100%',
            padding: `${tokens.space8} ${tokens.space12}`,
            background: hover ? tokens.hover : 'transparent',
            border: `1px dashed ${tokens.line}`,
            borderRadius: tokens.radius8,
            color: tokens.subtle,
            fontFamily: tokens.sans,
            fontSize: 12,
            textAlign: 'left',
            cursor: 'pointer',
          }}
        >
          + Add device
        </button>
      );
    }

    return (
      <div
        role="button"
        tabIndex={0}
        aria-label={
          ariaLabel ??
          `Device ${name}${category ? ` (${category})` : ''}${powered ? '' : ', bypassed'}${failed ? ', failed' : ''}${selected ? ', selected' : ''}`
        }
        aria-pressed={selected}
        className={[focusClass, animatedClass].join(' ')}
        onClick={() => onSelect?.(index)}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            onSelect?.(index);
          }
        }}
        onMouseEnter={() => setHover(true)}
        onMouseLeave={() => setHover(false)}
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: tokens.space8,
          padding: `${tokens.space8} ${tokens.space12}`,
          background: selected ? tokens.accentSoft : hover ? tokens.hover : tokens.raised,
          border: `1px solid ${selected ? tokens.accent : failed ? tokens.danger : tokens.line}`,
          borderRadius: tokens.radius8,
          cursor: 'pointer',
          opacity: powered ? 1 : 0.6,
          fontFamily: tokens.sans,
        }}
      >
        <button
          type="button"
          aria-label={`${powered ? 'Bypass' : 'Enable'} ${name}`}
          aria-pressed={!powered}
          className={focusClass}
          onClick={(e) => {
            e.stopPropagation();
            onTogglePower?.(index);
          }}
          style={{
            width: 10,
            height: 10,
            borderRadius: '50%',
            border: 'none',
            padding: 0,
            flexShrink: 0,
            cursor: 'pointer',
            background: powered ? tokens.mint : tokens.subtle,
          }}
        />
        <span
          style={{
            flex: 1,
            minWidth: 0,
            fontSize: 12,
            fontWeight: 500,
            color: failed ? tokens.danger : tokens.text,
            overflow: 'hidden',
            textOverflow: 'ellipsis',
            whiteSpace: 'nowrap',
          }}
        >
          {name}
        </span>
        {category ? (
          <span style={{ fontSize: 10, color: tokens.subtle, fontFamily: tokens.fontNumeric }}>
            {category}
          </span>
        ) : null}
        {failed ? (
          <span role="status" style={{ fontSize: 10, color: tokens.danger }}>
            failed
          </span>
        ) : null}
        {hover && onRemove ? (
          <IconButton
            icon="×"
            aria-label={`Remove ${name}`}
            size="sm"
            onClick={(e) => {
              e.stopPropagation();
              onRemove(index);
            }}
          />
        ) : null}
      </div>
    );
  },
);
DeviceSlot.displayName = 'DeviceSlot';
