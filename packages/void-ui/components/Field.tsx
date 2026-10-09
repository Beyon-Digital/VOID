import React from 'react';
import { injectVoidStyles, tokens } from './styles';

let fieldSeq = 0;

export interface FieldProps {
  /** Visible label — required for an accessible name on the control. */
  label: string;
  /** Unit suffix rendered after the control (e.g. "dB", "Hz", "%"). */
  unit?: string;
  /** Secondary hint under the control. */
  hint?: string;
  /** Error text — replaces hint, marks the control invalid. */
  error?: string;
  /** Render the label inline-left instead of stacked. */
  inline?: boolean;
  disabled?: boolean;
  children: React.ReactNode | ((id: string) => React.ReactNode);
  style?: React.CSSProperties;
}

/**
 * Labeled form field wrapper — label + control + unit + hint/error.
 * When `children` is a function it receives the generated control id so the
 * label's htmlFor wires to the real input (accessible names by contract).
 */
export const Field: React.FC<FieldProps> = ({
  label,
  unit,
  hint,
  error,
  inline = false,
  disabled = false,
  children,
  style,
}) => {
  React.useEffect(() => injectVoidStyles(), []);
  const idRef = React.useRef(`void-field-${++fieldSeq}`);
  const describedBy = `${idRef.current}-hint`;
  return (
    <div
      style={{
        display: 'flex',
        flexDirection: inline ? 'row' : 'column',
        alignItems: inline ? 'center' : 'stretch',
        gap: inline ? tokens.space8 : tokens.space4,
        opacity: disabled ? 0.55 : 1,
        fontFamily: tokens.sans,
        ...style,
      }}
    >
      <label
        htmlFor={idRef.current}
        style={{
          fontSize: 12,
          fontWeight: 500,
          color: tokens.muted,
          whiteSpace: inline ? 'nowrap' : undefined,
        }}
      >
        {label}
      </label>
      <div style={{ display: 'flex', alignItems: 'center', gap: tokens.space4, flex: 1 }}>
        {typeof children === 'function' ? children(idRef.current) : children}
        {unit ? (
          <span aria-hidden style={{ fontSize: 11, color: tokens.subtle, fontFamily: tokens.fontNumeric }}>
            {unit}
          </span>
        ) : null}
      </div>
      {error ? (
        <span id={describedBy} role="alert" style={{ fontSize: 11, color: tokens.danger }}>
          {error}
        </span>
      ) : hint ? (
        <span id={describedBy} style={{ fontSize: 11, color: tokens.subtle }}>
          {hint}
        </span>
      ) : null}
    </div>
  );
};

export interface TextInputProps
  extends React.InputHTMLAttributes<HTMLInputElement> {
  invalid?: boolean;
}

/** Token-styled text input — the default control inside `Field`. */
export const TextInput = React.forwardRef<HTMLInputElement, TextInputProps>(
  ({ invalid, style, ...rest }, ref) => {
    React.useEffect(() => injectVoidStyles(), []);
    return (
      <input
        ref={ref}
        className={injectableInputClass}
        aria-invalid={invalid || undefined}
        style={{
          flex: 1,
          minWidth: 0,
          background: tokens.raised,
          border: `1px solid ${invalid ? tokens.danger : tokens.line}`,
          borderRadius: tokens.radius8,
          color: tokens.text,
          fontFamily: tokens.sans,
          fontSize: 13,
          padding: '6px 8px',
          outline: 'none',
          ...style,
        }}
        {...rest}
      />
    );
  },
);
TextInput.displayName = 'TextInput';

const injectableInputClass = 'void-field-input';
