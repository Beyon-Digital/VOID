import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';

export type ActionButtonVariant = 'primary' | 'secondary' | 'ghost' | 'danger' | 'subtle';
export type ActionButtonSize = 'sm' | 'md';

export interface ActionButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ActionButtonVariant;
  size?: ActionButtonSize;
  /** true while the triggered command is in flight — disables + aria-busy. */
  loading?: boolean;
  /** Stretch to the container width (dialog footers, launcher). */
  fullWidth?: boolean;
}

const VARIANTS: Record<ActionButtonVariant, React.CSSProperties> = {
  primary: {
    background: tokens.accent,
    color: tokens.onAccent,
    border: `1px solid ${tokens.accent}`,
  },
  secondary: {
    background: tokens.raised,
    color: tokens.text,
    border: `1px solid ${tokens.line}`,
  },
  ghost: {
    background: 'transparent',
    color: tokens.muted,
    border: '1px solid transparent',
  },
  danger: {
    background: tokens.dangerSoft,
    color: tokens.danger,
    border: `1px solid ${tokens.danger}`,
  },
  subtle: {
    background: tokens.accentSoft,
    color: tokens.accent,
    border: `1px solid ${tokens.accentSoft}`,
  },
};

const SIZES: Record<ActionButtonSize, React.CSSProperties> = {
  sm: { fontSize: 11, padding: '4px 10px', borderRadius: tokens.radius8, minHeight: 24 },
  md: { fontSize: 13, padding: '7px 14px', borderRadius: tokens.radius8, minHeight: 32 },
};

/**
 * Canonical action affordance. Hover/active styling rides on the .void-studio
 * stylesheet; disabled never fires handlers (native button semantics).
 */
export const ActionButton: React.FC<ActionButtonProps> = React.memo(
  ({
    variant = 'primary',
    size = 'md',
    loading = false,
    fullWidth = false,
    disabled,
    style,
    className,
    children,
    ...rest
  }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const [hover, setHover] = React.useState(false);
    const off = disabled || loading;
    return (
      <button
        type="button"
        className={[focusClass, animatedClass, className].filter(Boolean).join(' ')}
        disabled={off}
        aria-busy={loading || undefined}
        onMouseEnter={() => setHover(true)}
        onMouseLeave={() => setHover(false)}
        style={{
          display: 'inline-flex',
          alignItems: 'center',
          justifyContent: 'center',
          gap: tokens.space8,
          fontFamily: tokens.sans,
          fontWeight: 600,
          cursor: off ? 'default' : 'pointer',
          opacity: off ? 0.5 : 1,
          width: fullWidth ? '100%' : undefined,
          transition: 'background 120ms ease, color 120ms ease, border-color 120ms ease',
          ...SIZES[size],
          ...VARIANTS[variant],
          ...(hover && !off && variant !== 'primary' ? { background: tokens.hover, color: tokens.text } : null),
          ...style,
        }}
        {...rest}
      >
        {children}
      </button>
    );
  },
);
ActionButton.displayName = 'ActionButton';
