// Piano-roll surface for the proposal lifecycle (S02/S25/S26/S03/S20).
//
// Real notes come from the NOTE_RANGE view cache (locked during a
// suggestion — originals are never touched pre-acceptance). Ghost
// notes render dashed; clicking one toggles it into the partial-accept
// subset. Locked-range ghosts paint muted and are never selectable.

import * as React from 'react';
import { tokens } from 'void-ui';
import type { GhostNote } from 'void-studio/src/proposals/ghost';
import { noteRect, ticksToPx } from 'void-studio';
import type { NoteView, TickViewport, Zoom } from 'void-studio';
import { useStudio } from 'void-studio';

const ROW_H = 14;
const VEL_H = 56;
const GRID_W = 760;
const ROWS = 128;

export interface SuggestionRollProps {
  notes: NoteView[];
  ghosts: GhostNote[];
  /** Indices in the partial-accept subset (from the proposals store). */
  selectedIndices: readonly number[];
  onToggleGhost?: (index: number) => void;
  /** Show the accepted-commit divider label instead of preview hints. */
  committed?: boolean;
  'aria-label'?: string;
}

function ghostRect(
  g: GhostNote,
  v: TickViewport,
  z: Zoom,
): { x: number; y: number; w: number; h: number } {
  return {
    x: ticksToPx(g.onsetTicks, v, z),
    y: (127 - g.pitch) * ROW_H,
    w: Math.max(1, Number(BigInt(g.lengthTicks)) / z.ticksPerPixel),
    h: ROW_H,
  };
}

export const SuggestionRoll: React.FC<SuggestionRollProps> = ({
  notes,
  ghosts,
  selectedIndices,
  onToggleGhost,
  committed = false,
  'aria-label': ariaLabel = 'Piano roll — suggestion preview',
}) => {
  const viewport = useStudio((s) => s.viewport);
  const zoom = useStudio((s) => s.zoom);
  const scrollRef = React.useRef<HTMLDivElement>(null);

  // Center vertically on the median pitch once.
  React.useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const all = [...notes.map((n) => n.pitch), ...ghosts.map((g) => g.pitch)];
    const med = all.length ? all.sort((a, b) => a - b)[Math.floor(all.length / 2)] : 64;
    el.scrollTop = Math.max(0, (127 - med) * ROW_H - el.clientHeight / 2);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const proposalStartX = React.useMemo(() => {
    if (ghosts.length === 0) return null;
    const minOnset = ghosts.reduce(
      (m, g) => (BigInt(g.onsetTicks) < m ? BigInt(g.onsetTicks) : m),
      BigInt(ghosts[0].onsetTicks),
    );
    return ticksToPx(minOnset.toString(10), viewport, zoom);
  }, [ghosts, viewport, zoom]);

  const gridW = Math.max(GRID_W, proposalStartX !== null ? proposalStartX + 480 : 0);
  const selSet = new Set(selectedIndices);

  return (
    <div
      ref={scrollRef}
      role="application"
      aria-label={ariaLabel}
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        overflow: 'auto',
        position: 'relative',
        background: tokens.surface,
      }}
    >
      <div
        style={{
          position: 'relative',
          width: gridW,
          height: ROWS * ROW_H + VEL_H,
        }}
      >
        {/* octave stripes */}
        {Array.from({ length: 11 }, (_, i) => (
          <div
            key={i}
            aria-hidden
            style={{
              position: 'absolute',
              top: i * 12 * ROW_H,
              left: 0,
              right: 0,
              height: 12 * ROW_H,
              background: i % 2 === 0 ? 'transparent' : 'rgba(255,255,255,0.02)',
              borderBottom: `1px solid ${tokens.line}`,
            }}
          />
        ))}
        {/* real notes — locked while a suggestion is being weighed */}
        {notes.map((n) => {
          const r = noteRect(n, viewport, zoom, ROW_H);
          return (
            <div
              key={n.noteId}
              aria-hidden
              style={{
                position: 'absolute',
                left: r.x,
                top: r.y + 1,
                width: r.w,
                height: r.h - 2,
                borderRadius: 3,
                background: committed ? tokens.accentSoft : tokens.raised,
                border: `1px solid ${committed ? tokens.accent : tokens.line}`,
                opacity: committed ? 1 : 0.75,
              }}
            />
          );
        })}
        {/* proposal region divider */}
        {proposalStartX !== null && !committed ? (
          <div
            aria-hidden
            style={{
              position: 'absolute',
              left: proposalStartX - 6,
              top: 0,
              bottom: VEL_H,
              borderLeft: `1px dashed ${tokens.accent}`,
              opacity: 0.5,
            }}
          >
            <div
              style={{
                position: 'absolute',
                top: 4,
                left: 4,
                fontFamily: tokens.sans,
                fontSize: 10,
                color: tokens.accent,
                whiteSpace: 'nowrap',
              }}
            >
              proposed
            </div>
          </div>
        ) : null}
        {/* ghost notes — dashed, toggleable unless locked */}
        {ghosts.map((g) => {
          const r = ghostRect(g, viewport, zoom);
          const picked = selSet.has(g.index);
          return (
            <button
              key={g.index}
              type="button"
              disabled={g.locked}
              onClick={() => onToggleGhost?.(g.index)}
              aria-pressed={picked}
              aria-label={
                g.locked
                  ? `Proposed note ${g.index + 1} — locked range, cannot accept`
                  : `Proposed note ${g.index + 1} — ${picked ? 'selected for accept' : 'click to include in partial accept'}`
              }
              style={{
                position: 'absolute',
                left: r.x,
                top: r.y + 1,
                width: r.w,
                height: r.h - 2,
                borderRadius: 3,
                padding: 0,
                cursor: g.locked ? 'not-allowed' : 'pointer',
                background: g.locked
                  ? 'transparent'
                  : picked
                    ? tokens.accentSoft
                    : 'rgba(216,252,113,0.10)',
                border: `1px dashed ${
                  g.locked ? tokens.textMuted : picked ? tokens.accent : tokens.accentSoft
                }`,
                opacity: g.locked ? 0.35 : 1,
              }}
            />
          );
        })}
        {/* velocity lane */}
        <div
          aria-hidden
          style={{
            position: 'absolute',
            left: 0,
            right: 0,
            bottom: 0,
            height: VEL_H,
            borderTop: `1px solid ${tokens.line}`,
            background: tokens.surface,
          }}
        >
          {notes.map((n) => {
            const r = noteRect(n, viewport, zoom, ROW_H);
            const h = Math.max(2, (n.velocity / 127) * (VEL_H - 8));
            return (
              <div
                key={n.noteId}
                style={{
                  position: 'absolute',
                  left: r.x,
                  bottom: 4,
                  width: Math.max(2, r.w - 2),
                  height: h,
                  background: tokens.textMuted,
                  opacity: 0.6,
                }}
              />
            );
          })}
          {ghosts.map((g) => {
            const r = ghostRect(g, viewport, zoom);
            const h = Math.max(2, (g.velocity / 127) * (VEL_H - 8));
            return (
              <div
                key={g.index}
                style={{
                  position: 'absolute',
                  left: r.x,
                  bottom: 4,
                  width: Math.max(2, r.w - 2),
                  height: h,
                  background: tokens.accent,
                  opacity: g.locked ? 0.2 : selSet.has(g.index) ? 0.9 : 0.4,
                }}
              />
            );
          })}
        </div>
      </div>
    </div>
  );
};
