import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';

export interface NoteBlockProps {
  noteId: string;
  /** Px rect from the piano-roll geometry layer. */
  x: number;
  y: number;
  w: number;
  h: number;
  /** 1..127 — drives fill intensity so velocity is visible on the grid. */
  velocity?: number;
  pitchLabel?: string;
  selected?: boolean;
  pending?: boolean;
  onPointerZone?: (
    zone: 'body' | 'resize-end',
    noteId: string,
    e: React.PointerEvent<HTMLDivElement>,
  ) => void;
  onSelect?: (noteId: string, additive: boolean) => void;
  'aria-label'?: string;
}

const EDGE = 6;

/**
 * One note cell on the piano-roll grid. Presentational only; resize zone
 * is the right EDGE px (half-width for narrow notes), reported to the
 * editor layer for the resize gesture.
 */
export const NoteBlock: React.FC<NoteBlockProps> = React.memo(
  ({
    noteId,
    x,
    y,
    w,
    h,
    velocity = 100,
    pitchLabel,
    selected = false,
    pending = false,
    onPointerZone,
    onSelect,
    'aria-label': ariaLabel,
  }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const edge = Math.min(EDGE, Math.max(2, w / 2));
    const intensity = Math.min(1, Math.max(0, velocity / 127));

    return (
      <div
        role="button"
        tabIndex={0}
        data-note-id={noteId}
        aria-pressed={selected}
        aria-label={
          ariaLabel ??
          `Note ${noteId}${pitchLabel ? ` ${pitchLabel}` : ''} velocity ${velocity}${selected ? ', selected' : ''}`
        }
        aria-busy={pending || undefined}
        className={`${focusClass} ${animatedClass}`}
        onPointerDown={(e) => {
          const rect = e.currentTarget.getBoundingClientRect();
          const zone =
            rect.width - (e.clientX - rect.left) <= edge
              ? ('resize-end' as const)
              : ('body' as const);
          if (zone === 'body') onSelect?.(noteId, e.shiftKey || e.metaKey || e.ctrlKey);
          onPointerZone?.(zone, noteId, e);
        }}
        onKeyDown={(e) => {
          if (e.key === 'Enter' || e.key === ' ') {
            e.preventDefault();
            onSelect?.(noteId, e.shiftKey);
          }
        }}
        style={{
          position: 'absolute',
          left: x,
          top: y,
          width: Math.max(2, w),
          height: Math.max(2, h - 1),
          background: selected
            ? tokens.accent
            : `color-mix(in srgb, ${tokens.accent} ${Math.round(28 + 40 * intensity)}%, ${tokens.surfaceRaised})`,
          border: `1px solid ${selected ? '#fff' : tokens.accent}`,
          borderRadius: 2,
          cursor: 'grab',
          opacity: pending ? 0.55 : 1,
          userSelect: 'none',
          boxSizing: 'border-box',
        }}
      >
        <div
          aria-hidden
          style={{
            position: 'absolute',
            right: 0,
            top: 0,
            bottom: 0,
            width: edge,
            cursor: 'col-resize',
            borderLeft: '1px solid rgba(255,255,255,0.25)',
          }}
        />
      </div>
    );
  },
);
NoteBlock.displayName = 'NoteBlock';
