import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';

export type TimelineClipVariant = 'audio' | 'midi' | 'ghost';

export interface TimelineClipProps {
  clipId: string;
  name?: string;
  /**
   * Clip family — audio renders a waveform texture, midi a note-lane
   * texture, ghost a non-interactive proposal preview (proposal contract:
   * ghost content is never durable until accepted).
   */
  variant?: TimelineClipVariant;
  /** Freeform kind label shown under the name (e.g. "audio", "drums"). */
  kind?: string;
  /** Left edge and width in px (from the timeline geometry layer). */
  x: number;
  w: number;
  top?: number;
  height?: number;
  /** Selected is a visual variant of the same clip, not another component. */
  selected?: boolean;
  /** Optimistic edit in flight — dimmed until the receipt lands. */
  pending?: boolean;
  color?: string;
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
 * Canonical arrangement clip — one component covering the audio / midi /
 * ghost / selected variants. Ghost clips never report pointer zones (they
 * are a proposal preview, not an editable object).
 */
export const TimelineClip: React.FC<TimelineClipProps> = React.memo(
  ({
    clipId,
    name,
    variant = 'audio',
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
    const ghost = variant === 'ghost';
    const base = color ?? (variant === 'midi' ? tokens.violet : tokens.blue);

    const zoneAt = (e: React.PointerEvent<HTMLDivElement>) => {
      const rect = e.currentTarget.getBoundingClientRect();
      const px = e.clientX - rect.left;
      if (px <= handle) return 'trim-start' as const;
      if (rect.width - px <= handle) return 'trim-end' as const;
      return 'body' as const;
    };

    return (
      <div
        role={ghost ? 'img' : 'button'}
        tabIndex={ghost ? -1 : 0}
        data-clip-id={clipId}
        aria-pressed={ghost ? undefined : selected}
        aria-label={
          ariaLabel ??
          `Clip ${name ?? clipId}${kind ? ` (${kind})` : ''}${ghost ? ', ghost preview' : ''}${selected ? ', selected' : ''}${pending ? ', edit pending' : ''}`
        }
        aria-busy={pending || undefined}
        className={ghost ? animatedClass : `${focusClass} ${animatedClass}`}
        onPointerDown={(e) => {
          if (ghost) return;
          const zone = zoneAt(e);
          if (zone === 'body') onSelect?.(clipId, e.shiftKey || e.metaKey || e.ctrlKey);
          onPointerZone?.(zone, clipId, e);
        }}
        onKeyDown={(e) => {
          if (ghost) return;
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
          background: ghost
            ? 'transparent'
            : `color-mix(in srgb, ${base} ${selected ? 34 : 22}%, ${tokens.raised})`,
          border: ghost
            ? `1px dashed ${tokens.subtle}`
            : `1px solid ${selected ? tokens.accent : tokens.line}`,
          borderRadius: tokens.radius4,
          overflow: 'hidden',
          cursor: ghost ? 'default' : 'grab',
          opacity: ghost ? 0.6 : pending ? 0.55 : 1,
          transition: 'opacity 120ms ease',
          userSelect: 'none',
          boxSizing: 'border-box',
        }}
      >
        {ghost ? null : (
          <>
            <div
              aria-hidden
              style={{
                position: 'absolute',
                left: 0,
                top: 0,
                bottom: 0,
                width: handle,
                cursor: 'col-resize',
                borderRight: `1px solid ${tokens.line}`,
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
                borderLeft: `1px solid ${tokens.line}`,
              }}
            />
          </>
        )}
        <span
          aria-hidden
          style={{
            display: 'block',
            padding: `2px ${tokens.space12}`,
            fontSize: 10,
            fontFamily: tokens.fontNumeric,
            color: ghost ? tokens.subtle : tokens.text,
            whiteSpace: 'nowrap',
            overflow: 'hidden',
            textOverflow: 'ellipsis',
          }}
        >
          {name ?? clipId}
        </span>
        {variant === 'audio' ? (
          <div
            aria-hidden
            style={{
              position: 'absolute',
              left: 8,
              right: 8,
              bottom: 4,
              height: '45%',
              opacity: 0.7,
              background: `repeating-linear-gradient(90deg, ${base} 0 1px, transparent 1px 4px)`,
              maskImage:
                'linear-gradient(90deg, black, black)',
            }}
          />
        ) : null}
        {variant === 'midi' ? (
          <div
            aria-hidden
            style={{
              position: 'absolute',
              left: 8,
              right: 8,
              bottom: 4,
              height: '45%',
              opacity: 0.7,
              background: `repeating-linear-gradient(90deg, ${base} 0 8px, transparent 8px 14px)`,
            }}
          />
        ) : null}
        {kind ? (
          <span
            aria-hidden
            style={{
              display: 'block',
              padding: `0 ${tokens.space12}`,
              fontSize: 9,
              fontFamily: tokens.fontNumeric,
              color: tokens.subtle,
            }}
          >
            {kind}
          </span>
        ) : null}
      </div>
    );
  },
);
TimelineClip.displayName = 'TimelineClip';
