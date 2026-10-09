// Save recovery screen — S21 (UI-T27).
//
// "Saved" means a durable checkpoint was published — this screen only
// ever reports what the wire said:
//   - the failed write = the latest SaveResultEvent with SAVE_FAILED
//     (error + engine message + the revision it was at);
//   - the last-good checkpoint = the latest SAVE_DURABLE event retained
//     by the recovery store (a later failure never overwrites it);
//   - unsaved edits = current project revision minus the failed save's
//     revision — a derived count, never a guess;
//   - unsaved edits survive the failed write: the in-memory song is
//     untouched, the screen says so;
//   - retry = real saveProject(); checkpoint = real CreateCheckpointOp;
//   - "choose another location" is honest guidance — there is no
//     save-destination op on the wire, so no picker is faked.

import * as React from 'react';
import { ActionButton, StatusBadge, tokens } from 'void-ui';
import {
  describeReceiptError,
  receiptFailed,
  sendWithStaleRetry,
  useStudio,
} from 'void-studio';
import { getClient } from '../../../client';
import { ensureRecoveryBound, useRecovery } from '../shared/recovery';
import {
  RecoveryBody,
  RecoveryCopy,
  RecoveryHeadline,
  RecoveryMain,
  RecoveryNote,
  RecoveryRail,
  RecoveryStep,
  StateRow,
} from '../shared/chrome';

export default function SaveRecoveryScreen() {
  const projectId = useStudio((s) => s.projectId);
  const revision = useStudio((s) => s.revision);
  const engineAttached = useStudio((s) => s.engine.attached);
  const latestSave = useStudio((s) => s.telemetry.save);
  const lastDurable = useRecovery((s) => s.lastDurableSave);
  const lastFailed = useRecovery((s) => s.lastFailedSave);

  const [busy, setBusy] = React.useState<'save' | 'checkpoint' | 'save-as' | null>(null);
  const [error, setError] = React.useState('');
  const [note, setNote] = React.useState('');
  const [saveAsOpen, setSaveAsOpen] = React.useState(false);
  const [saveAsDir, setSaveAsDir] = React.useState('');
  const [saveAsName, setSaveAsName] = React.useState('');

  /** rev-2 (NEEDS §30): SaveProjectAsOp checkpoints the song into a NEW
   * container dir — the destination the failed write couldn't reach. */
  const saveAs = async () => {
    const dir = saveAsDir.trim();
    if (!dir) {
      setError('enter a destination directory first');
      return;
    }
    setBusy('save-as');
    setError('');
    setNote('');
    try {
      const out = await sendWithStaleRetry(
        getClient(),
        {
          SaveProjectAsOp: {
            container_dir: dir,
            name: saveAsName.trim(),
            reason: 'checkpoint to a new location after the previous save failed',
          },
        },
        {},
      );
      if (receiptFailed(out.receipt)) {
        setError(describeReceiptError(out.receipt));
      } else {
        setNote('save-as issued — durable only once the engine reports SAVE_DURABLE for the new container');
        setSaveAsOpen(false);
      }
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(null);
    }
  };

  React.useEffect(() => {
    ensureRecoveryBound();
  }, []);

  // The failure shown is the latest SAVE_FAILED the wire delivered —
  // prefer the live frame, fall back to the recorded one.
  const failure = latestSave?.status === 'SAVE_FAILED' ? latestSave : lastFailed;
  const unsavedRevisions = React.useMemo(() => {
    if (!failure || !revision) return null;
    try {
      const d = BigInt(revision) - BigInt(failure.revision);
      return d > 0n ? d.toString() : '0';
    } catch {
      return null;
    }
  }, [failure, revision]);

  const retrySave = async () => {
    setBusy('save');
    setError('');
    setNote('');
    try {
      const r = await getClient().saveProject('retry after storage failure');
      if (receiptFailed(r)) {
        setError(describeReceiptError(r));
      } else {
        setNote(
          'save issued — the Saved state appears only when a durable checkpoint is published',
        );
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const writeCheckpoint = async () => {
    setBusy('checkpoint');
    setError('');
    setNote('');
    try {
      const out = await sendWithStaleRetry(
        getClient(),
        { CreateCheckpointOp: { reason: 'manual checkpoint after save failure' } },
        {},
      );
      if (receiptFailed(out.receipt)) {
        setError(describeReceiptError(out.receipt));
      } else {
        setNote('checkpoint command accepted — durable only once the engine reports SAVE_DURABLE');
      }
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(null);
    }
  };

  const canWrite = projectId !== null && engineAttached;

  return (
    <RecoveryBody>
      <RecoveryMain>
        <RecoveryHeadline>
          {failure ? 'The save did not finish.' : 'No failed save on record.'}
        </RecoveryHeadline>
        <RecoveryCopy>
          {failure
            ? 'The selected location is unavailable. Keep this session open and choose another location before closing.'
            : 'If a write fails, this screen reports it alongside the last verified checkpoint.'}
        </RecoveryCopy>
        <StatusBadge
          status={latestSave?.status === 'SAVE_DURABLE' ? 'saved' : failure ? 'error' : 'unavailable'}
          label={
            latestSave?.status === 'SAVE_DURABLE'
              ? `Saved · revision ${latestSave.revision}`
              : failure
                ? `Not saved · ${failure.error || 'write failed'}`
                : 'no save events yet'
          }
        />

        <div>
          <StateRow label="Project" value={projectId ?? 'none open'} />
          <StateRow
            label="Unsaved changes"
            value={
              failure
                ? unsavedRevisions !== null
                  ? `${unsavedRevisions} edit${unsavedRevisions === '1' ? '' : 's'} since last checkpoint`
                  : 'present since last checkpoint'
                : 'none pending'
            }
          />
          <StateRow
            label="Last good checkpoint"
            value={
              lastDurable
                ? `revision ${lastDurable.revision} · on this device`
                : 'none recorded this session'
            }
          />
          {failure ? (
            <StateRow
              label="Failure"
              value={failure.message ? `${failure.error}: ${failure.message}` : failure.error}
            />
          ) : null}
        </div>

        <div style={{ display: 'flex', gap: tokens.space8, flexWrap: 'wrap' }}>
          <ActionButton
            variant="primary"
            loading={busy === 'save'}
            disabled={!canWrite || busy !== null}
            onClick={() => void retrySave()}
          >
            Retry save
          </ActionButton>
          <ActionButton
            variant="secondary"
            loading={busy === 'checkpoint'}
            disabled={!canWrite || busy !== null}
            onClick={() => void writeCheckpoint()}
          >
            Write checkpoint
          </ActionButton>
          <ActionButton
            variant="secondary"
            disabled={!canWrite || busy !== null}
            title={canWrite ? 'SaveProjectAsOp — checkpoint into a new container dir' : 'engine detached'}
            onClick={() => setSaveAsOpen((v) => !v)}
          >
            Choose another location…
          </ActionButton>
          {saveAsOpen ? (
            <div style={{ display: 'flex', flexDirection: 'column', gap: 4, maxWidth: 420 }}>
              <input
                value={saveAsDir}
                onChange={(ev) => setSaveAsDir(ev.target.value)}
                placeholder="/path/to/new/container"
                aria-label="New container directory"
                style={{ padding: '6px 8px' }}
              />
              <input
                value={saveAsName}
                onChange={(ev) => setSaveAsName(ev.target.value)}
                placeholder="container name (optional)"
                aria-label="New container name"
                style={{ padding: '6px 8px' }}
              />
              <ActionButton
                variant="primary"
                loading={busy === 'save-as'}
                disabled={saveAsDir.trim() === '' || busy !== null}
                onClick={() => void saveAs()}
              >
                Save to new location
              </ActionButton>
            </div>
          ) : null}
        </div>

        {note ? (
          <p className="void-type-small" style={{ color: tokens.mint, margin: 0 }}>{note}</p>
        ) : null}
        {error ? (
          <p className="void-type-small" style={{ color: tokens.danger, margin: 0 }}>{error}</p>
        ) : null}
        {!canWrite ? (
          <p className="void-type-small" style={{ color: tokens.subtle, margin: 0 }}>
            {!projectId
              ? 'No project is open — nothing to save.'
              : 'Engine detached — a retry can only run once the engine re-attaches.'}
          </p>
        ) : null}

        <RecoveryNote>
          “Saved” appears only after a durable, verified checkpoint is
          published. A failed or cancelled write never discards the
          in-memory song, and never overwrites the last good checkpoint
          with staging data.
        </RecoveryNote>
      </RecoveryMain>

      <RecoveryRail title="What you can do now.">
        <RecoveryStep n="01" title="Keep the project open">
          Unsaved edits live in memory — closing loses them, a failed write
          did not.
        </RecoveryStep>
        <RecoveryStep n="02" title="Choose a writable folder">
          Free space or fix the target location outside this screen, then
          retry — no save-destination op exists on the wire to pick one here.
        </RecoveryStep>
        <RecoveryStep n="03" title="Verify before closing">
          Trust only a reported durable checkpoint — the header badge flips
          to Saved when the engine publishes it.
        </RecoveryStep>
      </RecoveryRail>
    </RecoveryBody>
  );
}
