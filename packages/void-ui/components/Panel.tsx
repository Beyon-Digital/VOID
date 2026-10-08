import React from 'react';
import { tokens } from './styles';

export interface PanelProps extends React.HTMLAttributes<HTMLElement> {
  /** Accessible name for the panel region. */
  title: string;
  /** Optional actions rendered in the header row. */
  actions?: React.ReactNode;
  /** Render header text (default true). */
  showHeader?: boolean;
}

/** Region panel with a labelled header — the workspace's bounded section. */
export const Panel: React.FC<PanelProps> = React.memo(
  ({ title, actions, showHeader = true, children, style, ...rest }) => {
    const labelId = React.useId();
    return (
      <section
        aria-labelledby={labelId}
        style={{
          display: 'flex',
          flexDirection: 'column',
          background: tokens.surface,
          border: `1px solid ${tokens.border}`,
          borderRadius: tokens.radius,
          minHeight: 0,
          ...style,
        }}
        {...rest}
      >
        {showHeader && (
          <header
            style={{
              display: 'flex',
              alignItems: 'center',
              justifyContent: 'space-between',
              gap: 8,
              padding: '8px 12px',
              borderBottom: `1px solid ${tokens.border}`,
            }}
          >
            <h2
              id={labelId}
              style={{
                margin: 0,
                fontSize: 11,
                fontFamily: tokens.mono,
                fontWeight: 600,
                letterSpacing: '0.08em',
                textTransform: 'uppercase',
                color: tokens.textSecondary,
              }}
            >
              {title}
            </h2>
            {actions ? <div style={{ display: 'flex', gap: 6 }}>{actions}</div> : null}
          </header>
        )}
        <div style={{ padding: 12, overflow: 'auto', minHeight: 0, flex: 1 }}>{children}</div>
      </section>
    );
  },
);
Panel.displayName = 'Panel';
