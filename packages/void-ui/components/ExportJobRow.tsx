import React from 'react';
import { tokens } from './styles';
import { Button } from './Button';

/** Job lifecycle states — mirror crates/void-jobs JobStatus. */
export type ExportJobStatus = 'queued' | 'running' | 'cancelling' | 'succeeded' | 'failed' | 'cancelled';

export interface ExportJobRowProps {
  jobId: string;
  /** Output display name (sanitized basename). */
  name: string;
  status: ExportJobStatus;
  /** 0..100 while running; null = indeterminate (runner hasn't reported). */
  percent?: number | null;
  /** Current stage label, e.g. "render" / "verify" / "publish". */
  stage?: string;
  /** Failure message when status === 'failed'. */
  error?: string;
  /** Published artifact path when status === 'succeeded'. */
  outputPath?: string;
  /** Bytes written so far, when reported. */
  bytesWritten?: string;
  onCancel?: (jobId: string) => void;
}

const STATUS_LABEL: Record<ExportJobStatus, string> = {
  queued: 'Queued',
  running: 'Running',
  cancelling: 'Cancelling…',
  succeeded: 'Done',
  failed: 'Failed',
  cancelled: 'Cancelled',
};

const statusColor = (s: ExportJobStatus): string =>
  s === 'failed' ? tokens.danger
    : s === 'succeeded' ? tokens.ok
    : s === 'cancelled' ? tokens.textMuted
    : tokens.accent;

/** One row of the export dialog's job list — live progress or terminal result. */
export const ExportJobRow: React.FC<ExportJobRowProps> = React.memo(
  ({ jobId, name, status, percent = null, stage, error, outputPath, bytesWritten, onCancel }) => {
    const busy = status === 'queued' || status === 'running' || status === 'cancelling';
    const frac = percent === null ? null : Math.min(1, Math.max(0, percent / 100));
    return (
      <div
        role="listitem"
        aria-label={`Export ${name}: ${STATUS_LABEL[status]}`}
        style={{
          display: 'flex', flexDirection: 'column', gap: 4, padding: '6px 8px',
          background: tokens.surface, border: `1px solid ${tokens.border}`,
          borderRadius: tokens.radius, fontFamily: tokens.sans, color: tokens.text,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span style={{ flex: 1, fontSize: 12, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={outputPath ?? name}>
            {name}
          </span>
          {stage && busy ? <span style={{ fontSize: 10, color: tokens.textSecondary }}>{stage}</span> : null}
          <span style={{ fontSize: 10, color: statusColor(status) }}>{STATUS_LABEL[status]}</span>
          {busy && onCancel ? (
            <Button variant="ghost" onClick={() => onCancel(jobId)} aria-label={`Cancel export ${name}`}>
              Cancel
            </Button>
          ) : null}
        </div>
        {busy ? (
          <div
            role="progressbar"
            aria-valuemin={0}
            aria-valuemax={100}
            {...(frac === null ? {} : { 'aria-valuenow': Math.round(frac * 100) })}
            style={{ height: 4, background: tokens.surfaceRaised, borderRadius: 2, overflow: 'hidden' }}
          >
            <div
              style={{
                height: '100%',
                width: frac === null ? '30%' : `${frac * 100}%`,
                background: statusColor(status),
                transition: 'width 120ms linear',
                ...(frac === null ? { animation: 'void-indeterminate 1.2s ease-in-out infinite alternate' } : {}),
              }}
            />
          </div>
        ) : null}
        {status === 'failed' && error ? (
          <div style={{ fontSize: 11, color: tokens.danger, fontFamily: tokens.mono }}>{error}</div>
        ) : null}
        {status === 'succeeded' && bytesWritten ? (
          <div style={{ fontSize: 10, color: tokens.textMuted, fontFamily: tokens.mono }}>{bytesWritten} bytes</div>
        ) : null}
      </div>
    );
  },
);
