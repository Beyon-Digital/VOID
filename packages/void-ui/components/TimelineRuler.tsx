import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';

export interface TimelineRulerProps {
  /** Viewport start in engine ticks (960000 per quarter; may be negative). */
  startTicks: bigint | number | string;
  /** Ticks represented by one pixel (zoom). */
  ticksPerPx: number;
  /** Pixel width of the ruler. */
  widthPx: number;
  /** Beats per bar (default 4). */
  beatsPerBar?: number;
  /** Playhead position in ticks — rendered as a marker when inside the view. */
  playheadTicks?: bigint | number | string | null;
  /** Optional seek callback (tick at click/drag position). */
  onSeek?: (ticks: string) => void;
  'aria-label'?: string;
}

const TICKS_PER_QUARTER = 960000n;
const BIG_TICK_STEPS = [1n, 2n, 4n, 8n, 16n, 64n, 256n, 512n, 1024n, 4096n] as const;

/** Choose a bar step (in whole bars, or fraction via subdivision) so major ticks are ≥72px apart. */
function barStep(beatsPerBar: number, ticksPerPx: number): bigint {
  const ticksPerBar = TICKS_PER_QUARTER * BigInt(beatsPerBar);
  let step = 1n;
  const subs: bigint[] = [...BIG_TICK_STEPS];
  // First try subdividing a bar.
  for (const sub of subs) {
    if (ticksPerBar / sub >= 0n && Number(ticksPerBar / sub) / ticksPerPx >= 72) {
      return ticksPerBar / sub;
    }
  }
  while (Number(ticksPerBar * step) / ticksPerPx < 72) step *= 2n;
  return ticksPerBar * step;
}

export const TimelineRuler: React.FC<TimelineRulerProps> = React.memo(
  ({
    startTicks,
    ticksPerPx,
    widthPx,
    beatsPerBar = 4,
    playheadTicks = null,
    onSeek,
    'aria-label': ariaLabel = 'Timeline ruler',
  }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const start = BigInt(startTicks);
    const span = BigInt(Math.max(1, Math.round(widthPx * ticksPerPx)));
    const end = start + span;
    const step = barStep(beatsPerBar, ticksPerPx);
    const ticksPerBar = TICKS_PER_QUARTER * BigInt(beatsPerBar);

    const majors: Array<{ x: number; tick: bigint; isBar: boolean }> = [];
    const firstTick = start - ((start % step) + step) % step;
    for (let t = firstTick; t <= end && majors.length < 512; t += step) {
      const x = Number(t - start) / ticksPerPx;
      if (x >= -1) {
        majors.push({ x, tick: t, isBar: t % ticksPerBar === 0n });
      }
    }

    const playheadX =
      playheadTicks === null || playheadTicks === undefined
        ? null
        : Number(BigInt(playheadTicks) - start) / ticksPerPx;

    const tickAtEvent = (e: React.PointerEvent<HTMLDivElement>): string => {
      const rect = e.currentTarget.getBoundingClientRect();
      const x = Math.min(Math.max(e.clientX - rect.left, 0), widthPx);
      return (start + BigInt(Math.round(x * ticksPerPx))).toString();
    };

    const handlePointer = (e: React.PointerEvent<HTMLDivElement>) => {
      if (onSeek && (e.type === 'pointerdown' || e.buttons === 1)) {
        if (e.type === 'pointerdown') e.currentTarget.setPointerCapture(e.pointerId);
        onSeek(tickAtEvent(e));
      }
    };

    return (
      <div
        role="slider"
        aria-label={ariaLabel}
        aria-orientation="horizontal"
        aria-valuemin={Number(start)}
        aria-valuemax={Number(end)}
        aria-valuenow={playheadTicks != null ? Number(playheadTicks) : undefined}
        aria-valuetext={playheadTicks != null ? `playhead at tick ${playheadTicks}` : 'no playhead'}
        aria-readonly={onSeek ? undefined : true}
        tabIndex={onSeek ? 0 : -1}
        className={`${focusClass} ${animatedClass}`}
        onPointerDown={handlePointer}
        onPointerMove={handlePointer}
        onKeyDown={(e) => {
          if (!onSeek || playheadTicks == null) return;
          const delta =
            e.key === 'ArrowRight' || e.key === 'ArrowUp'
              ? TICKS_PER_QUARTER
              : e.key === 'ArrowLeft' || e.key === 'ArrowDown'
                ? -TICKS_PER_QUARTER
                : e.key === 'PageUp'
                  ? ticksPerBar
                  : e.key === 'PageDown'
                    ? -ticksPerBar
                    : null;
          if (delta === null) return;
          e.preventDefault();
          const next = BigInt(playheadTicks) + delta;
          onSeek(next < 0n ? '0' : next.toString());
        }}
        style={{
          position: 'relative',
          height: 28,
          width: widthPx,
          background: tokens.surface,
          borderBottom: `1px solid ${tokens.border}`,
          overflow: 'hidden',
          cursor: onSeek ? 'text' : 'default',
          userSelect: 'none',
        }}
      >
        {majors.map((m) => (
          <React.Fragment key={m.tick.toString()}>
            <div
              aria-hidden
              style={{
                position: 'absolute',
                left: m.x,
                top: m.isBar ? 4 : 12,
                bottom: 0,
                width: 1,
                background: m.isBar ? tokens.textSecondary : tokens.border,
              }}
            />
            {m.isBar && (
              <span
                aria-hidden
                style={{
                  position: 'absolute',
                  left: m.x + 4,
                  top: 4,
                  fontSize: 9,
                  fontFamily: tokens.mono,
                  color: tokens.textMuted,
                }}
              >
                {m.tick < 0n ? '-' : ''}
                {(m.tick < 0n ? -m.tick : m.tick) / ticksPerBar + (m.tick < 0n ? -1n : 1n)}
              </span>
            )}
          </React.Fragment>
        ))}
        {playheadX !== null && playheadX >= 0 && playheadX <= widthPx && (
          <div
            aria-hidden
            style={{
              position: 'absolute',
              left: playheadX,
              top: 0,
              bottom: 0,
              width: 2,
              background: tokens.accent,
            }}
          />
        )}
      </div>
    );
  },
);
TimelineRuler.displayName = 'TimelineRuler';
