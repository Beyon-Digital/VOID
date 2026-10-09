import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';

export interface WorkspaceTabProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement> {
  /** Stable id used for aria-controls ↔ panel wiring. */
  tabId: string;
  title: string;
  active?: boolean;
  /** Shortcut hint text (e.g. "⌃1") rendered as a micro label. */
  shortcut?: string;
  /** Optional count badge (jobs, candidates…). */
  badge?: number | string;
}

/**
 * One workspace tab — the canonical tab used by the studio header's
 * tablist. Active tab gets the accent underline + raised surface.
 */
export const WorkspaceTab: React.FC<WorkspaceTabProps> = React.memo(
  ({ tabId, title, active = false, shortcut, badge, style, ...rest }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const [hover, setHover] = React.useState(false);
    return (
      <button
        type="button"
        role="tab"
        id={`workspace-tab-${tabId}`}
        aria-selected={active}
        aria-controls={`workspace-panel-${tabId}`}
        tabIndex={active ? 0 : -1}
        className={[focusClass, animatedClass].join(' ')}
        onMouseEnter={() => setHover(true)}
        onMouseLeave={() => setHover(false)}
        style={{
          display: 'inline-flex',
          alignItems: 'center',
          gap: tokens.space8,
          padding: `${tokens.space8} ${tokens.space12}`,
          background: active ? tokens.raised : hover ? tokens.hover : 'transparent',
          color: active ? tokens.text : tokens.muted,
          border: 'none',
          borderBottom: `2px solid ${active ? tokens.accent : 'transparent'}`,
          borderRadius: `${tokens.radius8} ${tokens.radius8} 0 0`,
          fontFamily: tokens.sans,
          fontSize: 13,
          fontWeight: active ? 600 : 500,
          cursor: 'pointer',
          transition: 'background 120ms ease, color 120ms ease',
          ...style,
        }}
        {...rest}
      >
        <span>{title}</span>
        {shortcut ? (
          <span aria-hidden style={{ fontSize: 10, color: tokens.subtle, fontFamily: tokens.fontNumeric }}>
            {shortcut}
          </span>
        ) : null}
        {badge !== undefined ? (
          <span
            style={{
              minWidth: 16,
              padding: '0 4px',
              borderRadius: tokens.radius24,
              background: tokens.accentSoft,
              color: tokens.accent,
              fontSize: 10,
              fontWeight: 700,
              lineHeight: '16px',
              textAlign: 'center',
              fontFamily: tokens.fontNumeric,
            }}
          >
            {badge}
          </span>
        ) : null}
      </button>
    );
  },
);
WorkspaceTab.displayName = 'WorkspaceTab';
