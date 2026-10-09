import React from 'react';
import { animatedClass, focusClass, injectVoidStyles, tokens } from './styles';
import { ActionButton } from './ActionButton';
import { IconButton } from './IconButton';

export type ToastVariant = 'info' | 'success' | 'warning' | 'error';

export interface ToastItem {
  id: string;
  variant: ToastVariant;
  title: string;
  message?: string;
  /** Optional action rendered as a small button. */
  action?: { label: string; run: () => void };
  /** Auto-dismiss after N ms (0 = sticky). Default 6000; errors sticky. */
  durationMs?: number;
}

const PALETTE: Record<ToastVariant, { fg: string; bg: string; border: string }> = {
  info: { fg: tokens.blue, bg: tokens.blueSoft, border: tokens.blue },
  success: { fg: tokens.mint, bg: tokens.mintSoft, border: tokens.mint },
  warning: { fg: tokens.orange, bg: tokens.orangeSoft, border: tokens.orange },
  error: { fg: tokens.danger, bg: tokens.dangerSoft, border: tokens.danger },
};

export const Toast: React.FC<{ toast: ToastItem; onDismiss: (id: string) => void }> = React.memo(
  ({ toast, onDismiss }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const p = PALETTE[toast.variant];
    React.useEffect(() => {
      const dur = toast.durationMs ?? (toast.variant === 'error' ? 0 : 6000);
      if (!dur) return;
      const reduced =
        typeof window !== 'undefined' &&
        window.matchMedia?.('(prefers-reduced-motion: reduce)').matches;
      const t = window.setTimeout(() => onDismiss(toast.id), reduced ? dur * 2 : dur);
      return () => window.clearTimeout(t);
    }, [toast, onDismiss]);
    return (
      <div
        role={toast.variant === 'error' ? 'alert' : 'status'}
        style={{
          display: 'flex',
          alignItems: 'flex-start',
          gap: tokens.space8,
          minWidth: 240,
          maxWidth: 360,
          padding: `${tokens.space8} ${tokens.space12}`,
          background: tokens.raised,
          border: `1px solid ${tokens.line}`,
          borderLeft: `3px solid ${p.border}`,
          borderRadius: tokens.radius8,
          boxShadow: `0 4px 16px ${tokens.ink}`,
          fontFamily: tokens.sans,
        }}
      >
        <span aria-hidden style={{ color: p.fg, fontSize: 13, lineHeight: '18px' }}>
          ●
        </span>
        <div style={{ flex: 1, minWidth: 0 }}>
          <div style={{ fontSize: 12, fontWeight: 600, color: tokens.text }}>{toast.title}</div>
          {toast.message ? (
            <div style={{ fontSize: 11, color: tokens.muted, marginTop: 2 }}>{toast.message}</div>
          ) : null}
          {toast.action ? (
            <ActionButton
              size="sm"
              variant="ghost"
              onClick={() => {
                toast.action?.run();
                onDismiss(toast.id);
              }}
              style={{ marginTop: 4, padding: '2px 6px' }}
            >
              {toast.action.label}
            </ActionButton>
          ) : null}
        </div>
        <IconButton icon="×" aria-label="Dismiss notification" size="sm" onClick={() => onDismiss(toast.id)} />
      </div>
    );
  },
);
Toast.displayName = 'Toast';

export function useToasts(): {
  toasts: ToastItem[];
  push: (t: Omit<ToastItem, 'id'>) => string;
  dismiss: (id: string) => void;
} {
  const [toasts, setToasts] = React.useState<ToastItem[]>([]);
  const dismiss = React.useCallback((id: string) => {
    setToasts((ts) => ts.filter((t) => t.id !== id));
  }, []);
  const push = React.useCallback((t: Omit<ToastItem, 'id'>) => {
    const id = `toast-${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 7)}`;
    setToasts((ts) => [...ts, { ...t, id }]);
    return id;
  }, []);
  return { toasts, push, dismiss };
}

/** Fixed bottom-right viewport stacking toasts — one per app. */
export const ToastViewport: React.FC<{
  toasts: ToastItem[];
  onDismiss: (id: string) => void;
}> = ({ toasts, onDismiss }) => (
  <div
    aria-label="Notifications"
    style={{
      position: 'fixed',
      right: 16,
      bottom: 40,
      display: 'flex',
      flexDirection: 'column',
      gap: tokens.space8,
      zIndex: 1000,
      pointerEvents: 'none',
    }}
  >
    {toasts.map((t) => (
      <div key={t.id} style={{ pointerEvents: 'auto' }}>
        <Toast toast={t} onDismiss={onDismiss} />
      </div>
    ))}
  </div>
);
