// S12 — Export / completed (figma 4:1108).
//
// Data contract:
//   results   ← exportView.results — real ExportResult cards fed by the
//               EXPORT_LIST read view when the engine reports items
//   jobs      ← exportView.jobs — real JobEvent telemetry (no JobEvent
//               kind exists in the schema yet, so a job only appears if
//               the engine actually emits one)
//   last spec ← the validated spec from S11 (display context only)
//   gaps      ← EXPORT_LIST is not a ViewKindName in the schema — the
//               request is issued with the honest 'EXPORT_LIST' view id
//               and a rejection surfaces as text, never as fake files.

import * as React from 'react';
import { ActionButton, StatusBadge, tokens } from 'void-ui';
import {
  exportResultsFromItems,
  isTerminal,
  useStore,
  useStudio,
  type ExportResultItem,
} from 'void-studio';
import { getClient } from '../../../client';
import { useStudioCompactContext } from '../../useStudioData';
import { exportView, lastSubmit } from '../export/exportStore';

export default function ExportDoneScreen() {
  const compact = useStudioCompactContext();
  const projectId = useStudio((s) => s.projectId);
  const attached = useStudio((s) => s.engine.attached);
  const results = useStore(exportView, (s) => s.results);
  const jobs = useStore(exportView, (s) => s.jobs);
  const [listNote, setListNote] = React.useState<string | null>(null);

  // Ask the engine for the export list. The view id is honest — when the
  // schema lacks it the coordinator replies UNSUPPORTED and we show that.
  React.useEffect(() => {
    if (!projectId || !attached) return;
    let cancelled = false;
    getClient()
      .readView({ view: 'EXPORT_LIST' as never, projectId })
      .then((res) => {
        if (cancelled) return;
        const items = res.items ?? [];
        const parsed = exportResultsFromItems(items);
        if (parsed.length > 0) exportView.getState().actions.applyResults(items);
        else setListNote('Export list view returned no result records.');
      })
      .catch((e: unknown) => {
        if (cancelled) return;
        setListNote(
          `Export results unavailable: ${e instanceof Error ? e.message : String(e)}`,
        );
      });
    return () => {
      cancelled = true;
    };
  }, [projectId, attached]);

  const finished = React.useMemo(
    () =>
      Object.values(jobs).filter((j) => isTerminal(j.status)),
    [jobs],
  );

  const latest: ExportResultItem | undefined = results[results.length - 1];
  const latestFailed = finished.find((j) => j.status === 'failed');
  const spec = lastSubmit.spec;

  const fileName = (r: ExportResultItem) => {
    const f = r.file ?? '';
    const base = f.split(/[\\/]/).pop();
    return base || '(unnamed file)';
  };

  return (
    <div style={{ height: '100%', overflowY: 'auto', background: tokens.ink, fontFamily: tokens.sans, color: tokens.text }}>
      <div style={{ maxWidth: 640, margin: '0 auto', padding: tokens.space24, display: 'flex', flexDirection: 'column', gap: tokens.space12 }}>
        <StatusBadge status={latestFailed ? 'error' : latest ? 'saved' : 'unavailable'} />
        <div style={{ fontSize: 18, fontWeight: 700 }}>
          {latest ? 'Ready for the next set of ears.' : latestFailed ? 'Export failed.' : 'No completed export on record.'}
        </div>
        <div style={{ fontSize: 12, color: tokens.muted }}>
          {latest
            ? 'Your files were written successfully. The editable project remains on this device.'
            : latestFailed
              ? 'The render pipeline reported a failure — nothing was published as verified output.'
              : 'When an export finishes, its verified output details appear here.'}
        </div>

        {latest ? (
          <div style={{ border: `1px solid ${tokens.line}`, borderRadius: tokens.radius12, background: tokens.surface, padding: tokens.space12, display: 'flex', gap: tokens.space12, alignItems: 'center' }}>
            <div
              aria-hidden
              style={{
                width: 64, height: 44, borderRadius: tokens.radius8,
                background: `repeating-linear-gradient(90deg, ${tokens.accentSoft} 0 3px, transparent 3px 6px)`,
                border: `1px solid ${tokens.line}`,
              }}
            />
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontSize: 13, fontWeight: 600, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }} title={latest.file}>
                {fileName(latest)}
              </div>
              <div style={{ fontSize: 11, color: tokens.muted, marginTop: 2, fontFamily: tokens.fontNumeric }}>
                {[
                  spec ? `${spec.format.toUpperCase()}` : null,
                  spec?.format === 'wav'
                    ? `${spec.channels ?? ''} · ${spec.bitDepth ?? ''} · ${spec.sampleRate ?? ''} Hz`
                    : null,
                  latest.frames ? `${latest.frames} frames` : null,
                  latest.bytes ? `${latest.bytes} bytes` : null,
                ].filter(Boolean).join(' · ') || 'details reported by the runner'}
              </div>
              {latest.sha256 ? (
                <div style={{ fontSize: 10, color: tokens.subtle, fontFamily: tokens.fontNumeric, marginTop: 2 }}>
                  sha256 {latest.sha256.slice(0, 16)}…
                </div>
              ) : null}
            </div>
            <StatusBadge status={latest.status === 'succeeded' ? 'saved' : 'error'} />
          </div>
        ) : null}

        {latestFailed ? (
          <div role="alert" style={{ border: `1px solid ${tokens.danger}`, borderRadius: tokens.radius8, padding: tokens.space12, background: tokens.dangerSoft }}>
            <div style={{ fontSize: 12, fontWeight: 600, color: tokens.danger }}>Job {latestFailed.jobId.slice(0, 8)} failed</div>
            <div style={{ fontSize: 11, color: tokens.danger, marginTop: 4 }}>
              {latestFailed.message ?? 'the runner reported failure without a message'}
            </div>
          </div>
        ) : null}

        {listNote ? (
          <div style={{ fontSize: 11, color: tokens.subtle, border: `1px dashed ${tokens.line}`, borderRadius: tokens.radius8, padding: tokens.space8 }}>
            {listNote}
          </div>
        ) : null}

        <div style={{ display: 'flex', gap: tokens.space8, flexDirection: compact ? 'column' : 'row' }}>
          <ActionButton variant="primary" onClick={() => { window.location.hash = '#/arrange'; }}>
            Back to arrange
          </ActionButton>
          <ActionButton variant="secondary" onClick={() => { window.location.hash = '#/export'; }}>
            Export again
          </ActionButton>
        </div>
        <div style={{ fontSize: 10, color: tokens.subtle }}>
          Reveal-in-folder and in-app audition need a filesystem/playback surface this shell does not expose yet.
        </div>
      </div>
    </div>
  );
}
