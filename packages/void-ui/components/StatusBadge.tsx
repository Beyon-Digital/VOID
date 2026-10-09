import React from 'react';
import { injectVoidStyles, tokens } from './styles';

/**
 * Truthful status pill — the ONLY way the shell may report engine/save/launch
 * state. `status` values map to real states; callers must not fabricate them
 * (queued ≠ playing until the native ack lands, saved means durable).
 */
export type StatusBadgeStatus =
  | 'ready'
  | 'armed'
  | 'recording'
  | 'queued'
  | 'playing'
  | 'unavailable'
  | 'error'
  | 'saved'
  | 'dirty'
  | 'processing'
  | 'disconnected';

const PALETTE: Record<StatusBadgeStatus, { fg: string; bg: string; label: string }> = {
  ready: { fg: tokens.mint, bg: tokens.mintSoft, label: 'Ready' },
  armed: { fg: tokens.orange, bg: tokens.orangeSoft, label: 'Armed' },
  recording: { fg: tokens.danger, bg: tokens.dangerSoft, label: 'Recording' },
  queued: { fg: tokens.blue, bg: tokens.blueSoft, label: 'Queued' },
  playing: { fg: tokens.mint, bg: tokens.mintSoft, label: 'Playing' },
  unavailable: { fg: tokens.subtle, bg: tokens.raised, label: 'Unavailable' },
  error: { fg: tokens.danger, bg: tokens.dangerSoft, label: 'Error' },
  saved: { fg: tokens.mint, bg: tokens.mintSoft, label: 'Saved' },
  dirty: { fg: tokens.orange, bg: tokens.orangeSoft, label: 'Unsaved' },
  processing: { fg: tokens.violet, bg: tokens.violetSoft, label: 'Working' },
  disconnected: { fg: tokens.subtle, bg: tokens.raised, label: 'Disconnected' },
};

export interface StatusBadgeProps {
  status: StatusBadgeStatus;
  /** Override the default label (still reflects the same status). */
  label?: string;
  /** Hide the leading state dot (compact rows). */
  hideDot?: boolean;
  title?: string;
}

export const StatusBadge: React.FC<StatusBadgeProps> = React.memo(
  ({ status, label, hideDot = false, title }) => {
    React.useEffect(() => injectVoidStyles(), []);
    const p = PALETTE[status];
    return (
      <span
        role="status"
        title={title}
        style={{
          display: 'inline-flex',
          alignItems: 'center',
          gap: tokens.space4,
          padding: `2px ${tokens.space8}`,
          borderRadius: tokens.radius24,
          background: p.bg,
          color: p.fg,
          fontFamily: tokens.sans,
          fontSize: 11,
          fontWeight: 600,
          lineHeight: '16px',
          whiteSpace: 'nowrap',
        }}
      >
        {hideDot ? null : (
          <span
            aria-hidden
            style={{ width: 6, height: 6, borderRadius: '50%', background: p.fg, flexShrink: 0 }}
          />
        )}
        {label ?? p.label}
      </span>
    );
  },
);
StatusBadge.displayName = 'StatusBadge';
