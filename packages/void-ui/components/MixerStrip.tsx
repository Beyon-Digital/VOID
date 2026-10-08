import React from 'react';
import { tokens } from './styles';
import { Fader } from './Fader';
import { Knob } from './Knob';
import { Meter } from './Meter';
import { Button } from './Button';

export interface MixerStripProps {
  trackId: string;
  name: string;
  /** Track kind from the summary (e.g. AUDIO/MIDI/BUS). */
  kind?: string;
  /** Gain in dB, -80..+12 (MIN_DB floor mirrors void-studio mixer/ops). */
  gainDb: number;
  pan: number;
  muted: boolean;
  soloed: boolean;
  /** Latest MeterFrame peak/RMS (0..1); ignored while meterIdle. */
  meterPeak?: number;
  meterRms?: number;
  /** True when no fresh MeterFrame has arrived — meter renders idle, never fake decay. */
  meterIdle: boolean;
  selected?: boolean;
  onGainDb?: (trackId: string, db: number) => void;
  onPan?: (trackId: string, pan: number) => void;
  onToggleMute?: (trackId: string, muted: boolean) => void;
  onToggleSolo?: (trackId: string, soloed: boolean) => void;
}

const MIN_DB = -80;
const fmtDb = (db: number): string => (db <= MIN_DB ? '-∞' : `${db.toFixed(1)} dB`);
const fmtPan = (p: number): string =>
  p === 0 ? 'C' : `${Math.round(Math.abs(p) * 100)}${p < 0 ? 'L' : 'R'}`;

/**
 * One mixer channel strip: pan knob, fader, live meter, mute/solo.
 * Pure view — the void-studio mixer controller owns ops + revisions.
 */
export const MixerStrip: React.FC<MixerStripProps> = React.memo(
  ({
    trackId, name, kind, gainDb, pan, muted, soloed,
    meterPeak = 0, meterRms, meterIdle, selected = false,
    onGainDb, onPan, onToggleMute, onToggleSolo,
  }) => {
    const wrap: React.CSSProperties = {
      display: 'inline-flex',
      flexDirection: 'column',
      alignItems: 'center',
      gap: 6,
      padding: 8,
      width: 72,
      background: selected ? tokens.accentSoft : tokens.surface,
      border: `1px solid ${selected ? tokens.accent : tokens.border}`,
      borderRadius: tokens.radius,
      fontFamily: tokens.sans,
      color: tokens.text,
    };
    const nameStyle: React.CSSProperties = {
      fontSize: 11,
      maxWidth: '100%',
      overflow: 'hidden',
      textOverflow: 'ellipsis',
      whiteSpace: 'nowrap',
    };
    return (
      <div style={wrap} data-track-id={trackId} role="group" aria-label={`Mixer strip ${name}`}>
        <div style={nameStyle} title={name}>{name}{kind === 'BUS' ? ' (bus)' : ''}</div>
        <Knob
          value={pan}
          min={-1}
          max={1}
          step={0.01}
          size={28}
          label={`${name} pan`}
          formatValue={fmtPan}
          onChange={(v) => onPan?.(trackId, v)}
        />
        <div style={{ display: 'flex', alignItems: 'flex-end', gap: 6 }}>
          <Fader
            value={gainDb}
            min={MIN_DB}
            max={12}
            step={0.1}
            height={96}
            label={`${name} gain`}
            formatValue={fmtDb}
            onChange={(v) => onGainDb?.(trackId, v)}
          />
          <Meter
            level={meterPeak}
            rms={meterRms}
            label={`${name} level`}
            size={96}
            idle={meterIdle}
          />
        </div>
        <div style={{ display: 'flex', gap: 4 }}>
          <Button
            variant={muted ? 'danger' : 'subtle'}
            aria-pressed={muted}
            onClick={() => onToggleMute?.(trackId, !muted)}
          >
            M
          </Button>
          <Button
            variant={soloed ? 'primary' : 'subtle'}
            aria-pressed={soloed}
            onClick={() => onToggleSolo?.(trackId, !soloed)}
          >
            S
          </Button>
        </div>
      </div>
    );
  },
);
