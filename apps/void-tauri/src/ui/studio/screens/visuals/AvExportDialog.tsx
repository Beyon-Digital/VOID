// S09 AV export dialog — the real void-av pipeline door (W24).
//
// The dialog builds an AvExportSpecDto exactly as the runner expects
// (packages/void-studio/src/av), from real state only: checkpoint_id +
// manifest sha come from the engine's SaveResultEvent telemetry after a
// real CreateCheckpointOp, inputs are sha-pinned asset blobs from
// ASSET_LIST, codec gates mirror `ffmpeg -encoders` probing (no probe
// channel in this build → every gate stays closed and says why). Launch
// goes through the submit_job seam — not in the shipped protocol, so the
// failure is reported verbatim instead of faking progress.

import * as React from 'react';
import { ActionButton, Field, OverlayShell, TextInput, tokens } from 'void-ui';
import { useStudio, type AvExportDraft, type AvInputDto } from 'void-studio';
import { getClient } from '../../../client';
import { assetBlobsByRole, JOB_SUBMIT_CHANNEL, REASON_NO_JOB_SUBMIT } from './model';
import { avPanelStore, useAssetListLoaded, useAvPanel } from './stores';

const label: React.CSSProperties = {
  fontFamily: tokens.mono,
  fontSize: 10,
  letterSpacing: '0.14em',
  textTransform: 'uppercase',
  color: tokens.textMuted,
};

const row: React.CSSProperties = {
  display: 'flex',
  gap: tokens.space8,
  alignItems: 'flex-end',
};

export const AvExportDialog: React.FC = () => {
  const open = useAvPanel((s) => s.open);
  const outputName = useAvPanel((s) => s.outputName);
  const codecId = useAvPanel((s) => s.codecId);
  const codecGates = useAvPanel((s) => s.codecGates);
  const width = useAvPanel((s) => s.width);
  const height = useAvPanel((s) => s.height);
  const rateNum = useAvPanel((s) => s.rateNum);
  const rateDen = useAvPanel((s) => s.rateDen);
  const errors = useAvPanel((s) => s.errors);
  const draft = useAvPanel((s) => s.draft);
  const jobs = useAvPanel((s) => s.jobs);
  const ffmpegPresent = useAvPanel((s) => s.ffmpegPresent);

  const projectId = useStudio((s) => s.projectId);
  const revision = useStudio((s) => s.revision);
  const attached = useStudio((s) => s.engine.attached);
  const clock = useStudio((s) => s.telemetry.clock);
  const save = useStudio((s) => s.telemetry.save);

  const { items } = useAssetListLoaded();
  const blobs = React.useMemo(() => assetBlobsByRole(items), [items]);

  const [videoSha, setVideoSha] = React.useState('');
  const [audioSha, setAudioSha] = React.useState('');
  const [rangeSamples, setRangeSamples] = React.useState('');
  const [checkpointBusy, setCheckpointBusy] = React.useState(false);
  const [launchNote, setLaunchNote] = React.useState<string | null>(null);
  // The job id is minted once per dialog open — a real uuid consumed only
  // by a real submit (or discarded on close). Never recycled per keystroke.
  const [jobId, setJobId] = React.useState('');
  React.useEffect(() => {
    if (open) setJobId(crypto.randomUUID());
  }, [open]);

  // Fill the draft whenever real dependencies change. checkpointId comes
  // only from a real SaveResultEvent — never minted locally.
  React.useEffect(() => {
    if (!open || !projectId || !jobId) return;
    const inputs: AvInputDto[] = [];
    if (videoSha) inputs.push({ kind: 'asset_blob', sha256: videoSha, role: 'video' });
    if (audioSha) inputs.push({ kind: 'asset_blob', sha256: audioSha, role: 'audio' });
    const d: AvExportDraft = {
      jobId,
      projectId,
      checkpointId: save?.status === 'SAVE_DURABLE' ? save.checkpoint_id : '',
      sourceRevision: revision,
      sampleRate: clock?.sample_rate ?? 0,
      rangeSamples,
      inputs,
    };
    avPanelStore.getState().actions.setDraft(d);
  }, [open, projectId, jobId, revision, save?.checkpoint_id, save?.status, clock?.sample_rate, videoSha, audioSha, rangeSamples]);

  if (!open) return null;

  const createCheckpoint = async () => {
    setCheckpointBusy(true);
    setLaunchNote(null);
    try {
      const receipt = await getClient().sendCommand({ CreateCheckpointOp: { reason: 'av export' } });
      if (receipt.status !== 'APPLIED' && receipt.status !== 'DUPLICATE') {
        setLaunchNote(`checkpoint failed — ${receipt.error}${receipt.message ? `: ${receipt.message}` : ''}`);
      }
      // Success surfaces through SaveResultEvent telemetry → draft above.
    } catch (e) {
      setLaunchNote(`checkpoint failed — ${String(e)}`);
    } finally {
      setCheckpointBusy(false);
    }
  };

  const launch = () => {
    setLaunchNote(null);
    const spec = avPanelStore.getState().actions.buildSpec();
    if (!spec) return; // store.errors carry the verbatim validator output
    if (!JOB_SUBMIT_CHANNEL) {
      setLaunchNote(
        `spec validated (job ${spec.jobId.slice(0, 8)}…) but launch failed — ${REASON_NO_JOB_SUBMIT}`,
      );
      return;
    }
  };

  const activeJobs = Object.values(jobs).filter((j) => j.status === 'queued' || j.status === 'running');

  return (
    <OverlayShell
      title="Export audio + visuals"
      onDismiss={() => avPanelStore.getState().actions.setOpen(false)}
      actions={
        <>
          <ActionButton variant="ghost" size="sm" onClick={() => avPanelStore.getState().actions.setOpen(false)}>
            Close
          </ActionButton>
          <ActionButton variant="primary" size="sm" onClick={launch}>
            Launch export
          </ActionButton>
        </>
      }
    >
      <div style={{ display: 'flex', flexDirection: 'column', gap: tokens.space12, minWidth: 440 }}>
        <span style={{ ...label, textTransform: 'none', letterSpacing: 0 }}>
          project {projectId ? `${projectId.slice(0, 8)}…` : '—'} · revision {revision || '—'} ·{' '}
          {clock ? `${clock.sample_rate} Hz` : 'no clock'}
        </span>

        <Field
          label="Checkpoint"
          hint={
            save?.status === 'SAVE_DURABLE' && save.checkpoint_id
              ? `checkpoint ${save.checkpoint_id.slice(0, 8)}… · manifest ${save.manifest_sha256.slice(0, 12)}…`
              : 'a verified checkpoint pins the render range — create one first'
          }
        >
          <div style={row}>
            <TextInput
              aria-label="Checkpoint id"
              value={save?.status === 'SAVE_DURABLE' ? save.checkpoint_id : ''}
              readOnly
              placeholder="no durable checkpoint yet"
            />
            <ActionButton
              variant="secondary"
              size="sm"
              loading={checkpointBusy}
              disabled={!attached}
              onClick={() => void createCheckpoint()}
            >
              Create checkpoint
            </ActionButton>
          </div>
        </Field>

        <Field label="Output name" hint="NAME_RE — letters, digits, . _ -">
          {(id) => (
            <TextInput
              id={id}
              value={outputName}
              onChange={(e) => avPanelStore.getState().actions.setField('outputName', e.target.value)}
            />
          )}
        </Field>

        <div style={{ display: 'flex', gap: tokens.space8 }}>
          <Field label="Width">
            {(id) => (
              <TextInput
                id={id}
                inputMode="numeric"
                value={String(width)}
                onChange={(e) => {
                  const n = Number(e.target.value);
                  if (Number.isFinite(n)) avPanelStore.getState().actions.setField('width', n);
                }}
              />
            )}
          </Field>
          <Field label="Height">
            {(id) => (
              <TextInput
                id={id}
                inputMode="numeric"
                value={String(height)}
                onChange={(e) => {
                  const n = Number(e.target.value);
                  if (Number.isFinite(n)) avPanelStore.getState().actions.setField('height', n);
                }}
              />
            )}
          </Field>
          <Field label="Frame rate (num/den)">
            <div style={row}>
              <TextInput
                aria-label="frame rate numerator"
                inputMode="numeric"
                value={rateNum}
                onChange={(e) => avPanelStore.getState().actions.setField('rateNum', e.target.value)}
                style={{ width: 72 }}
              />
              <span style={value}>/</span>
              <TextInput
                aria-label="frame rate denominator"
                inputMode="numeric"
                value={rateDen}
                onChange={(e) => avPanelStore.getState().actions.setField('rateDen', e.target.value)}
                style={{ width: 72 }}
              />
            </div>
          </Field>
        </div>

        <Field label="Range (samples)" hint="audio range in samples — u64 decimal">
          {(id) => (
            <TextInput
              id={id}
              inputMode="numeric"
              placeholder="e.g. 4800000"
              value={rangeSamples}
              onChange={(e) => setRangeSamples(e.target.value.trim())}
            />
          )}
        </Field>

        <Field
          label="Video input"
          hint={blobs.video.length === 0 ? 'no image/video assets in this project' : undefined}
        >
          {(id) => (
            <select
              id={id}
              aria-label="Video input"
              value={videoSha}
              onChange={(e) => setVideoSha(e.target.value)}
              style={selectStyle}
            >
              <option value="">— none selected —</option>
              {blobs.video.map((a) => (
                <option key={a.objectId} value={a.expectedSha256}>
                  {a.displayName} ({(a.mediaType ?? 'asset').toUpperCase()})
                </option>
              ))}
            </select>
          )}
        </Field>
        <Field
          label="Audio input"
          hint={blobs.audio.length === 0 ? 'no audio assets in this project' : undefined}
        >
          {(id) => (
            <select
              id={id}
              aria-label="Audio input"
              value={audioSha}
              onChange={(e) => setAudioSha(e.target.value)}
              style={selectStyle}
            >
              <option value="">— none selected —</option>
              {blobs.audio.map((a) => (
                <option key={a.objectId} value={a.expectedSha256}>
                  {a.displayName} ({(a.mediaType ?? 'asset').toUpperCase()})
                </option>
              ))}
            </select>
          )}
        </Field>

        <Field
          label="Codec"
          hint={
            ffmpegPresent
              ? 'encoders probed from the real ffmpeg build'
              : `codec gates closed — ${'no ffmpeg probe channel in this build'}`
          }
        >
          {(id) => (
            <select
              id={id}
              aria-label="Codec"
              value={codecId}
              onChange={(e) => avPanelStore.getState().actions.setField('codecId', e.target.value)}
              style={selectStyle}
            >
              {codecGates.map((g) => (
                <option key={g.codecId} value={g.codecId} disabled={!g.selectable}>
                  {g.codecId}
                  {g.selectable ? '' : ` — ${g.reason}`}
                </option>
              ))}
            </select>
          )}
        </Field>

        {errors.length > 0 ? (
          <div role="alert" style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
            {errors.map((e) => (
              <span key={e} style={{ fontFamily: tokens.mono, fontSize: 10, color: tokens.danger }}>
                {e}
              </span>
            ))}
          </div>
        ) : null}
        {launchNote ? (
          <p role="status" style={{ margin: 0, fontFamily: tokens.mono, fontSize: 10, color: tokens.orange }}>
            {launchNote}
          </p>
        ) : null}
        {activeJobs.map((j) => (
          <span key={j.jobId} style={{ fontFamily: tokens.mono, fontSize: 10, color: tokens.textMuted }}>
            job {j.jobId.slice(0, 8)}… {j.status}
            {j.percent != null ? ` ${j.percent}%` : ''}
            {j.message ? ` — ${j.message}` : ''}
          </span>
        ))}
        {draft ? (
          <span style={{ fontFamily: tokens.mono, fontSize: 9, color: tokens.textMuted }}>
            draft job {draft.jobId.slice(0, 8)}… · rev {draft.sourceRevision || '—'} · {draft.sampleRate || '—'} Hz · {draft.inputs.length} input(s)
          </span>
        ) : null}
      </div>
    </OverlayShell>
  );
};

const selectStyle: React.CSSProperties = {
  flex: 1,
  minWidth: 0,
  background: tokens.raised,
  border: `1px solid ${tokens.line}`,
  borderRadius: tokens.radius8,
  color: tokens.text,
  fontFamily: tokens.sans,
  fontSize: 13,
  padding: '6px 8px',
};

const value: React.CSSProperties = { fontFamily: tokens.mono, fontSize: 11, color: tokens.text };
