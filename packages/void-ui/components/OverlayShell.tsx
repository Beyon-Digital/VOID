import React from 'react';
import { tokens } from './styles';

export interface OverlayShellProps {
  /** Accessible dialog name (localized by the caller). */
  title: string;
  /** Optional step indicator, e.g. "2 of 5". */
  stepLabel?: string;
  /** Footer row — caller's localized action buttons. */
  actions?: React.ReactNode;
  /** Called on backdrop click or Escape — overlays must always be dismissible. */
  onDismiss?: () => void;
  children: React.ReactNode;
}

/**
 * Modal overlay scaffold for onboarding tours and the help panel.
 * Centered dialog over a dim backdrop; Escape/backdrop dismiss;
 * focus is left to the caller's first control.
 */
export const OverlayShell: React.FC<OverlayShellProps> = ({
  title,
  stepLabel,
  actions,
  onDismiss,
  children,
}) => {
  const labelId = React.useId();
  React.useEffect(() => {
    if (!onDismiss) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'Escape') onDismiss();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [onDismiss]);

  return (
    <div
      role="presentation"
      onClick={onDismiss}
      style={{
        position: 'fixed',
        inset: 0,
        background: 'rgba(0,0,0,0.55)',
        display: 'flex',
        alignItems: 'center',
        justifyContent: 'center',
        zIndex: 1000,
        fontFamily: tokens.sans,
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={labelId}
        onClick={(e) => e.stopPropagation()}
        style={{
          width: 'min(460px, 90vw)',
          maxHeight: '80vh',
          overflow: 'auto',
          background: tokens.surface,
          border: `1px solid ${tokens.border}`,
          borderRadius: tokens.radius,
          color: tokens.text,
          display: 'flex',
          flexDirection: 'column',
        }}
      >
        <header
          style={{
            display: 'flex',
            alignItems: 'center',
            justifyContent: 'space-between',
            gap: 8,
            padding: '10px 14px',
            borderBottom: `1px solid ${tokens.border}`,
          }}
        >
          <h2
            id={labelId}
            style={{
              margin: 0,
              fontSize: 12,
              fontFamily: tokens.mono,
              fontWeight: 600,
              letterSpacing: '0.08em',
              textTransform: 'uppercase',
              color: tokens.textSecondary,
            }}
          >
            {title}
          </h2>
          {stepLabel ? (
            <span style={{ fontSize: 10, color: tokens.textMuted, fontFamily: tokens.mono }}>
              {stepLabel}
            </span>
          ) : null}
        </header>
        <div style={{ padding: 14, fontSize: 12, lineHeight: 1.5, color: tokens.text }}>{children}</div>
        {actions ? (
          <footer
            style={{
              display: 'flex',
              justifyContent: 'flex-end',
              gap: 8,
              padding: '10px 14px',
              borderTop: `1px solid ${tokens.border}`,
            }}
          >
            {actions}
          </footer>
        ) : null}
      </div>
    </div>
  );
};
OverlayShell.displayName = 'OverlayShell';
