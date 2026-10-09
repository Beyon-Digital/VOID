// W11 support surfaces — launcher (templates + recents), notes, relink
// recovery, onboarding tour and help. All copy runs through the en
// catalog; all persistence is app-local via the injected KeyValueStore
// (localStorage) — authoritative ownership stays coordinator-side per
// CONTRACTS.md; the honest gaps are filed in docs/engine/NEEDS.md §rev2.

import React from 'react';
import {
  Button,
  MissingAssetRow,
  OverlayShell,
  Panel,
  RecentProjectRow,
  TemplateCard,
  tokens,
} from 'void-ui';
import {
  applyProjectTemplate,
  BUILTIN_TEMPLATES,
  createNoteDraftsStore,
  createOnboardingStore,
  createRecentsStore,
  createTranslator,
  en,
  loadViewPage,
  missingAssetRows,
  NOTE_OPS_AVAILABLE,
} from 'void-studio';
import {
  ONBOARDING_STEPS,
  parseNoteField,
  parseProjectInfo,
  projectNoteKey,
  studioStore,
  trackNoteKey,
  useStore,
  useStudio,
  useStudioActions,
  verifyCandidate,
  attachRelinkOp,
  webStore,
  makeViewKey,
} from 'void-studio';
import type { AssetRow, NoteDraftsStore, OnboardingStore, RecentsStore, TemplateApplyResult } from 'void-studio';
import { ensureClientStarted, getClient } from './client';

const t = createTranslator(en);

// App-local singleton stores (lazy so a non-Tauri preview never touches
// localStorage at import time).
let recentsStore: RecentsStore | null = null;
let noteDrafts: NoteDraftsStore | null = null;
let onboardingStore: OnboardingStore | null = null;

export function recents(): RecentsStore {
  recentsStore ??= createRecentsStore(webStore().store);
  return recentsStore;
}
export function drafts(): NoteDraftsStore {
  noteDrafts ??= createNoteDraftsStore(webStore().store);
  return noteDrafts;
}
export function onboarding(): OnboardingStore {
  onboardingStore ??= createOnboardingStore(webStore().store);
  return onboardingStore;
}

const row: React.CSSProperties = { display: 'flex', alignItems: 'center', gap: 8 };
const label: React.CSSProperties = { fontSize: 10, color: tokens.textMuted, fontFamily: tokens.mono };
const value: React.CSSProperties = { fontSize: 12, color: tokens.text, fontFamily: tokens.mono };
const inputStyle: React.CSSProperties = {
  background: tokens.bg,
  border: `1px solid ${tokens.border}`,
  borderRadius: tokens.radius,
  color: tokens.text,
  fontFamily: tokens.mono,
  fontSize: 12,
  padding: '6px 8px',
};
const okIcon = { ok: tokens.ok };

function slugify(name: string): string {
  return name.replace(/[^a-z0-9-]+/gi, '-').toLowerCase().replace(/^-+|-+$/g, '') || 'untitled';
}

// ---------------------------------------------------------------------------
// Launcher — template picker + open-by-path + recents

export function LauncherPanel() {
  const attached = useStudio((s) => s.engine.attached);
  const setProject = useStudioActions().setProject;
  const entries = useStore(recents(), (s) => s.entries);
  const persistError = useStore(recents(), (s) => s.persistError);
  const [name, setName] = React.useState('untitled');
  const [dir, setDir] = React.useState('');
  const [openDir, setOpenDir] = React.useState('');
  const [openPid, setOpenPid] = React.useState('');
  const [busy, setBusy] = React.useState<string | null>(null);
  const [notice, setNotice] = React.useState('');
  const [applyResult, setApplyResult] = React.useState<{ result: TemplateApplyResult; expected: number } | null>(null);
  const [error, setError] = React.useState('');

  const createFromTemplate = async (templateId: string) => {
    const tpl = BUILTIN_TEMPLATES.find((x) => x.id === templateId);
    if (!tpl) return;
    const projectId = crypto.randomUUID(); // project ids must be UUIDs (is_valid_id)
    const containerDir = dir.trim() || `./projects/${slugify(name)}.void`;
    setBusy(templateId);
    setError('');
    setApplyResult(null);
    try {
      const res = await applyProjectTemplate(
        getClient(),
        tpl,
        { projectId, name, containerDir },
        { t },
      );
      // Expected op count: create + optional time-signature + one op per
      // track spec + one insert per instrument spec.
      const expected =
        1 +
        (tpl.timeSignature && !(tpl.timeSignature.numerator === 4 && tpl.timeSignature.denominator === 4)
          ? 1
          : 0) +
        tpl.tracks.length +
        tpl.tracks.filter((s) => s.instrumentUid).length;
      setApplyResult({ result: res, expected });
      const last = res.steps[res.steps.length - 1];
      if (res.ok && last) {
        setProject(projectId, last.receipt.revision ?? '0');
        recents().getState().actions.recordOpen({
          containerDir,
          projectId,
          name,
          kind: 'created',
        });
        setNotice(t('launcher.created', { name, revision: last.receipt.revision ?? '?' }));
      } else if (res.failedStep) {
        setError(
          t('launcher.rejected', {
            op: res.failedStep.opName,
            error: res.failedStep.receipt.error,
            message: res.failedStep.receipt.message ? ` — ${res.failedStep.receipt.message}` : '',
          }),
        );
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const openProject = async (containerDir: string, projectId: string, displayName?: string) => {
    setBusy(`open:${containerDir}`);
    setError('');
    try {
      const c = getClient();
      if (projectId) c.setProject(projectId);
      const receipt = await c.sendCommand({ OpenProjectOp: { container_dir: containerDir } });
      if (receipt.status === 'APPLIED' || receipt.status === 'DUPLICATE') {
        setProject(projectId, receipt.revision ?? '0');
        recents().getState().actions.recordOpen({
          containerDir,
          projectId: projectId || containerDir,
          name: displayName ?? containerDir.split('/').filter(Boolean).pop() ?? containerDir,
          kind: 'opened',
        });
        setNotice(
          t('launcher.opened', {
            name: displayName ?? containerDir,
            revision: receipt.revision ?? '?',
          }),
        );
      } else {
        setError(
          t('launcher.rejected', {
            op: 'OpenProjectOp',
            error: receipt.error,
            message: receipt.message ? ` — ${receipt.message}` : '',
          }),
        );
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <Panel title={t('launcher.title')} style={{ flex: 1, margin: 12 }}>
      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: 16 }}>
        <section aria-label={t('launcher.newProject')}>
          <h3 style={{ ...label, margin: '0 0 8px', textTransform: 'uppercase', letterSpacing: '0.08em' }}>
            {t('launcher.newProject')}
          </h3>
          <div style={{ ...row, marginBottom: 8 }}>
            <input
              aria-label={t('launcher.projectName')}
              value={name}
              onChange={(e) => setName(e.target.value)}
              style={{ ...inputStyle, flex: 1 }}
              placeholder={t('launcher.projectName')}
            />
            <input
              aria-label={t('launcher.containerDir')}
              value={dir}
              onChange={(e) => setDir(e.target.value)}
              style={{ ...inputStyle, flex: 2 }}
              placeholder={t('launcher.containerDir')}
            />
          </div>
          <div role="list" style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            {BUILTIN_TEMPLATES.map((tpl) => (
              <TemplateCard
                key={tpl.id}
                name={t(tpl.nameKey)}
                description={t(tpl.descriptionKey)}
                specLine={`${tpl.tracks.length} tracks · ${tpl.initialBpm} BPM · ${tpl.sampleRate / 1000} kHz${
                  tpl.timeSignature ? ` · ${tpl.timeSignature.numerator}/${tpl.timeSignature.denominator}` : ''
                }`}
                actionLabel={t('launcher.create')}
                busy={busy === tpl.id}
                disabled={!attached || busy !== null}
                onUse={() => void createFromTemplate(tpl.id)}
              />
            ))}
          </div>
          {applyResult ? (
            <p
              role="status"
              style={{ ...label, marginTop: 8, color: applyResult.result.ok ? tokens.ok : tokens.warn }}
            >
              {t('templates.apply.summary', {
                applied: applyResult.result.steps.filter((s) => s.receipt.status === 'APPLIED' || s.receipt.status === 'DUPLICATE').length,
                total: applyResult.expected,
              })}
            </p>
          ) : null}
          <h3 style={{ ...label, margin: '16px 0 8px', textTransform: 'uppercase', letterSpacing: '0.08em' }}>
            {t('launcher.openByPath')}
          </h3>
          <div style={{ ...row, flexWrap: 'wrap' }}>
            <input
              aria-label={t('launcher.containerDir')}
              value={openDir}
              onChange={(e) => setOpenDir(e.target.value)}
              style={{ ...inputStyle, flex: 2, minWidth: 200 }}
              placeholder={t('launcher.containerDir')}
            />
            <input
              aria-label={t('launcher.openProjectId')}
              value={openPid}
              onChange={(e) => setOpenPid(e.target.value)}
              style={{ ...inputStyle, flex: 1, minWidth: 140 }}
              placeholder={t('launcher.openProjectId')}
            />
            <Button
              variant="primary"
              disabled={!attached || !openDir.trim() || busy !== null}
              onClick={() => void openProject(openDir.trim(), openPid.trim())}
            >
              {t('launcher.open')}
            </Button>
          </div>
        </section>
        <section aria-label={t('recents.title')}>
          <h3 style={{ ...label, margin: '0 0 8px', textTransform: 'uppercase', letterSpacing: '0.08em' }}>
            {t('recents.title')}
          </h3>
          {entries.length === 0 ? (
            <p style={label}>{t('recents.empty')}</p>
          ) : (
            <div role="list" style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
              {entries.map((e) => (
                <RecentProjectRow
                  key={e.containerDir}
                  name={e.name}
                  containerDir={e.containerDir}
                  kindLabel={e.kind === 'created' ? t('recents.kindCreated') : t('recents.kindOpened')}
                  lastOpenedLabel={t('recents.lastOpened', { at: e.lastOpenedAt })}
                  openLabel={t('recents.open')}
                  removeLabel={t('recents.remove')}
                  busy={busy === `open:${e.containerDir}`}
                  disabled={!attached || busy !== null}
                  onOpen={() => void openProject(e.containerDir, e.projectId, e.name)}
                  onRemove={() => recents().getState().actions.remove(e.containerDir)}
                />
              ))}
            </div>
          )}
          {persistError ? (
            <p role="alert" style={{ ...label, color: tokens.warn, marginTop: 8 }}>
              {t('recents.persistError', { error: persistError })}
            </p>
          ) : null}
        </section>
      </div>
      {notice ? (
        <p role="status" style={{ ...label, color: okIcon.ok, marginTop: 10 }}>{notice}</p>
      ) : null}
      {error ? (
        <p role="alert" style={{ color: tokens.danger, fontSize: 11, fontFamily: tokens.mono, marginTop: 10 }}>
          {error}
        </p>
      ) : null}
    </Panel>
  );
}

// ---------------------------------------------------------------------------
// Notes — document values from views + local unsent-intent drafts

export function NotesPanel() {
  const views = useStudio((s) => s.views);
  const projectId = useStudio((s) => s.projectId);
  const selection = useStudio((s) => s.selection);
  const attached = useStudio((s) => s.engine.attached);
  const draftsMap = useStore(drafts(), (s) => s.drafts);
  const [busy, setBusy] = React.useState(false);
  const [err, setErr] = React.useState('');
  const [projDraft, setProjDraft] = React.useState('');
  const [trackDraft, setTrackDraft] = React.useState('');

  const summaryEntry = views[makeViewKey('PROJECT_SUMMARY')];
  const info = React.useMemo(() => {
    if (!summaryEntry) return null;
    for (const it of summaryEntry.items) {
      const p = parseProjectInfo(it);
      if (p) return p;
    }
    return null;
  }, [summaryEntry]);

  const trackId = selection.trackId;
  const trackEntry = views[makeViewKey('TRACK_LIST')];
  const documentTrackNote = React.useMemo(() => {
    if (!trackEntry || !trackId) return undefined;
    for (const it of trackEntry.items) {
      try {
        const v = JSON.parse(it.summary_json) as Record<string, unknown>;
        const id = String(v.track_id ?? v.id ?? it.object_id);
        if (id === trackId) return parseNoteField(it);
      } catch {
        /* skip unparseable row */
      }
    }
    return undefined;
  }, [trackEntry, trackId]);

  const projKey = projectId ? projectNoteKey(projectId) : '';
  const trkKey = projectId && trackId ? trackNoteKey(projectId, trackId) : '';

  React.useEffect(() => {
    setProjDraft(projKey ? (draftsMap[projKey]?.text ?? '') : '');
  }, [projKey]); // eslint-disable-line react-hooks/exhaustive-deps
  React.useEffect(() => {
    setTrackDraft(trkKey ? (draftsMap[trkKey]?.text ?? '') : '');
  }, [trkKey]); // eslint-disable-line react-hooks/exhaustive-deps

  const refresh = async () => {
    setBusy(true);
    setErr('');
    try {
      await loadViewPage(studioStore, getClient(), 'PROJECT_SUMMARY');
      await loadViewPage(studioStore, getClient(), 'TRACK_LIST');
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  const infoFields: [string, string | undefined][] = [
    ['notes.field.name', info?.name],
    ['notes.field.sampleRate', info?.sampleRate !== undefined ? String(info.sampleRate) : undefined],
    ['notes.field.bpm', info?.bpm !== undefined ? String(info.bpm) : undefined],
    ['notes.field.key', info?.key],
    ['notes.field.timeSignature', info?.timeSignature],
    ['notes.field.trackCount', info?.trackCount !== undefined ? String(info.trackCount) : undefined],
    ['notes.field.assetCount', info?.assetCount !== undefined ? String(info.assetCount) : undefined],
    ['notes.field.storageBytes', info?.storageBytes !== undefined ? String(info.storageBytes) : undefined],
  ];

  return (
    <Panel
      title={t('notes.title')}
      actions={
        <Button loading={busy} disabled={!attached} onClick={refresh}>
          {t('relink.read').replace('ASSET_LIST', 'views')}
        </Button>
      }
      style={{ flex: 1 }}
    >
      {!NOTE_OPS_AVAILABLE ? (
        <p style={{ ...label, color: tokens.warn, marginTop: 0 }}>{t('notes.opsUnavailable')}</p>
      ) : null}
      <div style={{ display: 'grid', gridTemplateColumns: 'auto 1fr', gap: '4px 12px', marginBottom: 12 }}>
        {infoFields.map(([k, v]) => (
          <React.Fragment key={k}>
            <span style={label}>{t(k)}</span>
            <span style={value}>{v ?? '—'}</span>
          </React.Fragment>
        ))}
      </div>
      <h4 style={{ ...label, margin: '8px 0 4px' }}>{t('notes.projectNote')}</h4>
      <div style={{ ...row, alignItems: 'flex-start' }}>
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 4 }}>
          <span style={label}>{t('notes.documentValue')}</span>
          <span style={value}>{info?.note ?? t('notes.noDocumentValue')}</span>
          <span style={label}>{t('notes.draftLabel')}</span>
          <textarea
            aria-label={t('notes.projectNote')}
            value={projDraft}
            onChange={(e) => setProjDraft(e.target.value)}
            placeholder={t('notes.draftPlaceholder')}
            rows={2}
            style={{ ...inputStyle, resize: 'vertical', width: '100%', boxSizing: 'border-box' }}
            disabled={!projectId}
          />
          <div style={row}>
            <Button
              disabled={!projectId}
              onClick={() => drafts().getState().actions.setDraft(projKey, projDraft)}
            >
              {t('notes.draftLabel')}
            </Button>
            <Button
              variant="ghost"
              disabled={!projectId || !draftsMap[projKey]}
              onClick={() => {
                drafts().getState().actions.clearDraft(projKey);
                setProjDraft('');
              }}
            >
              {t('notes.clearDraft')}
            </Button>
          </div>
        </div>
      </div>
      <h4 style={{ ...label, margin: '12px 0 4px' }}>
        {trackId ? t('notes.trackNote', { track: trackId }) : t('notes.trackNote', { track: '—' })}
      </h4>
      {!trackId ? (
        <p style={label}>{t('notes.noTrackSelected')}</p>
      ) : (
        <div style={{ flex: 1, display: 'flex', flexDirection: 'column', gap: 4 }}>
          <span style={label}>{t('notes.documentValue')}</span>
          <span style={value}>{documentTrackNote ?? t('notes.noDocumentValue')}</span>
          <span style={label}>{t('notes.draftLabel')}</span>
          <textarea
            aria-label={t('notes.trackNote', { track: trackId })}
            value={trackDraft}
            onChange={(e) => setTrackDraft(e.target.value)}
            placeholder={t('notes.draftPlaceholder')}
            rows={2}
            style={{ ...inputStyle, resize: 'vertical', width: '100%', boxSizing: 'border-box' }}
          />
          <div style={row}>
            <Button onClick={() => drafts().getState().actions.setDraft(trkKey, trackDraft)}>
              {t('notes.draftLabel')}
            </Button>
            <Button
              variant="ghost"
              disabled={!draftsMap[trkKey]}
              onClick={() => {
                drafts().getState().actions.clearDraft(trkKey);
                setTrackDraft('');
              }}
            >
              {t('notes.clearDraft')}
            </Button>
          </div>
        </div>
      )}
      {err ? (
        <p role="alert" style={{ color: tokens.danger, fontSize: 11, fontFamily: tokens.mono }}>
          {err}
        </p>
      ) : null}
    </Panel>
  );
}

// ---------------------------------------------------------------------------
// Relink recovery — missing assets with verify-before-attach

function MissingRow({ row }: { row: AssetRow }) {
  const [busy, setBusy] = React.useState(false);
  const [status, setStatus] = React.useState('');
  const [verification, setVerification] = React.useState<'relink' | 'replace' | null>(null);
  const [verifiedSha, setVerifiedSha] = React.useState('');
  const [confirmReplace, setConfirmReplace] = React.useState(false);
  const fileRef = React.useRef<HTMLInputElement>(null);

  const onFile = async (f: File) => {
    setBusy(true);
    setStatus(t('relink.verifying'));
    setVerification(null);
    setConfirmReplace(false);
    try {
      const v = await verifyCandidate(row, await f.arrayBuffer());
      setVerifiedSha(v.sha256);
      setVerification(v.outcome);
      setStatus(
        v.outcome === 'relink'
          ? t('relink.match')
          : t('relink.mismatch', { actual: v.sha256.slice(0, 12), expected: (row.expectedSha256 ?? '').slice(0, 12) }),
      );
    } catch (e) {
      setStatus(String(e));
    } finally {
      setBusy(false);
    }
  };

  const attach = async () => {
    if (!verification || !verifiedSha) return;
    setBusy(true);
    try {
      const op = attachRelinkOp(row, {
        sha256: verifiedSha,
        outcome: verification,
        requiresExplicitReplace: verification === 'replace',
      });
      const receipt = await getClient().sendCommand(op);
      setStatus(
        receipt.status === 'APPLIED' || receipt.status === 'DUPLICATE'
          ? t('relink.attached', { status: receipt.status, revision: receipt.revision ?? '?' })
          : t('relink.attachFailed', {
              error: receipt.error,
              message: receipt.message ? ` — ${receipt.message}` : '',
            }),
      );
      if (receipt.status === 'APPLIED') {
        await loadViewPage(studioStore, getClient(), 'ASSET_LIST');
      }
    } catch (e) {
      setStatus(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div>
      <MissingAssetRow
        displayName={row.displayName}
        expectedSha256={row.expectedSha256 ?? ''}
        detail={row.detail}
        statusLabel={t('relink.expected')}
        relinkLabel={t('relink.chooseFile')}
        busy={busy}
        verification={verification}
        verifiedLabel={verification === 'relink' ? t('relink.match') : undefined}
        replaceWarnLabel={verification === 'replace' ? t('relink.confirmReplace') : undefined}
        onPickFile={() => fileRef.current?.click()}
      />
      <input
        ref={fileRef}
        type="file"
        aria-label={`${t('relink.chooseFile')} — ${row.displayName}`}
        style={{ display: 'none' }}
        onChange={(e) => {
          const f = e.target.files?.[0];
          if (f) void onFile(f);
          e.target.value = '';
        }}
      />
      {verification === 'replace' ? (
        <label style={{ ...label, display: 'flex', gap: 6, marginTop: 4, alignItems: 'center' }}>
          <input
            type="checkbox"
            checked={confirmReplace}
            onChange={(e) => setConfirmReplace(e.target.checked)}
          />
          {t('relink.confirmReplace')}
        </label>
      ) : null}
      {verification && (verification === 'relink' || confirmReplace) ? (
        <Button variant="primary" loading={busy} onClick={() => void attach()} style={{ marginTop: 6 }}>
          {t('relink.attach')}
        </Button>
      ) : null}
      {status ? (
        <p role="status" style={{ ...label, marginTop: 4 }}>{status}</p>
      ) : null}
    </div>
  );
}

export function RelinkPanel() {
  const views = useStudio((s) => s.views);
  const attached = useStudio((s) => s.engine.attached);
  const [busy, setBusy] = React.useState(false);
  const [err, setErr] = React.useState('');

  const entry = views[makeViewKey('ASSET_LIST')];
  const missing = React.useMemo(() => (entry ? missingAssetRows(entry.items) : []), [entry]);

  const load = async () => {
    setBusy(true);
    setErr('');
    try {
      await loadViewPage(studioStore, getClient(), 'ASSET_LIST');
    } catch (e) {
      setErr(String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <Panel
      title={t('relink.title')}
      actions={
        <Button loading={busy} disabled={!attached} onClick={load}>
          {t('relink.read')}
        </Button>
      }
      style={{ flex: 1 }}
    >
      <p style={{ ...label, marginTop: 0 }}>{t('relink.ingestGap')}</p>
      {!entry ? (
        <p style={label}>{t('relink.noData')}</p>
      ) : missing.length === 0 ? (
        <p style={{ ...label, color: tokens.ok }}>{t('relink.none')}</p>
      ) : (
        <div role="list" aria-label={t('relink.missingCount', { count: missing.length })} style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
          {missing.map((r) => (
            <MissingRow key={r.objectId} row={r} />
          ))}
        </div>
      )}
      {err ? (
        <p role="alert" style={{ color: tokens.danger, fontSize: 11, fontFamily: tokens.mono }}>
          {err}
        </p>
      ) : null}
    </Panel>
  );
}

// ---------------------------------------------------------------------------
// Onboarding tour + help overlay

export function SupportOverlays() {
  const ob = useStore(onboarding(), (s) => s);
  const actions = ob.actions;

  if (ob.tourOpen) {
    const step = ONBOARDING_STEPS[ob.step];
    const last = ob.step === ONBOARDING_STEPS.length - 1;
    return (
      <OverlayShell
        title={t('onboarding.title')}
        stepLabel={`${ob.step + 1} / ${ONBOARDING_STEPS.length}`}
        onDismiss={actions.finishTour}
        actions={
          <>
            <Button variant="ghost" onClick={actions.finishTour}>
              {t('onboarding.skip')}
            </Button>
            <Button disabled={ob.step === 0} onClick={actions.prevStep}>
              {t('onboarding.back')}
            </Button>
            {last ? (
              <Button variant="primary" onClick={actions.finishTour}>
                {t('onboarding.done')}
              </Button>
            ) : (
              <Button variant="primary" onClick={actions.nextStep}>
                {t('onboarding.next')}
              </Button>
            )}
          </>
        }
      >
        <h3 style={{ margin: '0 0 6px', fontSize: 14 }}>{t(step.titleKey)}</h3>
        <p style={{ margin: 0, color: tokens.textSecondary }}>{t(step.bodyKey)}</p>
      </OverlayShell>
    );
  }

  if (ob.helpOpen) {
    const sections = ['project', 'views', 'transport', 'notes', 'relink'] as const;
    return (
      <OverlayShell
        title={t('help.title')}
        onDismiss={actions.closeHelp}
        actions={
          <>
            <Button
              onClick={() => {
                actions.closeHelp();
                actions.openTour();
              }}
            >
              {t('help.reopenTour')}
            </Button>
            <Button variant="primary" onClick={actions.closeHelp}>
              {t('help.close')}
            </Button>
          </>
        }
      >
        {sections.map((s) => (
          <section key={s} style={{ marginBottom: 12 }}>
            <h3 style={{ margin: '0 0 4px', fontSize: 12, fontFamily: tokens.mono, color: tokens.accent }}>
              {t(`help.section.${s}.title`)}
            </h3>
            <p style={{ margin: 0, color: tokens.textSecondary }}>{t(`help.section.${s}.body`)}</p>
          </section>
        ))}
      </OverlayShell>
    );
  }

  return null;
}

export function HelpButton() {
  const actions = useStore(onboarding(), (s) => s.actions);
  return (
    <Button onClick={actions.openHelp} aria-label={t('help.title')} style={{ fontSize: 10 }}>
      {t('help.title')}
    </Button>
  );
}

/** Re-check Tauri availability + read the views the support panels use. */
export function useSupportStartup(): void {
  React.useEffect(() => {
    void ensureClientStarted();
  }, []);
}
