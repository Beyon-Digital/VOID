import React from 'react';
import { tokens } from './styles';
import { Button } from './Button';
import { formatBarBeat } from './ticks';

/** Proposal lifecycle — mirrors crates/void-proposals (snake_case). */
export type ProposalStatus =
  | 'pending'
  | 'ready'
  | 'stale'
  | 'accepted'
  | 'rejected'
  | 'failed';

export type StaleCause =
  | 'context_changed'
  | 'target_gone'
  | 'session_ended'
  | 'superseded'
  | 'cancelled';

export interface ProposalCardProps {
  proposalId: string;
  /** Candidate rank (1-based) as scored by the model. */
  rank: number;
  /** Total candidates in the proposal. */
  of: number;
  /** Model's own score — shown verbatim, never re-labeled. */
  score: number;
  /** Measured-facts rationale from the generator. */
  rationale: string;
  /** Note count + continuation span for the preview line. */
  noteCount: number;
  spanTicks: string;
  /** True when this card's candidate is the ghost-preview selection. */
  previewing?: boolean;
  /** Count of notes dropped for locked-range overlap. */
  lockedCount?: number;
  status: ProposalStatus;
  staleCause?: StaleCause;
  /** Generator id + model (provenance line, shown verbatim). */
  generatorId?: string;
  modelId?: string;
  seed?: string;
  selected?: boolean;
  onSelect?: (proposalId: string, rank: number) => void;
  onAccept?: (proposalId: string, rank: number) => void;
  onReject?: (proposalId: string) => void;
}

const STATUS_LABEL: Record<ProposalStatus, string> = {
  pending: 'Generating…',
  ready: 'Ready',
  stale: 'Stale',
  accepted: 'Accepted',
  rejected: 'Rejected',
  failed: 'Failed',
};

const CAUSE_LABEL: Record<StaleCause, string> = {
  context_changed: 'region changed since generation',
  target_gone: 'target clip removed',
  session_ended: 'session ended',
  superseded: 'superseded by a newer proposal',
  cancelled: 'generation cancelled',
};

const statusColor = (s: ProposalStatus): string =>
  s === 'failed' || s === 'rejected'
    ? tokens.danger
    : s === 'accepted'
      ? tokens.ok
      : s === 'stale'
        ? tokens.warn
        : s === 'pending'
          ? tokens.accent
          : tokens.textSecondary;

/** One ranked proposal candidate — rank, honest score, rationale, and
 * the accept/reject affordances. Terminal states render their cause;
 * nothing here applies edits — callbacks express intent only. */
export const ProposalCard: React.FC<ProposalCardProps> = React.memo(
  ({
    proposalId,
    rank,
    of,
    score,
    rationale,
    noteCount,
    spanTicks,
    previewing = false,
    lockedCount = 0,
    status,
    staleCause,
    generatorId,
    modelId,
    seed,
    selected = false,
    onSelect,
    onAccept,
    onReject,
  }) => {
    const live = status === 'ready';
    return (
      <div
        role="listitem"
        aria-label={`Proposal ${rank} of ${of}: ${STATUS_LABEL[status]}`}
        aria-selected={selected}
        onClick={onSelect ? () => onSelect(proposalId, rank) : undefined}
        onKeyDown={
          onSelect
            ? (e) => {
                if (e.key === 'Enter' || e.key === ' ') {
                  e.preventDefault();
                  onSelect(proposalId, rank);
                }
              }
            : undefined
        }
        tabIndex={onSelect ? 0 : -1}
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: 4,
          padding: '6px 8px',
          cursor: onSelect ? 'pointer' : 'default',
          background: selected ? tokens.accentSoft : tokens.surface,
          border: `1px solid ${selected ? tokens.accent : tokens.border}`,
          borderRadius: tokens.radius,
          fontFamily: tokens.sans,
          color: tokens.text,
        }}
      >
        <div style={{ display: 'flex', alignItems: 'center', gap: 8 }}>
          <span
            style={{
              fontSize: 9,
              color: tokens.textMuted,
              textTransform: 'uppercase',
              letterSpacing: '0.06em',
            }}
          >
            #{rank}/{of}
          </span>
          <span
            style={{
              flex: 1,
              fontSize: 12,
              overflow: 'hidden',
              textOverflow: 'ellipsis',
              whiteSpace: 'nowrap',
            }}
            title={rationale}
          >
            {rationale || `candidate ${rank}`}
          </span>
          <span style={{ fontSize: 10, color: tokens.textSecondary }}>
            score {score.toFixed(3)}
          </span>
          <span style={{ fontSize: 10, color: statusColor(status) }}>
            {STATUS_LABEL[status]}
          </span>
        </div>
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: 8,
            fontSize: 10,
            color: tokens.textSecondary,
          }}
        >
          <span>
            {noteCount} notes · {formatBarBeat(spanTicks)}
          </span>
          {lockedCount > 0 ? (
            <span style={{ color: tokens.warn }}>
              {lockedCount} locked — excluded
            </span>
          ) : null}
          {previewing ? (
            <span style={{ color: tokens.accent }}>ghost preview</span>
          ) : null}
        </div>
        {staleCause ? (
          <span style={{ fontSize: 9, color: tokens.warn }}>
            stale — {CAUSE_LABEL[staleCause]}
          </span>
        ) : null}
        {generatorId || modelId ? (
          <span style={{ fontSize: 9, color: tokens.textMuted }}>
            {generatorId}
            {modelId ? ` · ${modelId}` : ''}
            {seed ? ` · seed ${seed}` : ''}
          </span>
        ) : null}
        {live && (onAccept || onReject) ? (
          <div style={{ display: 'flex', gap: 4, marginTop: 2 }}>
            {onAccept ? (
              <Button
                variant="primary"
                onClick={(e) => {
                  e.stopPropagation();
                  onAccept(proposalId, rank);
                }}
                aria-label={`Accept proposal ${rank}`}
              >
                Accept
              </Button>
            ) : null}
            {onReject ? (
              <Button
                variant="ghost"
                onClick={(e) => {
                  e.stopPropagation();
                  onReject(proposalId);
                }}
                aria-label={`Reject proposal ${rank}`}
              >
                Reject
              </Button>
            ) : null}
          </div>
        ) : null}
      </div>
    );
  },
);
ProposalCard.displayName = 'ProposalCard';
