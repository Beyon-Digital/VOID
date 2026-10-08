import React from 'react';
import { tokens } from './styles';
import { BudgetBadge, formatNanos } from './BudgetBadge';

/** Model registry statuses — mirror crates/void-models ModelStatus. */
export type ModelStatus = 'available' | 'degraded' | 'missing' | 'rejected';
export type ModelKind = 'symbolic' | 'audio' | 'visual';
export type RuntimeKind = 'argv' | 'internal';

export interface ModelRowProps {
  modelId: string;
  name: string;
  version: string;
  kind: ModelKind;
  runtime: RuntimeKind;
  status: ModelStatus;
  /** Budget defaults — decimal strings. */
  cpuSeconds?: string;
  memoryBytes?: string;
  wallNs?: string;
  cpuThreads?: number;
  artifactCount?: number;
  missingArtifacts?: number;
  license?: string;
  onSelect?: (modelId: string) => void;
}

const STATUS_LABEL: Record<ModelStatus, string> = {
  available: 'Available',
  degraded: 'Degraded',
  missing: 'Not installed',
  rejected: 'Rejected',
};

const statusColor = (s: ModelStatus): string =>
  s === 'available' ? tokens.ok
    : s === 'degraded' ? tokens.warn
    : tokens.textMuted;

/** Whether the model may be picked for a new job (T51 honest state). */
export function modelRunnable(status: ModelStatus): boolean {
  return status === 'available' || status === 'degraded';
}

const ModelRow: React.FC<ModelRowProps> = React.memo(
  ({ modelId, name, version, kind, runtime, status, cpuSeconds, memoryBytes, wallNs, cpuThreads, artifactCount, missingArtifacts = 0, license, onSelect }) => {
    const runnable = modelRunnable(status);
    return (
      <div
        role="listitem"
        aria-label={`Model ${name} ${version}: ${STATUS_LABEL[status]}`}
        style={{
          display: 'flex', flexDirection: 'column', gap: 4, padding: '6px 8px',
          background: tokens.surface, border: `1px solid ${tokens.border}`,
          borderRadius: tokens.radius, fontFamily: tokens.sans, color: tokens.text,
          opacity: runnable ? 1 : 0.6,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span style={{ flex: 1, fontSize: 12, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={`${modelId}@${version}`}>
            {name}
            <span style={{ color: tokens.textMuted, fontSize: 10 }}> @{version}</span>
          </span>
          <span style={{ fontSize: 9, color: tokens.textMuted, textTransform: 'uppercase' }}>{kind}·{runtime}</span>
          <span style={{ fontSize: 10, color: statusColor(status) }}>{STATUS_LABEL[status]}</span>
          {onSelect ? (
            <button
              type="button"
              disabled={!runnable}
              onClick={() => onSelect(modelId)}
              aria-label={`Use model ${name}`}
              style={{
                fontSize: 10, fontFamily: tokens.sans, cursor: runnable ? 'pointer' : 'not-allowed',
                color: runnable ? tokens.accent : tokens.textMuted,
                background: 'transparent', border: `1px solid ${runnable ? tokens.accent : tokens.border}`,
                borderRadius: tokens.radius, padding: '1px 6px',
              }}
            >
              Use
            </button>
          ) : null}
        </div>
        <div style={{ display: 'flex', gap: 4, flexWrap: 'wrap', alignItems: 'center' }}>
          {cpuSeconds ? <BudgetBadge label="CPU·s" seconds={cpuSeconds} /> : null}
          {memoryBytes ? <BudgetBadge label="MEM" bytes={memoryBytes} /> : null}
          {cpuThreads ? <BudgetBadge label="CPU" threads={cpuThreads} /> : null}
          {wallNs && wallNs !== '0' ? (
            <span style={{ fontSize: 9, fontFamily: tokens.mono, color: tokens.textMuted }}>
              wall {formatNanos(wallNs)}
            </span>
          ) : null}
          {missingArtifacts > 0 ? (
            <span role="alert" style={{ fontSize: 9, color: tokens.warn }}>
              {missingArtifacts} artifact{missingArtifacts === 1 ? '' : 's'} missing
            </span>
          ) : null}
          {license ? (
            <span style={{ fontSize: 9, color: tokens.textMuted }}>{license}</span>
          ) : null}
        </div>
      </div>
    );
  },
);
ModelRow.displayName = 'ModelRow';

export interface ModelRegistryListProps {
  models: ModelRowProps[];
  /** Empty-state copy — honest, no fake rows. */
  emptyLabel?: string;
  onSelect?: (modelId: string) => void;
}

/** The registry list — rows straight from the MODEL_LIST read view. */
export const ModelRegistryList: React.FC<ModelRegistryListProps> = React.memo(
  ({ models, emptyLabel = 'No models installed', onSelect }) => {
    if (models.length === 0) {
      return (
        <div
          role="status"
          style={{
            padding: '12px 8px', fontSize: 11, color: tokens.textMuted,
            fontFamily: tokens.sans, textAlign: 'center',
          }}
        >
          {emptyLabel}
        </div>
      );
    }
    return (
      <div role="list" aria-label="Model registry" style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
        {models.map((m) => (
          <ModelRow key={`${m.modelId}@${m.version}`} {...m} onSelect={onSelect} />
        ))}
      </div>
    );
  },
);
ModelRegistryList.displayName = 'ModelRegistryList';
