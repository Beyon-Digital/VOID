// Suggestion inspector — proposal lifecycle states for the Compose
// workspace (S02 preview, S25 auditioning, S26 partial accept,
// S03 accepted, S20 stale).
//
// Every affordance is wired to the real path or gated with an honest
// reason. Accepted edits mint real InsertNoteOps under one transaction
// id; the undo affordance calls client.undo on that transaction.

import * as React from 'react';
import { Button, ProposalCard, StatusBadge, tokens } from 'void-ui';
import type { ProposalRecord } from 'void-studio/src/proposals/types';
import { formatBarBeat } from 'void-ui';
import { Card, Eyebrow, Mono, ReasonNote, Row, Stack } from './chrome';
import { PROPOSAL_OPS, REASONS } from './flags';
import {
  acceptProposal,
  auditionProposal,
  dismiss,
  proposalsStore,
  requestSuggestion,
  undoAccept,
  useProposals,
} from './runtime';

function recStatus(rec: ProposalRecord): {
  status: Parameters<typeof StatusBadge>[0]['status'];
  label: string;
} {
  switch (rec.status) {
    case 'pending':
      return { status: 'processing', label: 'Generating…' };
    case 'ready':
      return { status: 'ready', label: 'Preview only' };
    case 'stale':
      return { status: 'unavailable', label: 'Stale preview' };
    case 'accepted':
      return { status: 'saved', label: 'Committed' };
    case 'rejected':
      return { status: 'unavailable', label: 'Rejected' };
    case 'failed':
      return { status: 'error', label: 'Failed' };
  }
}

function spanLabel(rec: ProposalRecord, rank: number): string {
  const cand = rec.candidates.find((c) => c.rank === rank) ?? rec.candidates[0];
  if (!cand || cand.notes.length === 0) return '—';
  const last = cand.notes.reduce((a, b) =>
    BigInt(a.onsetTicks) > BigInt(b.onsetTicks) ? a : b,
  );
  const end = BigInt(last.onsetTicks) + BigInt(last.lengthTicks);
  return `${formatBarBeat(end.toString(10))}`;
}

export const SuggestionPanel: React.FC<{ proposalId?: string }> = ({
  proposalId,
}) => {
  const records = useProposals((s) => s.records);
  const order = useProposals((s) => s.order);
  const sel = useProposals((s) => s.selection);
  const busy = useProposals((s) => s.busy);
  const rec = proposalId ? records[proposalId] : undefined;
  const list = order
    .map((id) => records[id])
    .filter((r): r is ProposalRecord => !!r);
  const [flash, setFlash] = React.useState<string | null>(null);
  const [auditioning, setAuditioning] = React.useState(false);

  const notify = (msg: string) => {
    setFlash(msg);
    window.setTimeout(() => setFlash((m) => (m === msg ? null : m)), 5000);
  };

  /** RequestProposalOp — the digest binds request → live selection so
   * a coordinator never pairs the ask with stale context. */
  const ask = () =>
    void requestSuggestion()
      .then((r) => {
        if (r.status === 'REJECTED' || r.status === 'OUTCOME_UNKNOWN')
          notify(`suggestion request ${r.status.toLowerCase()} — ${r.message || 'no detail'}`);
      })
      .catch((e) => notify(String(e instanceof Error ? e.message : e)));

  if (!rec) {
    return (
      <Stack gap={10}>
        <Eyebrow>Suggestion</Eyebrow>
        <Card>
          <div style={{ fontSize: 12, color: tokens.text, fontWeight: 600 }}>
            No suggestion yet
          </div>
          <ReasonNote>
            Ask the engine for a continuation of the selected clip. The
            preview shows as dashed ghost notes — originals stay locked.
          </ReasonNote>
          <Button
            variant="primary"
            size="sm"
            disabled={!PROPOSAL_OPS.request}
            title={PROPOSAL_OPS.request ? 'RequestProposalOp on the selected clip' : REASONS.proposalRequest}
            onClick={ask}
          >
            Suggest a continuation
          </Button>
          {!PROPOSAL_OPS.request ? <ReasonNote>{REASONS.proposalRequest}</ReasonNote> : null}
        </Card>
        {list.length > 0 ? (
          <Card>
            <Eyebrow>Recent</Eyebrow>
            {list.slice(0, 4).map((r) => (
              <Row key={r.proposalId} justify="space-between">
                <Mono>{r.proposalId.slice(0, 8)}</Mono>
                <StatusBadge
                  status={recStatus(r).status}
                  label={recStatus(r).label}
                />
              </Row>
            ))}
          </Card>
        ) : null}
      </Stack>
    );
  }

  const st = recStatus(rec);
  const selRank =
    sel && sel.proposalId === rec.proposalId ? sel.candidateRank : undefined;
  const pickedCount =
    sel && sel.proposalId === rec.proposalId ? sel.indices.length : 0;

  return (
    <div role="region" aria-label="Suggestion" style={{ display: 'flex', flexDirection: 'column', gap: 10 }}>
      <Row justify="space-between">
        <Eyebrow>Suggestion</Eyebrow>
        <StatusBadge status={st.status} label={st.label} />
      </Row>

      {flash ? (
        <Card pad={8} role="status" aria-label="Result">
          <ReasonNote>{flash}</ReasonNote>
        </Card>
      ) : null}

      {rec.status === 'stale' ? (
        <Card>
          <div style={{ fontSize: 12, color: tokens.warn, fontWeight: 600 }}>
            The context changed.
          </div>
          <ReasonNote>
            {rec.staleCause === 'target_gone'
              ? 'The clip this suggestion was generated from was removed.'
              : rec.staleCause === 'session_ended'
                ? 'The engine session ended.'
                : rec.staleCause === 'superseded'
                  ? 'A newer suggestion replaced this one.'
                  : rec.staleCause === 'cancelled'
                    ? 'Generation was cancelled.'
                    : 'The region changed since generation.'}{' '}
            Acceptance is refused — never silently rebase musical edits.
          </ReasonNote>
          <Button
            variant="secondary"
            size="sm"
            disabled={!PROPOSAL_OPS.request}
            title={PROPOSAL_OPS.request ? 'RequestProposalOp — re-ask over current context' : REASONS.proposalRequest}
            onClick={ask}
          >
            Refresh suggestion
          </Button>
          {!PROPOSAL_OPS.request ? <ReasonNote>{REASONS.proposalRequest}</ReasonNote> : null}
          <Button
            variant="ghost"
            size="sm"
            onClick={() => dismiss(rec.proposalId)}
          >
            Dismiss preview
          </Button>
        </Card>
      ) : null}

      {rec.status === 'accepted' ? (
        <Card>
          <div style={{ fontSize: 12, color: tokens.text, fontWeight: 600 }}>
            {rec.accepted?.noteIds.length ?? 0} notes written as your edit
          </div>
          {rec.accepted?.droppedIndices.length ? (
            <ReasonNote>
              {rec.accepted.droppedIndices.length} proposed note(s) overlapped
              locked ranges and were not committed.
            </ReasonNote>
          ) : null}
          <Mono>tx {rec.accepted?.transactionId.slice(0, 8)}</Mono>
          <Button
            variant="secondary"
            size="sm"
            loading={busy}
            onClick={() =>
              void undoAccept(rec.proposalId).then((r) => {
                if (!r.ok) notify(r.reason ?? 'undo failed');
              })
            }
          >
            Undo acceptance
          </Button>
          <ReasonNote>
            One transaction — one undo. Notes stay editable like any others.
          </ReasonNote>
        </Card>
      ) : null}

      {rec.status === 'failed' ? (
        <Card>
          <StatusBadge status="error" label="Generation failed" />
          <ReasonNote>{rec.error ?? 'The generator reported a failure.'}</ReasonNote>
          <Button
            variant="secondary"
            size="sm"
            disabled={!PROPOSAL_OPS.request}
            title={PROPOSAL_OPS.request ? 'RequestProposalOp' : REASONS.proposalRequest}
            onClick={ask}
          >
            Try again
          </Button>
          {!PROPOSAL_OPS.request ? <ReasonNote>{REASONS.proposalRequest}</ReasonNote> : null}
        </Card>
      ) : null}

      {rec.status === 'pending' ? (
        <Card>
          <ReasonNote>
            Waiting for the generator — the job appears in Jobs when the
            coordinator emits it.
          </ReasonNote>
        </Card>
      ) : null}

      {rec.status === 'ready' ? (
        <>
          <Card>
            <Eyebrow>Context</Eyebrow>
            <Row justify="space-between">
              <ReasonNote>Clip</ReasonNote>
              <Mono>{rec.context?.clipId ?? '—'}</Mono>
            </Row>
            <Row justify="space-between">
              <ReasonNote>Source revision</ReasonNote>
              <Mono>r{rec.sourceRevision}</Mono>
            </Row>
            {(rec.context?.lockedRanges?.length ?? 0) > 0 ? (
              <ReasonNote>
                {rec.context?.lockedRanges?.length} locked range(s) — proposed
                notes overlapping them are muted and cannot be accepted.
              </ReasonNote>
            ) : null}
          </Card>

          <Card>
            <Row justify="space-between">
              <Eyebrow>Candidates</Eyebrow>
              <ReasonNote>{rec.candidates.length} ranked</ReasonNote>
            </Row>
            {rec.candidates.map((c) => {
              const isSel =
                sel?.proposalId === rec.proposalId && sel.candidateRank === c.rank;
              return (
                <ProposalCard
                  key={c.rank}
                  proposalId={rec.proposalId}
                  rank={c.rank}
                  of={rec.candidates.length}
                  score={c.score}
                  rationale={c.rationale}
                  noteCount={c.notes.length}
                  spanTicks={spanLabel(rec, c.rank)}
                  previewing={isSel}
                  status="ready"
                  generatorId={rec.provenance?.generatorId}
                  modelId={rec.provenance?.modelId}
                  seed={rec.provenance?.seed}
                  selected={isSel}
                  onSelect={(pid, rank) =>
                    proposalsStore.getState().actions.select(pid, rank)
                  }
                />
              );
            })}
          </Card>

          <Card>
            <Row gap={6} wrap>
              <Button
                variant="secondary"
                size="sm"
                disabled={!PROPOSAL_OPS.audition}
                title={
                  PROPOSAL_OPS.audition
                    ? auditioning
                      ? 'PreviewLayerOp{enable:false} — stop the preview'
                      : 'PreviewLayerOp{enable:true} — hear the ghost notes'
                    : REASONS.audition
                }
                onClick={() =>
                  void auditionProposal(rec.proposalId, !auditioning)
                    .then((r) => {
                      if (r === null) return;
                      if (r.status === 'APPLIED' || r.status === 'DUPLICATE')
                        setAuditioning((a) => !a);
                      else notify(`audition ${r.status.toLowerCase()} — ${r.message || 'no detail'}`);
                    })
                    .catch((e) => notify(String(e instanceof Error ? e.message : e)))
                }
              >
                {auditioning ? 'Stop audition' : 'Audition'}
              </Button>
              <Button
                variant="primary"
                size="sm"
                loading={busy}
                onClick={() =>
                  void acceptProposal(rec.proposalId).then((r) => {
                    if (!r.ok) notify(r.reason ?? 'accept failed');
                  })
                }
              >
                {pickedCount > 0 ? `Accept ${pickedCount} selected` : 'Accept all'}
              </Button>
              <Button
                variant="ghost"
                size="sm"
                disabled={busy}
                onClick={() => dismiss(rec.proposalId)}
              >
                Keep my original
              </Button>
            </Row>
            <ReasonNote>
              {REASONS.audition} Click ghost notes to choose a subset — only
              those notes are committed.
            </ReasonNote>
            {selRank !== undefined ? (
              <ReasonNote>
                Previewing candidate {selRank} — span {spanLabel(rec, selRank)}.
                Accept commits exactly the selected notes as one undoable
                transaction.
              </ReasonNote>
            ) : null}
          </Card>
        </>
      ) : null}
    </div>
  );
};
