// Shared layout primitives for the UIP5 recovery/diagnostic screens —
// the handoff's recovery pattern is identical across S18/S19/S21:
// headline + body + preserved-state rows + numbered "what you can do
// now" steps + a closing note. These render tokens only; every piece of
// state text comes from the caller.

import * as React from 'react';
import { tokens, injectVoidStyles } from 'void-ui';

/** Full-height column the recovery screens sit in. */
export const RecoveryBody: React.FC<{ children: React.ReactNode }> = ({ children }) => {
  React.useEffect(() => injectVoidStyles(), []);
  return (
    <div
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        overflowY: 'auto',
        display: 'flex',
        gap: tokens.space24,
        padding: `${tokens.space24} ${tokens.space32}`,
        alignItems: 'flex-start',
      }}
    >
      {children}
    </div>
  );
};

/** Main (left) column: headline + copy + tables + actions. */
export const RecoveryMain: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', gap: tokens.space16 }}>
    {children}
  </div>
);

/** Right rail: "What you can do now." numbered steps. */
export const RecoveryRail: React.FC<{ title: string; children: React.ReactNode }> = ({
  title,
  children,
}) => (
  <aside
    aria-label={title}
    style={{
      width: 300,
      flexShrink: 0,
      borderLeft: `1px solid ${tokens.line}`,
      paddingLeft: tokens.space24,
      display: 'flex',
      flexDirection: 'column',
      gap: tokens.space16,
    }}
  >
    <span className="void-type-title" style={{ color: tokens.text }}>
      {title}
    </span>
    {children}
  </aside>
);

export const RecoveryHeadline: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <h1 className="void-type-display" style={{ color: tokens.text, margin: 0 }}>
    {children}
  </h1>
);

export const RecoveryCopy: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <p className="void-type-body" style={{ color: tokens.muted, margin: 0, maxWidth: 520 }}>
    {children}
  </p>
);

/** One numbered step in the right rail. */
export const RecoveryStep: React.FC<{
  n: string;
  title: string;
  children?: React.ReactNode;
}> = ({ n, title, children }) => (
  <div style={{ display: 'flex', gap: tokens.space12 }}>
    <span className="void-type-small" style={{ color: tokens.accent, fontFamily: tokens.mono }}>
      {n}
    </span>
    <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
      <span className="void-type-body" style={{ color: tokens.text }}>
        {title}
      </span>
      {children ? (
        <span className="void-type-small" style={{ color: tokens.subtle }}>
          {children}
        </span>
      ) : null}
    </div>
  </div>
);

/** One preserved-state row (label : value). */
export const StateRow: React.FC<{ label: string; value: React.ReactNode }> = ({
  label,
  value,
}) => (
  <div
    style={{
      display: 'flex',
      justifyContent: 'space-between',
      gap: tokens.space16,
      padding: `${tokens.space8} 0`,
      borderBottom: `1px solid ${tokens.line}`,
      maxWidth: 520,
    }}
  >
    <span className="void-type-small" style={{ color: tokens.subtle }}>
      {label}
    </span>
    <span className="void-type-small" style={{ color: tokens.text, textAlign: 'right' }}>
      {value}
    </span>
  </div>
);

/** Honest contract footnote. */
export const RecoveryNote: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <p
    className="void-type-small"
    style={{ color: tokens.subtle, margin: 0, maxWidth: 520 }}
  >
    {children}
  </p>
);
