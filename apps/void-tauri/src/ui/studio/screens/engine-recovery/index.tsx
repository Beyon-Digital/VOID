// Engine recovery screen — S19 (UI-T26, UI-T31).
//
// Truthful engine-stopped surface:
//   - audio is stopped, stated plainly — VOID never kept playing, and
//     restart does NOT resume transport (restart returns stopped);
//   - the last VERIFIED checkpoint is shown from the recovery store —
//     only SaveResultEvent/SAVE_DURABLE rows the wire actually delivered;
//   - restart goes through the real spawn_engine invoke with the path
//     the operator supplies — there is no bundled engine to pretend;
//   - engine state lives natively: a renderer reload loses none of it
//     (engine-lost arrives via the event channel, not a WebView timer).

import * as React from 'react';
import { ActionButton, StatusBadge, tokens } from 'void-ui';
import { studioStore, useStudio } from 'void-studio';
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

export default function EngineRecoveryScreen() {
  const engine = useStudio((s) => s.engine);
  const clock = useStudio((s) => s.telemetry.clock);
  const lastLost = useRecovery((s) => s.lastEngineLost);
  const lastDurable = useRecovery((s) => s.lastDurableSave);
  const lastExe = useRecovery((s) => s.lastSpawnExecutable);
  const actions = useRecovery((s) => s.actions);

  const [exe, setExe] = React.useState('');
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState('');
  const [restarted, setRestarted] = React.useState<string | null>(null);

  React.useEffect(() => {
    ensureRecoveryBound();
  }, []);

  React.useEffect(() => {
    if (engine.attached) actions.noteEngineAttached();
  }, [engine.attached, actions]);

  const restart = async () => {
    const path = exe.trim() || lastExe;
    if (!path) {
      setError('enter the engine executable path — VOID does not bundle one');
      return;
    }
    setBusy(true);
    setError('');
    setRestarted(null);
    try {
      actions.noteSpawnExecutable(path);
      const s = await getClient().spawnEngine(path);
      const st = await getClient().engineStatus();
      studioStore.getState().actions.setEngine({
        attached: st.attached,
        workerId: st.worker_id,
        epoch: st.engine_epoch,
        state: st.state,
      });
      setRestarted(
        `engine attached (worker ${s.worker_id ?? st.worker_id ?? '?'}) — transport stopped, nothing auto-plays`,
      );
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const stopped = !engine.attached;
  const transport = clock?.transport ?? 'STOPPED';

  return (
    <RecoveryBody>
      <RecoveryMain>
        <RecoveryHeadline>
          {stopped ? 'Playback has stopped.' : 'Engine is running.'}
        </RecoveryHeadline>
        <RecoveryCopy>
          {stopped
            ? 'The audio engine exited. VOID did not keep playing, and it will not restart the transport automatically.'
            : 'The engine is attached. If it exits, this screen reports the loss and the last verified checkpoint.'}
        </RecoveryCopy>
        <StatusBadge
          status={stopped ? 'disconnected' : transport === 'PLAYING' ? 'playing' : 'ready'}
          label={
            stopped
              ? lastLost
                ? `engine lost${lastLost.workerId ? ` · worker ${lastLost.workerId}` : ''}`
                : 'engine detached'
              : `attached${engine.workerId ? ` · worker ${engine.workerId}` : ''} · ${transport.toLowerCase()}`
          }
        />

        <div>
          <StateRow
            label="Last verified checkpoint"
            value={
              lastDurable
                ? `revision ${lastDurable.revision} · ${lastDurable.checkpoint_id || 'checkpoint recorded'}`
                : 'none recorded this session'
            }
          />
          <StateRow
            label="Recovery"
            value={stopped ? 'restart starts stopped — verified, never auto-played' : 'not needed'}
          />
          <StateRow
            label="Recording"
            value={
              stopped
                ? 'audio stopped with the engine — no salvage claim is made'
                : 'live'
            }
          />
        </div>

        {stopped ? (
          <div
            style={{
              display: 'flex',
              flexDirection: 'column',
              gap: tokens.space8,
              maxWidth: 480,
            }}
          >
            <label className="void-type-small" style={{ color: tokens.subtle, display: 'flex', flexDirection: 'column', gap: 4 }}>
              Engine executable
              <input
                value={exe}
                onChange={(e) => setExe(e.target.value)}
                placeholder={lastExe ?? './target/debug/void-mock-worker'}
                style={{
                  background: tokens.raised,
                  color: tokens.text,
                  border: `1px solid ${tokens.line}`,
                  borderRadius: tokens.radius8,
                  padding: '6px 8px',
                  font: 'inherit',
                }}
              />
            </label>
            <div style={{ display: 'flex', gap: tokens.space8 }}>
              <ActionButton variant="primary" loading={busy} onClick={() => void restart()}>
                Restart engine — starts stopped
              </ActionButton>
            </div>
          </div>
        ) : null}

        {restarted ? (
          <p className="void-type-small" style={{ color: tokens.mint, margin: 0 }}>{restarted}</p>
        ) : null}
        {error ? (
          <p className="void-type-small" style={{ color: tokens.danger, margin: 0 }}>{error}</p>
        ) : null}

        <RecoveryNote>
          A scanner subprocess is not runtime plugin isolation. Restart
          returns stopped.
        </RecoveryNote>
        <RecoveryNote>
          Engine state survives a renderer reload: it is reported over the
          event channel, never kept alive by a WebView timer.
        </RecoveryNote>
      </RecoveryMain>

      <RecoveryRail title="What you can do now.">
        <RecoveryStep n="01" title="Restart stopped">
          Spawn the engine again. The transport comes back stopped — nothing
          resumes on its own.
        </RecoveryStep>
        <RecoveryStep n="02" title="Verify the checkpoint">
          {lastDurable
            ? `Last durable checkpoint at revision ${lastDurable.revision}.`
            : 'No verified checkpoint has been reported this session.'}
        </RecoveryStep>
        <RecoveryStep n="03" title="Re-enable carefully">
          Re-arm recording and playback only after the engine reports
          attached.
        </RecoveryStep>
      </RecoveryRail>
    </RecoveryBody>
  );
}
