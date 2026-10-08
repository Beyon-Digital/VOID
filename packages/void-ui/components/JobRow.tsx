import React from 'react';
import { tokens } from './styles';
import { Button } from './Button';
import { BudgetBadge } from './BudgetBadge';

/** AI job lifecycle — mirrors crates/void-jobs JobStatus (snake_case). */
export type JobStatus = 'queued' | 'running' | 'cancelling' | 'succeeded' | 'failed' | 'cancelled';

export type JobKind =
  | 'symbolic'
  | 'transcription'
  | 'separation'
  | 'audio_generation'
  | 'visual_generation'
  | 'analysis'
  | 'av_export';

export interface JobRowProps {
  jobId: string;
  /** Display name — model name or kind label; never fabricated. */
  name: string;
  kind?: JobKind;
  status: JobStatus;
  /** 0..100 while running; null = indeterminate (no reported percent). */
  percent?: number | null;
  message?: string;
  error?: string;
  /** Budget badges: reservation + effective caps as decimal strings. */
  ramBytes?: string;
  vramBytes?: string;
  cpuThreads?: number;
  cpuSeconds?: string;
  memoryBytes?: string;
  /** Set when a late/foreign result was quarantined (never applied). */
  quarantined?: boolean;
  /** Committed artifact count for a succeeded job. */
  artifactCount?: number;
  onCancel?: (jobId: string) => void;
}

const STATUS_LABEL: Record<JobStatus, string> = {
  queued: 'Queued',
  running: 'Running',
  cancelling: 'Cancelling…',
  succeeded: 'Done',
  failed: 'Failed',
  cancelled: 'Cancelled',
};

const KIND_LABEL: Record<JobKind, string> = {
  symbolic: 'Symbolic',
  transcription: 'Transcription',
  separation: 'Separation',
  audio_generation: 'Audio gen',
  visual_generation: 'Visual gen',
  analysis: 'Analysis',
  av_export: 'Export',
};

const statusColor = (s: JobStatus): string =>
  s === 'failed' ? tokens.danger
    : s === 'succeeded' ? tokens.ok
    : s === 'cancelled' ? tokens.textMuted
    : s === 'cancelling' ? tokens.warn
    : tokens.accent;

/** One row of the jobs list — status, budget badges, live progress. */
export const JobRow: React.FC<JobRowProps> = React.memo(
  ({
    jobId, name, kind = 'analysis', status, percent = null, message, error,
    ramBytes, vramBytes, cpuThreads, cpuSeconds, memoryBytes,
    quarantined = false, artifactCount, onCancel,
  }) => {
    const busy = status === 'queued' || status === 'running' || status === 'cancelling';
    const frac = percent === null ? null : Math.min(1, Math.max(0, percent / 100));
    return (
      <div
        role="listitem"
        aria-label={`${KIND_LABEL[kind]} job ${name}: ${STATUS_LABEL[status]}`}
        style={{
          display: 'flex', flexDirection: 'column', gap: 4, padding: '6px 8px',
          background: tokens.surface, border: `1px solid ${tokens.border}`,
          borderRadius: tokens.radius, fontFamily: tokens.sans, color: tokens.text,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span style={{ fontSize: 9, color: tokens.textMuted, textTransform: 'uppercase', letterSpacing: '0.06em' }}>
            {KIND_LABEL[kind]}
          </span>
          <span style={{ flex: 1, fontSize: 12, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={jobId}>
            {name}
          </span>
          <span style={{ fontSize: 10, color: statusColor(status) }}>{STATUS_LABEL[status]}</span>
          {busy && onCancel ? (
            <Button variant="ghost" onClick={() => onCancel(jobId)} aria-label={`Cancel job ${name}`}>
              Cancel
            </Button>
          ) : null}
        </div>
        {(ramBytes || cpuThreads || cpuSeconds || memoryBytes) ? (
          <div style={{ display: 'flex', gap: 4, flexWrap: 'wrap' }}>
            {ramBytes ? <BudgetBadge label="RAM" bytes={ramBytes} /> : null}
            {vramBytes && vramBytes !== '0' ? <BudgetBadge label="VRAM" bytes={vramBytes} /> : null}
            {cpuThreads ? <BudgetBadge label="CPU" threads={cpuThreads} /> : null}
            {cpuSeconds ? <BudgetBadge label="CPU·s" seconds={cpuSeconds} /> : null}
            {memoryBytes ? <BudgetBadge label="CAP" bytes={memoryBytes} /> : null}
          </div>
        ) : null}
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
                transition: 'width 120ms ease-out',
              }}
            />
          </div>
        ) : null}
        {message && busy ? (
          <span style={{ fontSize: 10, color: tokens.textSecondary }}>{message}</span>
        ) : null}
        {error ? (
          <span role="alert" style={{ fontSize: 10, color: tokens.danger }}>{error}</span>
        ) : null}
        {quarantined ? (
          <span style={{ fontSize: 9, color: tokens.warn }}>
            late result quarantined — not applied
          </span>
        ) : null}
        {status === 'succeeded' && artifactCount !== undefined ? (
          <span style={{ fontSize: 10, color: tokens.textSecondary }}>
            {artifactCount} artifact{artifactCount === 1 ? '' : 's'}
          </span>
        ) : null}
      </div>
    );
  },
);
JobRow.displayName = 'JobRow';
