import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: 'primary' | 'ghost' | 'danger' | 'subtle';
  /** true while the triggered command is in flight — disables + aria-busy. */
  loading?: boolean;
}

const VARIANTS: Record<
  NonNullable<ButtonProps['variant']>,
  React.CSSProperties
> = {
  primary: {
    background: tokens.accent,
    color: '#0b0d14',
    border: `1px solid ${tokens.accent}`,
  },
  ghost: {
    background: 'transparent',
    color: tokens.text,
    border: `1px solid ${tokens.border}`,
  },
  subtle: {
    background: tokens.surfaceRaised,
    color: tokens.text,
    border: `1px solid ${tokens.border}`,
  },
  danger: {
    background: 'transparent',
    color: tokens.danger,
    border: `1px solid ${tokens.danger}`,
  },
};

export const Button: React.FC<ButtonProps> = React.memo(
  ({ variant = 'ghost', loading = false, disabled, children, style, ...rest }) => {
    React.useEffect(() => injectVoidStyles(), []);
    return (
      <button
        type="button"
        disabled={disabled || loading}
        aria-busy={loading || undefined}
        style={{
          fontFamily: tokens.mono,
          fontSize: 12,
          padding: '6px 12px',
          borderRadius: tokens.radius,
          cursor: disabled || loading ? 'not-allowed' : 'pointer',
          opacity: disabled || loading ? 0.55 : 1,
          transition: 'background 120ms ease, border-color 120ms ease',
          ...VARIANTS[variant],
          ...style,
        }}
        {...rest}
        className={`${focusClass} ${animatedClass} ${rest.className ?? ''}`}
      >
        {loading ? '…' : children}
      </button>
    );
  },
);
Button.displayName = 'Button';
