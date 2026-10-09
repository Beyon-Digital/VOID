// Shared chrome for the proposal / gesture / jobs screens —
// eyebrow headings, rail/panel frames, honest-reason notes.

import * as React from 'react';
import { tokens } from 'void-ui';

export const Eyebrow: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <div
    style={{
      fontFamily: tokens.sans,
      fontSize: 10,
      fontWeight: 700,
      letterSpacing: '0.12em',
      textTransform: 'uppercase',
      color: tokens.textMuted,
    }}
  >
    {children}
  </div>
);

export const Card: React.FC<{
  children: React.ReactNode;
  pad?: number;
  role?: string;
  'aria-label'?: string;
}> = ({ children, pad = 10, role, 'aria-label': ariaLabel }) => (
  <div
    role={role}
    aria-label={ariaLabel}
    style={{
      background: tokens.raised,
      border: `1px solid ${tokens.line}`,
      borderRadius: tokens.radius8,
      padding: pad,
      display: 'flex',
      flexDirection: 'column',
      gap: 6,
    }}
  >
    {children}
  </div>
);

/** Small muted line — the honest-reason affordance under gated actions. */
export const ReasonNote: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <div
    role="note"
    style={{
      fontFamily: tokens.sans,
      fontSize: 11,
      lineHeight: 1.45,
      color: tokens.textMuted,
    }}
  >
    {children}
  </div>
);

export const Row: React.FC<{
  children: React.ReactNode;
  gap?: number;
  wrap?: boolean;
  align?: React.CSSProperties['alignItems'];
  justify?: React.CSSProperties['justifyContent'];
}> = ({ children, gap = 6, wrap = false, align = 'center', justify }) => (
  <div
    style={{
      display: 'flex',
      gap,
      alignItems: align,
      justifyContent: justify,
      flexWrap: wrap ? 'wrap' : undefined,
    }}
  >
    {children}
  </div>
);

export const Stack: React.FC<{
  children: React.ReactNode;
  gap?: number;
  style?: React.CSSProperties;
}> = ({ children, gap = 8, style }) => (
  <div style={{ display: 'flex', flexDirection: 'column', gap, ...style }}>
    {children}
  </div>
);

/** Right/left side rail column. */
export const Rail: React.FC<{
  children: React.ReactNode;
  width?: number;
  side?: 'left' | 'right';
  'aria-label'?: string;
}> = ({ children, width = 220, side = 'left', 'aria-label': label }) => (
  <aside
    aria-label={label}
    style={{
      width,
      flexShrink: 0,
      borderRight: side === 'left' ? `1px solid ${tokens.line}` : undefined,
      borderLeft: side === 'right' ? `1px solid ${tokens.line}` : undefined,
      overflowY: 'auto',
      padding: '12px 10px',
      display: 'flex',
      flexDirection: 'column',
      gap: 10,
      background: tokens.surface,
    }}
  >
    {children}
  </aside>
);

export const Divider: React.FC = () => (
  <div style={{ borderTop: `1px solid ${tokens.line}` }} aria-hidden />
);

export const Mono: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <span style={{ fontFamily: tokens.mono, fontSize: 11 }}>{children}</span>
);
