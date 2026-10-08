import React from 'react';
import { tokens } from './styles';

/** Compact reservation/budget chip — renders decimal-string bytes/s honestly. */
export interface BudgetBadgeProps {
  /** Short label: "RAM", "VRAM", "CPU", "WALL". */
  label: string;
  /** Decimal-string byte count (reservations are u64 strings). */
  bytes?: string | null;
  /** Decimal-string seconds/nanoseconds for time budgets. */
  seconds?: string | null;
  nanos?: string | null;
  /** Thread count when the badge is a CPU reservation. */
  threads?: number | null;
  /** True when the job/model exceeds its budget default. */
  over?: boolean;
  title?: string;
}

const K = 1024;
export function formatBytes(decimal: string | null | undefined): string {
  if (!decimal) return '—';
  const n = Number(decimal);
  if (!Number.isFinite(n) || n < 0) return '—';
  if (n < K) return `${n}B`;
  if (n < K * K) return `${(n / K).toFixed(0)}KiB`;
  if (n < K * K * K) return `${(n / K / K).toFixed(0)}MiB`;
  return `${(n / K / K / K).toFixed(1)}GiB`;
}

export function formatNanos(decimal: string | null | undefined): string {
  if (!decimal) return '—';
  const n = Number(decimal);
  if (!Number.isFinite(n) || n <= 0) return '—';
  if (n < 1e6) return `${(n / 1e3).toFixed(0)}µs`;
  if (n < 1e9) return `${(n / 1e6).toFixed(0)}ms`;
  return `${(n / 1e9).toFixed(1)}s`;
}

/** One small monospace chip; value verbatim, never a unit the wire didn't send. */
export const BudgetBadge: React.FC<BudgetBadgeProps> = React.memo(
  ({ label, bytes, seconds, nanos, threads, over = false, title }) => {
    const value =
      threads !== null && threads !== undefined
        ? `${threads}×`
        : bytes !== undefined && bytes !== null
          ? formatBytes(bytes)
          : seconds !== undefined && seconds !== null
            ? `${seconds}s`
            : formatNanos(nanos);
    return (
      <span
        role="note"
        aria-label={`${label} ${value}${over ? ' (over budget)' : ''}`}
        title={title ?? `${label} ${value}`}
        style={{
          display: 'inline-flex', alignItems: 'center', gap: 3,
          padding: '1px 5px', borderRadius: 3,
          fontSize: 9, fontFamily: tokens.mono, letterSpacing: '0.02em',
          color: over ? tokens.danger : tokens.textSecondary,
          background: over ? tokens.accentSoft : tokens.surfaceRaised,
          border: `1px solid ${over ? tokens.danger : tokens.border}`,
        }}
      >
        <span style={{ opacity: 0.7 }}>{label}</span>
        <span>{value}</span>
      </span>
    );
  },
);
BudgetBadge.displayName = 'BudgetBadge';
