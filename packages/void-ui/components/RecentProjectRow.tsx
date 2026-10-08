import React from 'react';
import { tokens } from './styles';
import { Button } from './Button';

export interface RecentProjectRowProps {
  name: string;
  /** Absolute container dir — shown verbatim (it is the reopen token). */
  containerDir: string;
  /** 'created' | 'opened' — rendered as a small tag; caller localizes labels. */
  kindLabel: string;
  /** Localized timestamp line, e.g. "last opened 2026-10-08 19:40". */
  lastOpenedLabel: string;
  busy?: boolean;
  disabled?: boolean;
  openLabel: string;
  removeLabel: string;
  onOpen: () => void;
  onRemove?: () => void;
}

/** One recent-projects row — reopen token + real recorded metadata only. */
export const RecentProjectRow: React.FC<RecentProjectRowProps> = React.memo(
  ({ name, containerDir, kindLabel, lastOpenedLabel, busy = false, disabled = false, openLabel, removeLabel, onOpen, onRemove }) => (
    <div
      role="listitem"
      aria-label={`Recent project ${name}`}
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 8,
        padding: '6px 8px',
        background: tokens.surface,
        border: `1px solid ${tokens.border}`,
        borderRadius: tokens.radius,
        fontFamily: tokens.sans,
        color: tokens.text,
      }}
    >
      <span style={{ fontSize: 9, color: tokens.textMuted, textTransform: 'uppercase', letterSpacing: '0.06em', minWidth: 48 }}>
        {kindLabel}
      </span>
      <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column' }}>
        <span style={{ fontSize: 12, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>{name}</span>
        <span
          style={{ fontSize: 9, color: tokens.textMuted, fontFamily: tokens.mono, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}
          title={containerDir}
        >
          {containerDir}
        </span>
        <span style={{ fontSize: 9, color: tokens.textMuted }}>{lastOpenedLabel}</span>
      </div>
      <Button variant="primary" loading={busy} disabled={disabled} onClick={onOpen}>
        {openLabel}
      </Button>
      {onRemove ? (
        <Button variant="ghost" onClick={onRemove} aria-label={`${removeLabel} ${name}`}>
          {removeLabel}
        </Button>
      ) : null}
    </div>
  ),
);
RecentProjectRow.displayName = 'RecentProjectRow';
