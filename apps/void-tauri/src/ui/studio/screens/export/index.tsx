// S11 — Export / delivery check (figma 4:1008).
//
// Data contract:
//   fields      ← exportView dialog store (selections) — the spec model is
//                 the source of truth for every option presented
//   preflight   ← runPreflight (void-studio export/preflight) over REAL
//                 TRACK_LIST / ASSET_LIST / CLIP_LIST / PLUGIN_LIST items +
//                 telemetry.save checkpoint — UI-T29, no hardcoded greens
//   submit      ← store.submit(ctx) runs spec construction + validation
//                 against a REAL ExportContext (checkpoint id from the
//                 engine's SaveResultEvent, revision from the studio
//                 store, asset hashes from ASSET_LIST). There is no job /
//                 export invoke on this shell yet — a valid spec reports
//                 honest unavailability with the reason instead of
//                 pretending a render started.
//   normalize / dither — NOT in the export spec: rendered as disabled
//                 rows that say so (never a silent or double application).

import * as React from 'react';
import { ActionButton, Field, StatusBadge, TextInput, tokens } from 'void-ui';
import {
  APPROVED_SAMPLE_RATES,
  devicesFromItems,
  makeViewKey,
  runPreflight,
  stripsFromTrackItems,
  useStore,
  useStudio,
  loadViewPage,
  studioStore,
  type ExportBitDepth,
  type ExportChannels,
  type ExportContext,
  type ExportFormat,
  type PreflightCheck,
} from 'void-studio';
import type { ReadItem } from 'void-client';
import { getClient } from '../../../client';
import { useStudioCompactContext } from '../../useStudioData';
import { exportView, lastSubmit } from './exportStore';

const FORMATS: { id: ExportFormat; label: string }[] = [
  { id: 'wav', label: 'WAV' },
  { id: 'midi', label: 'MIDI' },
];
const CHANNELS: ExportChannels[] = ['stereo', 'mono'];
const DEPTHS: { id: ExportBitDepth; label: string }[] = [
  { id: 'pcm16', label: '16-bit PCM' },
  { id: 'pcm24', label: '24-bit PCM' },
  { id: 'float32', label: '32-bit float' },
];

function assetRefFromClip(objectId: string, summaryJson: string): string | null {
  try {
    const s = JSON.parse(summaryJson) as Record<string, unknown>;
    const v =
      (typeof s.asset_id === 'string' && s.asset_id) ||
      (typeof s.asset === 'string' && s.asset) ||
      (typeof s.source_asset_id === 'string' && s.source_asset_id);
    return v || null;
  } catch {
    return null;
  }
}

function assetFromItem(it: ReadItem): { id: string; sha?: string } | null {
  try {
    const s = JSON.parse(it.summary_json) as Record<string, unknown>;
    const id =
      (typeof s.asset_id === 'string' && s.asset_id) || (typeof s.id === 'string' && s.id) || it.object_id;
    const sha = typeof s.sha256 === 'string' && s.sha256 ? s.sha256 : undefined;
    return { id, sha };
  } catch {
    return { id: it.object_id };
  }
}

function checkIcon(c: PreflightCheck): string {
  switch (c.status) {
    case 'pass': return '✓';
    case 'fail': return '✕';
    case 'warn': return '⚠';
    default: return '?';
  }
}

function checkColor(c: PreflightCheck): string {
  switch (c.status) {
    case 'pass': return tokens.mint;
    case 'fail': return tokens.danger;
    case 'warn': return tokens.orange;
    default: return tokens.subtle;
  }
}

export default function ExportScreen() {
  const compact = useStudioCompactContext();
  const projectId = useStudio((s) => s.projectId);
  const revision = useStudio((s) => s.revision);
  const attached = useStudio((s) => s.engine.attached);
  const save = useStudio((s) => s.telemetry.save);
  const trackEntry = useStudio((s) => s.views[makeViewKey('TRACK_LIST')]);
  const assetEntry = useStudio((s) => s.views[makeViewKey('ASSET_LIST')]);
  const clipEntry = useStudio((s) => s.views[makeViewKey('CLIP_LIST')]);
  const pluginEntry = useStudio((s) => s.views[makeViewKey('PLUGIN_LIST')]);
  const summaryEntry = useStudio((s) => s.views[makeViewKey('PROJECT_SUMMARY')]);

  const outputName = useStore(exportView, (s) => s.outputName);
  const format = useStore(exportView, (s) => s.format);
  const rangeStartTicks = useStore(exportView, (s) => s.rangeStartTicks);
  const rangeEndTicks = useStore(exportView, (s) => s.rangeEndTicks);
  const channels = useStore(exportView, (s) => s.channels);
  const bitDepth = useStore(exportView, (s) => s.bitDepth);
  const sampleRate = useStore(exportView, (s) => s.sampleRate);
  const tail = useStore(exportView, (s) => s.tail);
  const errors = useStore(exportView, (s) => s.errors);
  const sel = { outputName, format, rangeStartTicks, rangeEndTicks, channels, bitDepth, sampleRate, tail };

  const [submitNote, setSubmitNote] = React.useState<string | null>(null);

  const trackItems: ReadItem[] = trackEntry?.items ?? [];
  const assetItems: ReadItem[] = assetEntry?.items ?? [];
  const clipItems: ReadItem[] = clipEntry?.items ?? [];
  const pluginItems: ReadItem[] = pluginEntry?.items ?? [];

  React.useEffect(() => {
    if (!projectId || !attached) return;
    const c = getClient();
    for (const view of ['TRACK_LIST', 'ASSET_LIST', 'CLIP_LIST', 'PLUGIN_LIST', 'PROJECT_SUMMARY'] as const) {
      void loadViewPage(studioStore, c, view).catch(() => undefined);
    }
  }, [projectId, attached, revision]);

  const bpm = React.useMemo(() => {
    const item = summaryEntry?.items?.[0];
    if (!item) return null;
    try {
      const v = JSON.parse(item.summary_json) as { bpm?: unknown };
      return typeof v.bpm === 'number' ? v.bpm : null;
    } catch {
      return null;
    }
  }, [summaryEntry]);

  const assets = React.useMemo(
    () => assetItems.map(assetFromItem).filter((a): a is { id: string; sha?: string } => a !== null),
    [assetItems],
  );
  const clipRefs = React.useMemo(
    () =>
      clipItems
        .map((it) => assetRefFromClip(it.object_id, it.summary_json))
        .filter((x): x is string => x !== null),
    [clipItems],
  );
  const failedDevices = React.useMemo(
    () =>
      devicesFromItems(pluginItems)
        .filter((d) => d.failed || d.powered === false)
        .map((d) => ({
          instanceId: d.instanceId,
          name: d.name,
          reason: d.failed ?? 'disabled',
        })),
    [pluginItems],
  );
  const checkpointId = save?.checkpoint_id;
  const rangeNonEmpty = React.useMemo(() => {
    try {
      return BigInt(sel.rangeEndTicks) > BigInt(sel.rangeStartTicks);
    } catch {
      return false;
    }
  }, [sel.rangeStartTicks, sel.rangeEndTicks]);

  const report = React.useMemo(
    () =>
      runPreflight({
        engineAttached: attached,
        trackCount: stripsFromTrackItems(trackItems).length,
        busCount: stripsFromTrackItems(trackItems).filter((s) => s.kind === 'BUS').length,
        assetIds: assets.map((a) => a.id),
        clipAssetRefs: clipRefs,
        failedDevices,
        checkpointId,
        rangeNonEmpty,
      }),
    [attached, trackItems, assets, clipRefs, failedDevices, checkpointId, rangeNonEmpty],
  );

  const submit = () => {
    setSubmitNote(null);
    if (!projectId || !attached) {
      setSubmitNote('Export unavailable: engine is detached.');
      lastSubmit.status = 'rejected';
      lastSubmit.message = 'engine detached';
      return;
    }
    if (!report.ready) {
      setSubmitNote(`Preflight blocks export: ${report.blockers.map((b) => b.label).join('; ')}.`);
      return;
    }
    const ctx: ExportContext = {
      jobId: crypto.randomUUID(),
      projectId,
      checkpointId: checkpointId ?? '',
      sourceRevision: String(revision),
      assetHashes: assets.map((a) => a.sha).filter((x): x is string => Boolean(x)),
      tempoMap: bpm !== null ? [{ atTicks: '0', bpm }] : [],
    };
    const spec = exportView.getState().actions.submit(ctx);
    if (!spec) {
      lastSubmit.status = 'rejected';
      lastSubmit.message = 'spec validation failed';
      return;
    }
    // The spec validated. This shell exposes no export/job invoke — the
    // honest report is the capability gap, not a fabricated progress bar.
    lastSubmit.spec = spec;
    lastSubmit.status = 'unavailable';
    lastSubmit.message =
      'spec validated, but this shell has no export-runner invoke (void-export is not wired to send_command) — submission held';
    setSubmitNote(
      `Spec for "${spec.outputName}" validated against checkpoint ${ctx.checkpointId}. Submission is unavailable: the export runner has no invoke surface on this shell yet.`,
    );
  };

  const setField = exportView.getState().actions.set;
  const selStyle: React.CSSProperties = {
    flex: 1, background: tokens.raised, border: `1px solid ${tokens.line}`,
    borderRadius: tokens.radius8, color: tokens.text, fontFamily: tokens.sans,
    fontSize: 13, padding: '6px 8px', outline: 'none',
  };
  const wav = sel.format === 'wav';

  const settings = (
    <section style={{ flex: 1, minWidth: 300, display: 'flex', flexDirection: 'column', gap: tokens.space12, overflowY: 'auto' }}>
      <div>
        <div style={{ fontSize: 16, fontWeight: 700 }}>Export settings</div>
        <div style={{ fontSize: 12, color: tokens.muted, marginTop: 2 }}>Finish the way you intended.</div>
      </div>
      <div style={{ display: 'flex', gap: tokens.space8 }} role="group" aria-label="Export type">
        {FORMATS.map((f) => (
          <ActionButton
            key={f.id}
            variant={sel.format === f.id ? 'primary' : 'secondary'}
            onClick={() => setField('format', f.id)}
            aria-pressed={sel.format === f.id}
          >
            {f.label}
          </ActionButton>
        ))}
      </div>

      <div>
        <span style={{ fontSize: 10, fontWeight: 700, letterSpacing: 1, color: tokens.subtle }}>AUDIO SETTINGS</span>
        <div style={{ display: 'flex', flexDirection: 'column', gap: tokens.space8, marginTop: tokens.space8, opacity: wav ? 1 : 0.5 }}>
          <Field label="Sample rate" unit="Hz" hint={wav ? 'approved rates only' : 'audio fields are WAV-only'}>
            {(id) => (
              <select id={id} disabled={!wav} value={sel.sampleRate} onChange={(e) => setField('sampleRate', Number(e.target.value))} style={selStyle}>
                {APPROVED_SAMPLE_RATES.map((r) => <option key={r} value={r}>{r}</option>)}
              </select>
            )}
          </Field>
          <Field label="Bit depth">
            {(id) => (
              <select id={id} disabled={!wav} value={sel.bitDepth} onChange={(e) => setField('bitDepth', e.target.value as ExportBitDepth)} style={selStyle}>
                {DEPTHS.map((d) => <option key={d.id} value={d.id}>{d.label}</option>)}
              </select>
            )}
          </Field>
          <Field label="Channels">
            {(id) => (
              <select id={id} disabled={!wav} value={sel.channels} onChange={(e) => setField('channels', e.target.value as ExportChannels)} style={selStyle}>
                {CHANNELS.map((c) => <option key={c} value={c}>{c}</option>)}
              </select>
            )}
          </Field>
        </div>
        <div style={{ display: 'flex', gap: tokens.space8, marginTop: tokens.space8 }}>
          <Field label="Range start" unit="ticks" style={{ flex: 1 }}>
            {(id) => (
              <TextInput id={id} value={sel.rangeStartTicks} onChange={(e) => setField('rangeStartTicks', e.target.value)} inputMode="numeric" aria-label="Range start ticks" />
            )}
          </Field>
          <Field label="Range end" unit="ticks" style={{ flex: 1 }} error={rangeNonEmpty ? undefined : 'end must be after start'}>
            {(id) => (
              <TextInput id={id} value={sel.rangeEndTicks} onChange={(e) => setField('rangeEndTicks', e.target.value)} inputMode="numeric" invalid={!rangeNonEmpty} aria-label="Range end ticks" />
            )}
          </Field>
        </div>
      </div>

      <div>
        <span style={{ fontSize: 10, fontWeight: 700, letterSpacing: 1, color: tokens.subtle }}>OUTPUT SETTINGS</span>
        <div style={{ display: 'flex', flexDirection: 'column', gap: tokens.space8, marginTop: tokens.space8 }}>
          <Field label="File name" hint="letters, digits, space, dash, underscore — up to 120 chars">
            {(id) => (
              <TextInput id={id} value={sel.outputName} onChange={(e) => setField('outputName', e.target.value)} aria-label="Output file name" />
            )}
          </Field>
          <Field label="Destination" hint="Exports publish inside the project container — no filesystem picker exists on this shell.">
            {(id) => <TextInput id={id} value="" readOnly disabled aria-label="Destination" />}
          </Field>
          <Field label="Tail handling" hint="Render effect tails past the range end.">
            {(id) => (
              <select
                id={id}
                value={sel.tail.mode === 'milliseconds' ? 'ms' : 'none'}
                onChange={(e) => {
                  if (e.target.value === 'ms') exportView.getState().actions.setTailMs(2000);
                  else exportView.getState().actions.setTailNone();
                }}
                style={selStyle}
              >
                <option value="none">None — cut at range end</option>
                <option value="ms">Milliseconds of tail</option>
              </select>
            )}
          </Field>
          {sel.tail.mode === 'milliseconds' ? (
            <Field label="Tail length" unit="ms">
              {(id) => (
                <TextInput
                  id={id}
                  value={String(sel.tail.mode === 'milliseconds' ? sel.tail.ms : '')}
                  onChange={(e) => {
                    const n = Number(e.target.value);
                    if (Number.isFinite(n)) exportView.getState().actions.setTailMs(n);
                  }}
                  inputMode="numeric"
                  aria-label="Tail length in milliseconds"
                />
              )}
            </Field>
          ) : null}
        </div>
      </div>

      <div>
        <span style={{ fontSize: 10, fontWeight: 700, letterSpacing: 1, color: tokens.subtle }}>OPTIONS</span>
        <div style={{ display: 'flex', flexDirection: 'column', gap: 4, marginTop: tokens.space8, fontSize: 12 }}>
          <label style={{ display: 'flex', gap: tokens.space8, alignItems: 'center' }}>
            <input
              type="checkbox"
              checked={sel.tail.mode !== 'none'}
              onChange={(e) => (e.target.checked ? exportView.getState().actions.setTailMs(2000) : exportView.getState().actions.setTailNone())}
              aria-label="Include effect tails"
            />
            Include effect tails
          </label>
          <label style={{ display: 'flex', gap: tokens.space8, alignItems: 'center', opacity: 0.55 }}>
            <input type="checkbox" disabled aria-label="Normalize loudness — unavailable" />
            Normalize loudness — not in the export spec; never applied
          </label>
          <label style={{ display: 'flex', gap: tokens.space8, alignItems: 'center', opacity: 0.55 }}>
            <input type="checkbox" disabled aria-label="Dither — unavailable" />
            Dither — not in the export spec; never applied
          </label>
          <span style={{ fontSize: 10, color: tokens.subtle }}>
            Normalization stays off unless you choose a delivery target. Do not double-dither.
          </span>
        </div>
      </div>

      {errors.length > 0 ? (
        <div role="alert" style={{ border: `1px solid ${tokens.danger}`, borderRadius: tokens.radius8, padding: tokens.space8, display: 'flex', flexDirection: 'column', gap: 4 }}>
          <span style={{ fontSize: 11, fontWeight: 600, color: tokens.danger }}>Spec validation</span>
          {errors.map((e, i) => (
            <span key={i} style={{ fontSize: 11, color: tokens.danger }}>{e}</span>
          ))}
        </div>
      ) : null}
      {submitNote ? (
        <div role="status" style={{ border: `1px solid ${tokens.line}`, borderRadius: tokens.radius8, padding: tokens.space8, fontSize: 11, color: tokens.muted }}>
          {submitNote}
        </div>
      ) : null}

      <div style={{ display: 'flex', gap: tokens.space8, marginTop: 'auto' }}>
        <ActionButton variant="ghost" onClick={() => { window.location.hash = '#/arrange'; }}>Back</ActionButton>
        <ActionButton
          variant="primary"
          disabled={!attached || !projectId || !report.ready}
          onClick={submit}
          aria-label="Start export"
        >
          Export
        </ActionButton>
      </div>
    </section>
  );

  const preflight = (
    <section style={{ width: compact ? '100%' : 320, flex: '0 0 auto', display: 'flex', flexDirection: 'column', gap: tokens.space8 }}>
      <div>
        <div style={{ fontSize: 14, fontWeight: 700 }}>Export preflight</div>
        <div style={{ fontSize: 12, color: tokens.muted, marginTop: 2 }}>Before the bounce.</div>
      </div>
      <div style={{ display: 'flex', flexDirection: 'column', gap: tokens.space8 }} role="list" aria-label="Preflight checks">
        {report.checks.map((c) => (
          <div key={c.id} role="listitem" style={{ border: `1px solid ${tokens.line}`, borderRadius: tokens.radius8, padding: tokens.space8, background: tokens.surface }}>
            <div style={{ display: 'flex', gap: tokens.space8, alignItems: 'center', fontSize: 12 }}>
              <span aria-hidden style={{ color: checkColor(c), fontFamily: tokens.fontNumeric }}>{checkIcon(c)}</span>
              <span style={{ flex: 1 }}>{c.label}</span>
              <StatusBadge
                status={c.status === 'pass' ? 'ready' : c.status === 'fail' ? 'error' : c.status === 'warn' ? 'queued' : 'unavailable'}
              />
            </div>
            <div style={{ fontSize: 10, color: tokens.subtle, marginTop: 4, marginLeft: 22 }}>{c.detail}</div>
          </div>
        ))}
      </div>
      <div style={{ fontSize: 10, color: tokens.subtle }}>
        Render uses the selected revision. Later edits do not silently alter an in-progress export.
      </div>
    </section>
  );

  return (
    <div style={{ height: '100%', overflowY: 'auto', background: tokens.ink, fontFamily: tokens.sans, color: tokens.text }}>
      <div style={{ display: 'flex', flexDirection: compact ? 'column' : 'row', gap: tokens.space16, padding: tokens.space16, maxWidth: 1060, margin: '0 auto' }}>
        {settings}
        {preflight}
      </div>
    </div>
  );
}
