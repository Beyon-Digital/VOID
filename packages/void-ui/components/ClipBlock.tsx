import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';

export interface ClipBlockProps {
  clipId: string;
  name?: string;
  kind?: string;
  /** Left edge and width in px (from the timeline geometry layer). */
  x: number;
  w: number;
  top?: number;
  height?: number;
  selected?: boolean;
  /** Optimistic drag preview in flight — dimmed until the receipt lands. */
  pending?: boolean;
  color?: string;
  /** Pointer zone that grabbed the clip: 'body' | 'trim-start' | 'trim-end'. */
  onPointerZone?: (
    zone: 'body' | 'trim-start' | 'trim-end',
    clipId: string,
    e: React.PointerEvent<HTMLDivElement>,
  ) => void;
  onSelect?: (clipId: string, additive: boolean) => void;
  'aria-label'?: string;
}

const HANDLE_W = 8;

/**
 * One arrangement clip block. Presentational only — px geometry arrives
 * pre-computed; pointer zones are reported back so the editor layer can
 * start the matching gesture (move/trim). Trim zones are the outer
 * HANDLE_W px; a narrower clip splits its width in two.
 */
export const ClipBlock: React.FC<ClipBlockProps> = React.memo(
  ({
    clipId,
    name,
    kind,
    x,
    w,
    top = 0,
    height = 40,
    selected = false,
    pending = false,
    color,
    onPointerZone,
    onSelect,
    'aria-label': ariaLabel,
  }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const handle = Math.min(HANDLE_W, Math.max(2, w / 2));
    const base = color ?? tokens.accent;

    const zoneAt = (e: React.PointerEvent<HTMLDivElement>) => {
      const rect = e.currentTarget.getBoundingClientRect();
      const px = e.clientX - rect.left;
      if (px <= handle) return 'trim-start' as const;
      if (rect.width - px <= handle) return 'trim-end' as const;
      return 'body' as const;
    };

    return (
      <div
        role="button"
        tabIndex={0}
        data-clip-id={clipId}
        aria-pressed={selected}
        aria-label={
          ariaLabel ??
          `Clip ${name ?? clipId}${kind ? ` (${kind})` : ''}${selected ? ', selected' : ''}${pending ? ', edit pending' : ''}`
        }
        aria-busy={pending || undefined}
        className={`${focusClass} ${animatedClass}`}
        onPointerDown={(e) => {
          const zone = zoneAt(e);
          if (zone === 'body') onSelect?.(clipId, e.shiftKey || e.metaKey || e.ctrlKey);
          onPointerZone?.(zone, clipId, e);
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            onSelect?.(clipId, e.shiftKey);
          }
        }}
        style={{
          position: 'absolute',
          left: x,
          top,
          width: Math.max(2, w),
          height,
          background: `color-mix(in srgb, ${base} ${selected ? 34 : 22}%, ${tokens.surfaceRaised})`,
          border: `1px solid ${selected ? tokens.accent : tokens.border}`,
          borderRadius: 4,
          overflow: 'hidden',
          cursor: 'grab',
          opacity: pending ? 0.55 : 1,
          transition: 'opacity 120ms ease',
          userSelect: 'none',
          boxSizing: 'border-box',
        }}
      >
        <div
          aria-hidden
          style={{
            position: 'absolute',
            left: 0,
            top: 0,
            bottom: 0,
            width: handle,
            cursor: 'col-resize',
            borderRight: `1px solid ${tokens.border}`,
          }}
        />
        <div
          aria-hidden
          style={{
            position: 'absolute',
            right: 0,
            top: 0,
            bottom: 0,
            width: handle,
            cursor: 'col-resize',
            borderLeft: `1px solid ${tokens.border}`,
          }}
        />
        <span
          aria-hidden
          style={{
            display: 'block',
            padding: '2px 10px',
            fontSize: 10,
            fontFamily: tokens.mono,
            color: tokens.text,
            whiteSpace: 'nowrap',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
          }}
        >
          {name ?? clipId}
        </span>
        {kind ? (
          <span
            aria-hidden
            style={{
              display: 'block',
              padding: '0 10px',
              fontSize: 9,
              fontFamily: tokens.mono,
              color: tokens.textMuted,
            }}
          >
            {kind}
          </span>
        ) : null}
      </div>
    );
  },
);
ClipBlock.displayName = 'ClipBlock';
