import React from 'react';
import { tokens } from './styles';
import { Knob } from './Knob';
import { Button } from './Button';

/** One instrument parameter as rendered — mirrors void-studio ParamDescriptor. */
export interface InstrumentParamView {
  paramId: string;
  name: string;
  value: number;
  min: number;
  max: number;
  step?: number;
  /** 'enum' params show the label for their integer value. */
  labels?: readonly string[];
  unit?: string;
}

export interface InstrumentRackProps {
  /** Display name, e.g. "FourOsc". */
  instrumentName: string;
  params: readonly InstrumentParamView[];
  /** Selected preset name shown beside the title, if any. */
  presetName?: string;
  onParamChange?: (paramId: string, value: number) => void;
  onRemove?: () => void;
}

const fmt = (p: InstrumentParamView) => (v: number): string => {
  if (p.labels && p.labels.length > 0) {
    const i = Math.min(p.labels.length - 1, Math.max(0, Math.round(v)));
    return p.labels[i];
  }
  const s = Math.abs(p.step ?? 0) >= 1 ? v.toFixed(0) : v.toFixed(2);
  return p.unit ? `${s} ${p.unit}` : s;
};

/** Grid of knobs for one loaded instrument slot. View-only — binding lives in void-studio. */
export const InstrumentRack: React.FC<InstrumentRackProps> = React.memo(
  ({ instrumentName, params, presetName, onParamChange, onRemove }) => (
    <section
      aria-label={`Instrument ${instrumentName}`}
      style={{
        display: 'flex', flexDirection: 'column', gap: 8, padding: 8,
        background: tokens.surface, border: `1px solid ${tokens.border}`,
        borderRadius: tokens.radius, fontFamily: tokens.sans, color: tokens.text,
      }}
    >
      <header style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <strong style={{ fontSize: 12 }}>{instrumentName}</strong>
        {presetName ? <span style={{ fontSize: 11, color: tokens.textSecondary }}>{presetName}</span> : null}
        <span style={{ flex: 1 }} />
        {onRemove ? (
          <Button variant="ghost" onClick={onRemove} aria-label={`Remove ${instrumentName}`}>×</Button>
        ) : null}
      </header>
      <div
        role="group"
        aria-label={`${instrumentName} parameters`}
        style={{ display: 'grid', gridTemplateColumns: 'repeat(auto-fill, minmax(56px, 1fr))', gap: 6 }}
      >
        {params.map((p) => (
          <Knob
            key={p.paramId}
            value={p.value}
            min={p.min}
            max={p.max}
            step={p.step}
            label={p.name}
            size={40}
            formatValue={fmt(p)}
            onChange={(v) => onParamChange?.(p.paramId, v)}
          />
        ))}
      </div>
    </section>
  ),
);
