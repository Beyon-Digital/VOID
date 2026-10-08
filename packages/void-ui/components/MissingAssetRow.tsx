import React from 'react';
import { tokens } from './styles';
import { Button } from './Button';

export interface MissingAssetRowProps {
  displayName: string;
  /** Expected content address — truncated for display by the caller if wanted. */
  expectedSha256: string;
  detail?: string;
  /** Localized status line, e.g. "missing — expected sha256 ab12…". */
  statusLabel: string;
  busy?: boolean;
  disabled?: boolean;
  relinkLabel: string;
  /** 'relink' | 'replace' | null — verification outcome once a candidate is chosen. */
  verification?: 'relink' | 'replace' | null;
  verifiedLabel?: string;
  replaceWarnLabel?: string;
  onPickFile?: () => void;
}

/**
 * One missing-asset row in the relink recovery surface. Renders the
 * placeholder state honestly — expected hash + coordinator detail — and
 * never suggests the audio was silently substituted.
 */
export const MissingAssetRow: React.FC<MissingAssetRowProps> = React.memo(
  ({ displayName, expectedSha256, detail, statusLabel, busy = false, disabled = false, relinkLabel, verification = null, verifiedLabel, replaceWarnLabel, onPickFile }) => (
    <div
      role="listitem"
      aria-label={`Missing asset ${displayName}`}
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 4,
        padding: '6px 8px',
        background: tokens.surface,
        border: `1px solid ${tokens.warn}`,
        borderRadius: tokens.radius,
        fontFamily: tokens.sans,
        color: tokens.text,
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <span style={{ fontSize: 9, color: tokens.warn, textTransform: 'uppercase', letterSpacing: '0.06em' }}>
          missing
        </span>
        <span style={{ flex: 1, fontSize: 12, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
          {displayName}
        </span>
        <Button variant="primary" loading={busy} disabled={disabled || !onPickFile} onClick={onPickFile}>
          {relinkLabel}
        </Button>
      </div>
      <span style={{ fontSize: 9, color: tokens.textMuted, fontFamily: tokens.mono }} title={expectedSha256}>
        {statusLabel} sha256 {expectedSha256.slice(0, 16)}{expectedSha256.length > 16 ? '…' : ''}
      </span>
      {detail ? <span style={{ fontSize: 10, color: tokens.textSecondary }}>{detail}</span> : null}
      {verification === 'relink' && verifiedLabel ? (
        <span role="status" style={{ fontSize: 10, color: tokens.ok }}>{verifiedLabel}</span>
      ) : null}
      {verification === 'replace' && replaceWarnLabel ? (
        <span role="alert" style={{ fontSize: 10, color: tokens.warn }}>{replaceWarnLabel}</span>
      ) : null}
    </div>
  ),
);
MissingAssetRow.displayName = 'MissingAssetRow';
