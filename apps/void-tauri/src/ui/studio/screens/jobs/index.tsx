// Jobs screen — job queue + model registry (S17).
//
// Cards are real coordinator job events folded into the jobs store.
// Submit/cancel/pause/model-install ops don't exist in protocol
// major.1, so those affordances render disabled with their reason —
// nothing fakes a submission or a cancellation (UI-T19/T32).

import * as React from 'react';
import {
  BudgetBadge,
  Button,
  JobRow,
  ModelRegistryList,
  StatusBadge,
  tokens,
} from 'void-ui';
import type { JobCard } from 'void-studio/src/jobs/job';
import type { ModelRowProps } from 'void-ui';
import {
  useProjectSummary,
  useStudioCompactContext,
} from '../../useStudioData';
import {
  Card,
  Divider,
  Eyebrow,
  ReasonNote,
  Row,
  Stack,
} from '../../uip4/chrome';
import { JOB_OPS, REASONS } from '../../uip4/flags';
import { go, useFeatureStores, useJobs, useModelRows } from '../../uip4/runtime';

type Filter = 'all' | 'generations' | 'downloads' | 'completed';

const FILTERS: { id: Filter; label: string }[] = [
  { id: 'all', label: 'All jobs' },
  { id: 'generations', label: 'Generations' },
  { id: 'downloads', label: 'Model downloads' },
  { id: 'completed', label: 'Completed' },
];

function isDone(c: JobCard): boolean {
  return (
    c.status === 'succeeded' || c.status === 'failed' || c.status === 'cancelled'
  );
}

function matchFilter(c: JobCard, f: Filter): boolean {
  if (f === 'all') return true;
  if (f === 'completed') return isDone(c);
  if (f === 'generations')
    return c.kind === 'audio_generation' || c.kind === 'visual_generation' || c.kind === 'symbolic';
  if (f === 'downloads') return c.kind === 'analysis' || c.modelId !== null;
  return true;
}

export default function JobsScreen() {
  const compact = useStudioCompactContext();
  useFeatureStores();
  const summary = useProjectSummary();
  const cards = useJobs((s) => s.cards);
  const order = useJobs((s) => s.order);
  const loaded = useJobs((s) => s.loaded);
  const models = useModelRows();
  const [filter, setFilter] = React.useState<Filter>('all');

  const list = order
    .map((id) => cards[id])
    .filter((c): c is JobCard => !!c)
    .filter((c) => matchFilter(c, filter));
  const active = list.filter((c) => !isDone(c));
  const done = list.filter(isDone);

  const modelRows: ModelRowProps[] = models.rows.map((m) => ({
    modelId: m.modelId,
    version: m.version,
    name: m.name,
    kind: m.kind,
    runtime: m.runtime,
    status: m.status,
    cpuSeconds: m.cpuSeconds,
    memoryBytes: m.memoryBytes,
    wallNs: m.wallNs,
    cpuThreads: m.cpuThreads,
    artifactCount: m.artifactCount,
    license: m.license,
    source: m.source,
  }));

  const leftRail = (
    <Stack gap={10}>
      <Eyebrow>Jobs</Eyebrow>
      <Card>
        {FILTERS.map((f) => (
          <Button
            key={f.id}
            variant={filter === f.id ? 'secondary' : 'ghost'}
            size="sm"
            aria-pressed={filter === f.id}
            onClick={() => setFilter(f.id)}
          >
            {f.label}
          </Button>
        ))}
      </Card>
      <Card>
        <Eyebrow>Policy</Eyebrow>
        <Row gap={4} wrap>
          <StatusBadge status="processing" label="Audio priority" />
          <StatusBadge status="ready" label="Local workers" />
        </Row>
        <ReasonNote>
          Bounded memory, no automatic cloud fallback. Diagnostics are
          redacted — paths stay inside the project container.
        </ReasonNote>
        <Button
          variant="secondary"
          size="sm"
          disabled
          title={REASONS.jobPause}
        >
          Pause optional jobs
        </Button>
        <ReasonNote>{REASONS.jobPause}</ReasonNote>
      </Card>
      <Card>
        <Eyebrow>Updates</Eyebrow>
        <ReasonNote>
          {summary.recording?.isRecording
            ? 'A take is recording — update checks are deferred until it finishes.'
            : 'Update checks run outside takes; a take in progress defers them.'}
        </ReasonNote>
      </Card>
    </Stack>
  );

  const jobSections = (
    <Stack gap={10} style={{ flex: 1, minHeight: 0 }}>
      <Card>
        <Row justify="space-between">
          <Eyebrow>Active queue</Eyebrow>
          <ReasonNote>{active.length} running or queued</ReasonNote>
        </Row>
        {active.length === 0 ? (
          <ReasonNote>
            {loaded
              ? 'Nothing running — new jobs land here when the coordinator emits them.'
              : 'Waiting for the coordinator… job events appear here when they arrive.'}
          </ReasonNote>
        ) : (
          active.map((c) => (
            <JobRow
              key={c.jobId}
              jobId={c.jobId}
              name={c.modelId ?? c.kind}
              kind={c.kind}
              status={c.status}
              percent={c.percent}
              message={c.message ?? undefined}
              error={c.quarantined ? 'late result quarantined — never applied' : undefined}
              ramBytes={c.ramBytes}
              vramBytes={c.vramBytes}
              cpuThreads={c.cpuThreads}
              cpuSeconds={c.cpuSeconds}
              memoryBytes={c.memoryBytes}
              quarantined={c.quarantined}
              artifactCount={c.artifacts.length}
            />
          ))
        )}
        <ReasonNote>{REASONS.jobCancel}</ReasonNote>
      </Card>
      <Card>
        <Row justify="space-between">
          <Eyebrow>Recent</Eyebrow>
          <ReasonNote>{done.length} finished</ReasonNote>
        </Row>
        {done.length === 0 ? (
          <ReasonNote>No finished jobs yet.</ReasonNote>
        ) : (
          done.slice(0, 12).map((c) => (
            <JobRow
              key={c.jobId}
              jobId={c.jobId}
              name={c.modelId ?? c.kind}
              kind={c.kind}
              status={c.status}
              percent={c.percent}
              message={c.message ?? undefined}
              quarantined={c.quarantined}
              artifactCount={c.artifacts.length}
            />
          ))
        )}
      </Card>
    </Stack>
  );

  const rightRail = (
    <Stack gap={10}>
      <Eyebrow>Model downloads</Eyebrow>
      <Card>
        <ModelRegistryList
          models={modelRows}
          emptyLabel={
            models.loaded
              ? 'No models registered'
              : 'Registry not pushed yet — MODEL_LIST read view is not in this build'
          }
        />
        <Divider />
        <Button
          variant="secondary"
          size="sm"
          disabled
          title={REASONS.modelInstall}
        >
          Install model…
        </Button>
        <ReasonNote>
          {REASONS.modelInstall} Installs are always your choice — nothing
          downloads automatically.
        </ReasonNote>
      </Card>
      <Card>
        <Eyebrow>Budgets</Eyebrow>
        <Row gap={4} wrap>
          <BudgetBadge label="RAM" bytes={active[0]?.ramBytes ?? null} />
          <BudgetBadge label="VRAM" bytes={active[0]?.vramBytes ?? null} />
          <BudgetBadge
            label="CPU"
            threads={active[0]?.cpuThreads ?? null}
          />
          <BudgetBadge
            label="WALL"
            nanos={active[0]?.deadlineMonotonicNs ?? null}
          />
        </Row>
        <ReasonNote>
          Budgets are per-job reservations reported by the worker — shown
          verbatim from the job card.
        </ReasonNote>
      </Card>
      <Card>
        <Eyebrow>Generation results</Eyebrow>
        <ReasonNote>
          Accepted candidates become immutable project assets with full
          provenance; rejected alternates are never rewritten.
        </ReasonNote>
        <Button variant="ghost" size="sm" onClick={() => go('generate')}>
          Open Generate →
        </Button>
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
      {jobSections}
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
        {leftRail}
        {center}
        {rightRail}
      </div>
    );
  }

  return (
    <div style={{ display: 'flex', flex: 1, minWidth: 0, minHeight: 0 }}>
      <aside
        aria-label="Job filters"
        style={{
          width: 220,
          flexShrink: 0,
          borderRight: `1px solid ${tokens.line}`,
          overflowY: 'auto',
          padding: '12px 10px',
        }}
      >
        {leftRail}
      </aside>
      {center}
      <aside
        aria-label="Models and budgets"
        style={{
          width: 280,
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
