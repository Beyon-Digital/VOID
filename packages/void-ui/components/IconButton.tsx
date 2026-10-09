import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';

export interface IconButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** The glyph — icon element or short text. Rendered centered. */
  icon: React.ReactNode;
  /** Required accessible name — an icon-only button must never be nameless. */
  'aria-label': string;
  variant?: 'ghost' | 'solid' | 'danger';
  size?: 'sm' | 'md' | 'lg';
  /** Toggle button state (aria-pressed). */
  pressed?: boolean;
}

const SIZES = { sm: 24, md: 32, lg: 40 } as const;

/** Square icon-only button. `aria-label` is required by contract. */
export const IconButton: React.FC<IconButtonProps> = React.memo(
  ({
    icon,
    variant = 'ghost',
    size = 'md',
    pressed,
    disabled,
    style,
    className,
    ...rest
  }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const [hover, setHover] = React.useState(false);
    const dim = SIZES[size];
    const palette: React.CSSProperties =
      variant === 'danger'
        ? { background: pressed ? tokens.dangerSoft : 'transparent', color: tokens.danger, border: `1px solid ${pressed ? tokens.danger : 'transparent'}` }
        : variant === 'solid'
          ? { background: tokens.raised, color: tokens.text, border: `1px solid ${tokens.line}` }
          : { background: pressed ? tokens.accentSoft : 'transparent', color: pressed ? tokens.accent : tokens.muted, border: `1px solid ${pressed ? tokens.accent : 'transparent'}` };
    return (
      <button
        type="button"
        className={[focusClass, animatedClass, className].filter(Boolean).join(' ')}
        aria-pressed={pressed === undefined ? undefined : pressed}
        disabled={disabled}
        onMouseEnter={() => setHover(true)}
        onMouseLeave={() => setHover(false)}
        style={{
          width: dim,
          height: dim,
          display: 'inline-flex',
          alignItems: 'center',
          justifyContent: 'center',
          borderRadius: tokens.radius8,
          fontFamily: tokens.sans,
          fontSize: size === 'sm' ? 12 : 14,
          cursor: disabled ? 'default' : 'pointer',
          opacity: disabled ? 0.5 : 1,
          transition: 'background 120ms ease, color 120ms ease',
          ...palette,
          ...(hover && !disabled
            ? { background: variant === 'ghost' ? tokens.hover : palette.background, color: variant === 'danger' ? tokens.danger : tokens.text }
            : null),
          ...style,
        }}
        {...rest}
      >
        {icon}
      </button>
    );
  },
);
IconButton.displayName = 'IconButton';
