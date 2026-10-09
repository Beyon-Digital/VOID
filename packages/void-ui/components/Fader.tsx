// Back-compat adapter — `Fader` was the pre-Signal name; the canonical
// family is ChannelFader (continuous level, same contract).
import React from 'react';
import { ChannelFader, type ChannelFaderProps } from './ChannelFader';

export interface FaderProps {
  value: number;
  min?: number;
  max?: number;
  onChange?: (value: number) => void;
  height?: number;
  label?: string;
  step?: number;
  formatValue?: (value: number) => string;
}

export const Fader: React.FC<FaderProps> = ({
  value,
  min = 0,
  max = 1,
  onChange,
  height = 120,
  label = 'fader',
  step,
  formatValue,
}) => {
  const props: ChannelFaderProps = {
    value,
    min,
    max,
    step,
    unit: '',
    label,
    height,
    format: formatValue,
    onPreview: onChange ? (v) => onChange(v) : undefined,
    onCommit: onChange ? (tx) => onChange(tx.to) : undefined,
  };
  return <ChannelFader {...props} />;
};
