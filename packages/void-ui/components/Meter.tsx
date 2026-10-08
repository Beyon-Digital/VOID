import React from 'react';
import { tokens } from './styles';

export interface MeterProps {
  /** 0..1 peak level from the engine's MeterFrame. */
  level: number;
  /** 0..1 RMS, optional second bar. */
  rms?: number;
  /** Accessible name, e.g. "Track 2 level". */
  label: string;
  /** Vertical (channel-strip) or horizontal orientation. */
  orientation?: 'vertical' | 'horizontal';
  /** Pixel length of the full span (major axis). */
  size?: number;
  /** true when no live MeterFrame is feeding this meter — announced as idle. */
  idle?: boolean;
  format?: (level: number) => string;
}

const clamp01 = (v: number): number => (Number.isFinite(v) ? Math.min(1, Math.max(0, v)) : 0);
const toDb = (level: number): string =>
  level <= 0 ? '-∞ dB' : `${(20 * Math.log10(level)).toFixed(1)} dB`;

const COLORS = { low: tokens.ok, mid: tokens.warn, hot: tokens.danger } as const;

function fillColor(frac: number): string {
  return frac >= 0.95 ? COLORS.hot : frac >= 0.7 ? COLORS.mid : COLORS.low;
}

/**
 * Read-only level meter (ARIA `meter`). Renders exactly the level the
 * telemetry channel delivers — it never invents animation when idle.
 */
export const Meter: React.FC<MeterProps> = React.memo(
  ({ level, rms, label, orientation = 'vertical', size = 96, idle = false, format = toDb }) => {
    const lv = clamp01(level);
    const rmsV = rms === undefined ? undefined : clamp01(rms);
    const vertical = orientation === 'vertical';
    const barColor = fillColor(lv);
    const track: React.CSSProperties = {
      position: 'relative',
      overflow: 'hidden',
      background: tokens.surfaceRaised,
      border: `1px solid ${tokens.border}`,
      borderRadius: 3,
      ...(vertical ? { width: 12, height: size } : { width: size, height: 12 }),
    };
    const fill: React.CSSProperties = {
      position: 'absolute',
      background: barColor,
      ...(vertical
        ? { bottom: 0, left: 0, right: 0, height: `${lv * 100}%` }
        : { left: 0, top: 0, bottom: 0, width: `${lv * 100}%` }),
    };
    const rmsBar: React.CSSProperties | null = rmsV === undefined
      ? null
      : {
          position: 'absolute',
          background: 'rgba(255,255,255,0.35)',
          ...(vertical
            ? { bottom: 0, left: 0, right: 0, height: `${rmsV * 100}%` }
            : { left: 0, top: 0, bottom: 0, width: `${rmsV * 100}%` }),
        };
    return (
      <div
        role="meter"
        aria-label={label}
        aria-valuemin={0}
        aria-valuemax={100}
        aria-valuenow={Math.round(lv * 100)}
        aria-valuetext={`${format(lv)}${idle ? ' (idle)' : ''}`}
        style={{ display: 'inline-flex', flexDirection: 'column', alignItems: 'center', gap: 4 }}
      >
        <div style={track}>
          {rmsBar ? <div style={rmsBar} /> : null}
          <div style={fill} />
        </div>
        <span style={{ fontSize: 10, fontFamily: tokens.mono, color: tokens.textMuted }}>
          {idle ? 'idle' : format(lv)}
        </span>
      </div>
    );
  },
);
Meter.displayName = 'Meter';
