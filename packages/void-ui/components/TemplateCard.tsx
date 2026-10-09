import React from 'react';
import { tokens } from './styles';
import { Button } from './Button';

export interface TemplateCardProps {
  /** Resolved display name (caller localizes). */
  name: string;
  /** Resolved description (caller localizes). */
  description?: string;
  /** Real spec summary — e.g. "4 tracks · 110 BPM · 48 kHz". */
  specLine: string;
  selected?: boolean;
  busy?: boolean;
  disabled?: boolean;
  /** Localized action label, e.g. "Create". */
  actionLabel: string;
  onSelect?: () => void;
  onUse?: () => void;
}

/** Project template card in the launcher — name, spec summary, create action. */
export const TemplateCard: React.FC<TemplateCardProps> = React.memo(
  ({ name, description, specLine, selected = false, busy = false, disabled = false, actionLabel, onSelect, onUse }) => (
    <div
      role="listitem"
      aria-label={`Template ${name}`}
      onClick={onSelect}
      onKeyDown={onSelect ? (e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); onSelect(); } } : undefined}
      tabIndex={onSelect ? 0 : undefined}
      style={{
        display: 'flex',
        flexDirection: 'column',
        gap: 4,
        padding: '8px 10px',
        background: tokens.surface,
        border: `1px solid ${selected ? tokens.accent : tokens.border}`,
        borderRadius: tokens.radius,
        fontFamily: tokens.sans,
        color: tokens.text,
        cursor: onSelect ? 'pointer' : 'default',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
        <span style={{ flex: 1, fontSize: 12, fontWeight: 600 }}>{name}</span>
        <Button variant="primary" loading={busy} disabled={disabled || !onUse} onClick={(e) => { e.stopPropagation(); onUse?.(); }}>
          {actionLabel}
        </Button>
      </div>
      <span style={{ fontSize: 10, color: tokens.textMuted, fontFamily: tokens.mono }}>{specLine}</span>
      {description ? (
        <span style={{ fontSize: 10, color: tokens.textSecondary }}>{description}</span>
      ) : null}
    </div>
  ),
);
TemplateCard.displayName = 'TemplateCard';
