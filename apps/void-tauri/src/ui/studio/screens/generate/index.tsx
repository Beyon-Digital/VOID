// Generate screen — audio candidates (S10).
//
// Candidates arrive as generation records from the jobs/generation
// stores (real coordinator job events). The request form is honest:
// submitting a job needs the coordinator SubmitJob op, absent in
// protocol major.1 — the button stays disabled with the reason shown.
//
// "Use this audio" is REAL: AttachAssetOp (sha256 + container-relative
// path + media type from the artifact record) then InsertAudioClipOp
// on the selected track — one transaction, undoable as a unit.

import * as React from 'react';
import { Button, StatusBadge, tokens } from 'void-ui';
import type { GenerationRecord } from 'void-studio/src/generation/store';
import type { ModelRow } from 'void-studio/src/jobs/models';
import { getClient } from '../../../client';
import {
  useEditor,
  useProjectSummary,
  useStudioCompactContext,
} from '../../useStudioData';
import {
  Card,
  Divider,
  Eyebrow,
  Mono,
  ReasonNote,
  Row,
  Stack,
} from '../../uip4/chrome';
import { REASONS } from '../../uip4/flags';
import {
  generationStore,
  go,
  useFeatureStores,
  useGeneration,
  useModelRows,
  useSelectedTrackClips,
} from '../../uip4/runtime';

function mediaTypeOf(name: string): string {
  const ext = name.split('.').pop()?.toLowerCase() ?? '';
  if (ext === 'wav') return 'wav';
  if (ext === 'aif' || ext === 'aiff') return 'aiff';
  if (ext === 'mid' || ext === 'midi') return 'midi';
  if (ext === 'flac') return 'flac';
  if (ext === 'mp3') return 'mp3';
  return ext || 'bin';
}

function recStatus(s: GenerationRecord['status']) {
  switch (s) {
    case 'queued':
      return { status: 'queued' as const, label: 'Queued' };
    case 'running':
      return { status: 'processing' as const, label: 'Generating…' };
    case 'succeeded':
      return { status: 'saved' as const, label: 'Done' };
    case 'failed':
      return { status: 'error' as const, label: 'Failed' };
    case 'cancelled':
      return { status: 'unavailable' as const, label: 'Cancelled' };
  }
}

const CandidateCard: React.FC<{
  rec: GenerationRecord;
  onUse: (rec: GenerationRecord, sha256: string) => Promise<void>;
}> = ({ rec, onUse }) => {
  const [busy, setBusy] = React.useState<string | null>(null);
  const st = recStatus(rec.status);
  return (
    <Card>
      <Row justify="space-between">
        <Mono>{rec.modelId ?? 'model —'}</Mono>
        <StatusBadge status={st.status} label={st.label} />
      </Row>
      {rec.prompt ? <ReasonNote>“{rec.prompt}”</ReasonNote> : null}
      {rec.error ? (
        <ReasonNote>{rec.error}</ReasonNote>
      ) : null}
      {rec.artifacts.length > 0 ? (
        <Stack gap={4}>
          {rec.artifacts.map((a) => {
            const accepted = rec.acceptedSha256 === a.sha256;
            return (
              <Row key={a.sha256} justify="space-between">
                <Stack gap={2} style={{ minWidth: 0 }}>
                  <Mono>{a.name}</Mono>
                  <ReasonNote>
                    {a.assetRel ? 'container asset' : 'artifact'} ·{' '}
                    {a.sha256.slice(0, 12)}…
                  </ReasonNote>
                </Stack>
                <Row gap={4}>
                  <Button
                    variant="secondary"
                    size="sm"
                    disabled
                    title={REASONS.assetAudition}
                  >
                    Audition
                  </Button>
                  {accepted ? (
                    <StatusBadge status="saved" label="Accepted" />
                  ) : (
                    <Button
                      variant="primary"
                      size="sm"
                      loading={busy === a.sha256}
                      disabled={!a.assetRel}
                      title={
                        a.assetRel
                          ? 'Attach the asset and insert an audio clip on the selected track'
                          : 'Artifact has no committed container asset yet'
                      }
                      onClick={() => {
                        setBusy(a.sha256);
                        void onUse(rec, a.sha256).finally(() => setBusy(null));
                      }}
                    >
                      Use this audio
                    </Button>
                  )}
                </Row>
              </Row>
            );
          })}
        </Stack>
      ) : rec.status === 'succeeded' ? (
        <ReasonNote>No artifacts committed.</ReasonNote>
      ) : null}
      {rec.status === 'failed' ? (
        <ReasonNote>
          The job failed — the error above is what the runner reported.
          Nothing was written to the project.
        </ReasonNote>
      ) : null}
    </Card>
  );
};

export default function GenerateScreen() {
  const compact = useStudioCompactContext();
  useFeatureStores();
  const summary = useProjectSummary();
  const models = useModelRows();
  const audioModels = models.rows.filter(
    (m) => m.kind === 'audio' && (m.status === 'available' || m.status === 'degraded'),
  );
  const clips = useSelectedTrackClips();
  const trackId = useEditor((s) => s.clipSelection.trackId);
  const records = useGeneration((s) => s.records);
  const order = useGeneration((s) => s.order);
  const [prompt, setPrompt] = React.useState('');
  const [duration, setDuration] = React.useState('8');
  const [modelId, setModelId] = React.useState<string | null>(null);
  const [flash, setFlash] = React.useState<string | null>(null);

  const chosen = audioModels.find((m) => m.modelId === modelId) ?? audioModels[0];

  /** Attach the accepted artifact then insert it as an audio clip —
   * one transaction. */
  const useAudio = async (rec: GenerationRecord, sha256: string) => {
    const art = rec.artifacts.find((a) => a.sha256 === sha256);
    if (!art?.assetRel) {
      setFlash('artifact has no committed container asset');
      return;
    }
    if (!trackId) {
      setFlash('select a track in Arrange first — no destination');
      return;
    }
    const client = getClient();
    const tx = crypto.randomUUID();
    const assetId = crypto.randomUUID();
    try {
      const attach = await client.sendCommand(
        {
          AttachAssetOp: {
            asset_id: assetId,
            sha256: art.sha256,
            media_type: mediaTypeOf(art.name),
            rel_path: art.assetRel,
          },
        },
        { transactionId: tx },
      );
      if (attach.status !== 'APPLIED' && attach.status !== 'DUPLICATE') {
        setFlash(`attach rejected: ${attach.status}`);
        return;
      }
      const clipId = crypto.randomUUID();
      const insert = await client.sendCommand(
        {
          InsertAudioClipOp: {
            clip_id: clipId,
            track_id: trackId,
            asset_id: assetId,
            start_ticks: '0',
            length_ticks: '3840000',
            offset_ticks: '0',
          },
        },
        { transactionId: tx },
      );
      if (insert.status !== 'APPLIED' && insert.status !== 'DUPLICATE') {
        setFlash(`clip insert rejected: ${insert.status}`);
        return;
      }
      generationStore.getState().actions.acceptResult(rec.jobId, sha256);
      setFlash('Audio placed as a clip — one undo reverts it.');
    } catch (e) {
      setFlash(String(e));
    }
  };

  const form = (
    <Card>
      <Eyebrow>Generate audio</Eyebrow>
      <textarea
        value={prompt}
        onChange={(e) => setPrompt(e.target.value)}
        placeholder="Describe the take — e.g. a slow ambient pad"
        aria-label="Generation prompt"
        rows={3}
        style={{
          background: tokens.surface,
          color: tokens.text,
          border: `1px solid ${tokens.line}`,
          borderRadius: tokens.radius4,
          padding: '6px 8px',
          fontFamily: tokens.sans,
          fontSize: 12,
          resize: 'vertical',
        }}
      />
      <Row justify="space-between">
        <ReasonNote>Duration</ReasonNote>
        <Row gap={4}>
          {['4', '8', '16'].map((d) => (
            <Button
              key={d}
              variant={duration === d ? 'secondary' : 'ghost'}
              size="sm"
              onClick={() => setDuration(d)}
            >
              {d} bars
            </Button>
          ))}
        </Row>
      </Row>
      <Row justify="space-between">
        <ReasonNote>Tempo</ReasonNote>
        <Mono>{summary.bpm !== null ? `${summary.bpm} bpm (project)` : '—'}</Mono>
      </Row>
      <Row justify="space-between">
        <ReasonNote>Destination</ReasonNote>
        <Mono>
          {trackId
            ? `track ${trackId.slice(0, 8)}${clips.length ? ` · ${clips.length} clips` : ''}`
            : 'select a track'}
        </Mono>
      </Row>
      <Row justify="space-between">
        <ReasonNote>Model</ReasonNote>
        {audioModels.length ? (
          <select
            value={chosen?.modelId ?? ''}
            onChange={(e) => setModelId(e.target.value)}
            aria-label="Audio model"
            style={{
              background: tokens.raised,
              color: tokens.text,
              border: `1px solid ${tokens.line}`,
              borderRadius: tokens.radius4,
              padding: '4px 6px',
              fontFamily: tokens.sans,
              fontSize: 12,
            }}
          >
            {audioModels.map((m: ModelRow) => (
              <option key={`${m.modelId}@${m.version}`} value={m.modelId}>
                {m.name} · {m.status}
              </option>
            ))}
          </select>
        ) : (
          <Mono>none installed</Mono>
        )}
      </Row>
      <Button
        variant="primary"
        size="sm"
        disabled
        title={REASONS.jobSubmit}
      >
        Generate 3 candidates
      </Button>
      <ReasonNote>
        {REASONS.jobSubmit} {REASONS.modelInstall}
      </ReasonNote>
    </Card>
  );

  const results = (
    <Stack gap={10} style={{ flex: 1, minHeight: 0 }}>
      {flash ? (
        <Card pad={8} role="status">
          <ReasonNote>{flash}</ReasonNote>
        </Card>
      ) : null}
      {order.length === 0 ? (
        <Card>
          <Eyebrow>Candidates</Eyebrow>
          <ReasonNote>
            No generation jobs yet. Candidates land here with their
            provenance (model, seed, job id); accepting one writes a real
            project asset — alternates stay immutable.
          </ReasonNote>
        </Card>
      ) : (
        order.map((id) => {
          const rec = records[id];
          if (!rec) return null;
          return <CandidateCard key={id} rec={rec} onUse={useAudio} />;
        })
      )}
      <Button variant="ghost" size="sm" onClick={() => go('jobs')}>
        Open Jobs →
      </Button>
    </Stack>
  );

  const rightRail = (
    <Stack gap={10}>
      <Eyebrow>Provenance</Eyebrow>
      <Card>
        <ReasonNote>
          Accepted candidates become immutable project assets — sha256,
          model id, seed, and job id are recorded on the generation
          record. The alternates are never deleted or rewritten.
        </ReasonNote>
      </Card>
      <Card>
        <Eyebrow>Permissions</Eyebrow>
        <ReasonNote>
          Generators run as local workers with bounded budgets. No model
          downloads happen automatically; network access is only what
          you start.
        </ReasonNote>
      </Card>
    </Stack>
  );

  const center = (
    <div
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        overflowY: 'auto',
        padding: 12,
        display: 'flex',
        flexDirection: 'column',
        gap: 10,
      }}
    >
      {form}
      {results}
    </div>
  );

  if (compact) {
    return (
      <div
        style={{
          display: 'flex',
          flexDirection: 'column',
          flex: 1,
          minHeight: 0,
          overflowY: 'auto',
          padding: 12,
          gap: 12,
        }}
      >
        {center}
        {rightRail}
      </div>
    );
  }

  return (
    <div style={{ display: 'flex', flex: 1, minWidth: 0, minHeight: 0 }}>
      <aside
        aria-label="Generate"
        style={{
          width: 300,
          flexShrink: 0,
          borderRight: `1px solid ${tokens.line}`,
          overflowY: 'auto',
          padding: '12px 10px',
        }}
      >
        <Stack gap={10}>
          <Eyebrow>Generate</Eyebrow>
          {form}
        </Stack>
      </aside>
      {center}
      <aside
        aria-label="Provenance"
        style={{
          width: 240,
          flexShrink: 0,
          borderLeft: `1px solid ${tokens.line}`,
          overflowY: 'auto',
          padding: '12px 10px',
        }}
      >
        {rightRail}
      </aside>
    </div>
  );
}
