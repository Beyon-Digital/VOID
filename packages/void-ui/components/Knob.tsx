// Back-compat adapter — `Knob` was the pre-Signal name; the canonical family
// is ParameterKnob. Old callsites used `onChange` fired on every preview tick
// AND on settle; the adapter preserves that semantics on top of the new
// gesture core (preview stream + single final commit).
import React from 'react';
import { ParameterKnob, type ParameterKnobProps } from './ParameterKnob';

export interface KnobProps {
  value: number;
  min?: number;
  max?: number;
  onChange?: (value: number) => void;
  label?: string;
  size?: number;
  step?: number;
  fineStepRatio?: number;
  formatValue?: (value: number) => string;
}

export const Knob: React.FC<KnobProps> = ({
  value,
  min = 0,
  max = 1,
  onChange,
  label,
  size = 40,
  step,
  fineStepRatio = 0.1,
  formatValue,
}) => {
  const baseStep = step ?? ((max - min) / 50 || 0.01);
  const knobProps: ParameterKnobProps = {
    value,
    min,
    max,
    step: baseStep,
    fineStep: baseStep * fineStepRatio,
    defaultValue: min,
    label,
    size,
    format: formatValue,
    onPreview: onChange ? (v) => onChange(v) : undefined,
    onCommit: onChange ? (tx) => onChange(tx.to) : undefined,
  };
  return <ParameterKnob {...knobProps} />;
};
