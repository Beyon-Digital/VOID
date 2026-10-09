// Plugin recovery screen — S18 missing/quarantined plugins (UI-T25).
//
// Wired truth from PLUGIN_LIST (the only plugin surface on the wire):
// the engine keeps the slot — including a MISSING row carrying its
// reason — so saved state and routing are retained by construction.
// Nothing is substituted silently:
//   - "Remove instance" sends RemovePluginOp (explicit, one op);
//   - "Replace" inserts a NEW instance via InsertPluginOp only after the
//     user picks the track + descriptor — never automatic;
//   - "Locate" (rescan/install) has no command on the control surface —
//     it is shown as unavailable with the reason, not hidden;
//   - bypass is engine-reported state (PLUGIN_LIST status); there is no
//     bypass command on the wire, so the screen never fakes toggling it.

import * as React from 'react';
import { ActionButton, StatusBadge, tokens } from 'void-ui';
import {
  describeReceiptError,
  receiptFailed,
  sendWithStaleRetry,
  useStudio,
} from 'void-studio';
import { parsePluginSlots, type PluginSlotItem } from 'void-studio/src/exchange/pluginList';
import { getClient } from '../../../client';
import { loadAllPages, parseTrackRow, type TrackRow } from '../shared/views';
import {
  RecoveryBody,
  RecoveryCopy,
  RecoveryHeadline,
  RecoveryMain,
  RecoveryNote,
  RecoveryRail,
  RecoveryStep,
} from '../shared/chrome';

export default function PluginRecoveryScreen() {
  const projectId = useStudio((s) => s.projectId);
  const revision = useStudio((s) => s.revision);
  const engineAttached = useStudio((s) => s.engine.attached);

  const [slots, setSlots] = React.useState<PluginSlotItem[]>([]);
  const [tracks, setTracks] = React.useState<TrackRow[]>([]);
  const [loadError, setLoadError] = React.useState('');
  const [opError, setOpError] = React.useState('');
  const [notice, setNotice] = React.useState('');
  const [busy, setBusy] = React.useState(false);
  const [replacing, setReplacing] = React.useState<string | null>(null);
  const [replaceTrack, setReplaceTrack] = React.useState('');
  const [replaceFormat, setReplaceFormat] = React.useState('vst3');
  const [replaceUid, setReplaceUid] = React.useState('');

  const refresh = React.useCallback(async () => {
    const p = await loadAllPages('PLUGIN_LIST');
    setSlots(parsePluginSlots(p.items));
    const t = await loadAllPages('TRACK_LIST');
    setTracks(
      t.items
        .map(parseTrackRow)
        .filter((r): r is TrackRow => r !== null)
        .sort((a, b) => (a.index ?? 0) - (b.index ?? 0)),
    );
  }, []);

  React.useEffect(() => {
    if (!projectId || !engineAttached) return;
    refresh()
      .then(() => setLoadError(''))
      .catch((e) => setLoadError(String(e)));
  }, [projectId, engineAttached, revision, refresh]);

  const missing = slots.filter((s) => s.status === 'MISSING');
  const bypassed = slots.filter((s) => s.status === 'BYPASSED');
  const healthy = slots.length - missing.length - bypassed.length;

  const send = async (op: Parameters<ReturnType<typeof getClient>['sendCommand']>[0]) => {
    const out = await sendWithStaleRetry(getClient(), op, { refresh });
    if (receiptFailed(out.receipt)) throw new Error(describeReceiptError(out.receipt));
  };

  const removeInstance = async (slot: PluginSlotItem) => {
    setBusy(true);
    setOpError('');
    setNotice('');
    try {
      await send({ RemovePluginOp: { plugin_instance_id: slot.instanceId } });
      setNotice(`removed ${slot.name ?? slot.instanceId} — an explicit removal, no substitute`);
      await refresh();
    } catch (e) {
      setOpError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const replace = async (slot: PluginSlotItem) => {
    if (!replaceTrack || !replaceUid.trim()) {
      setOpError('pick a track and enter the replacement plugin uid — no substitution is made without it');
      return;
    }
    setBusy(true);
    setOpError('');
    setNotice('');
    try {
      await send({
        InsertPluginOp: {
          track_id: replaceTrack,
          plugin_instance_id: crypto.randomUUID(),
          format: replaceFormat,
          plugin_uid: replaceUid.trim(),
        },
      });
      setNotice(
        `inserted ${replaceFormat}/${replaceUid.trim()} on ${replaceTrack} — the missing slot stays until you remove it`,
      );
      setReplacing(null);
      setReplaceUid('');
      await refresh();
    } catch (e) {
      setOpError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const rows = [...missing, ...bypassed];

  return (
    <RecoveryBody>
      <RecoveryMain>
        <RecoveryHeadline>Fix the missing piece.</RecoveryHeadline>
        {rows.length > 0 ? (
          <RecoveryCopy>
            {missing.length > 0
              ? `${missing.length} plugin${missing.length === 1 ? '' : 's'} could not be loaded. `
              : ''}
            Saved state and routing are preserved — the engine keeps each
            slot. No substitute has been inserted.
          </RecoveryCopy>
        ) : (
          <RecoveryCopy>
            No missing or bypassed plugins on this project.
            {healthy > 0 ? ` ${healthy} slot${healthy === 1 ? '' : 's'} report healthy.` : ''}
          </RecoveryCopy>
        )}
        {loadError ? (
          <p className="void-type-small" style={{ color: tokens.danger, margin: 0 }}>
            {loadError}
          </p>
        ) : null}

        {rows.map((slot) => (
          <div
            key={slot.instanceId}
            style={{
              maxWidth: 560,
              border: `1px solid ${tokens.line}`,
              borderRadius: tokens.radius12,
              padding: tokens.space16,
              display: 'flex',
              flexDirection: 'column',
              gap: tokens.space8,
            }}
          >
            <div style={{ display: 'flex', alignItems: 'center', gap: tokens.space8 }}>
              <span className="void-type-body" style={{ color: tokens.text }}>
                {slot.name ?? slot.instanceId}
              </span>
              <StatusBadge
                status={slot.status === 'MISSING' ? 'unavailable' : 'armed'}
                label={slot.status === 'MISSING' ? 'missing' : 'bypassed'}
              />
            </div>
            {slot.reason ? (
              <span className="void-type-small" style={{ color: tokens.subtle }}>
                {slot.reason}
              </span>
            ) : null}
            <span className="void-type-small" style={{ color: tokens.subtle }}>
              instance {slot.instanceId}
            </span>

            <div style={{ display: 'flex', gap: tokens.space8, flexWrap: 'wrap' }}>
              <ActionButton
                variant="secondary"
                size="sm"
                disabled
                title="a plugin rescan/locate is not exposed on the control surface — install or rescan from outside this screen, then reload"
              >
                Locate… unavailable
              </ActionButton>
              <ActionButton
                variant="secondary"
                size="sm"
                disabled={busy}
                onClick={() =>
                  setReplacing(replacing === slot.instanceId ? null : slot.instanceId)
                }
              >
                Replace…
              </ActionButton>
              <ActionButton
                variant="danger"
                size="sm"
                disabled={busy}
                onClick={() => void removeInstance(slot)}
              >
                Remove instance
              </ActionButton>
            </div>

            {replacing === slot.instanceId ? (
              <div
                style={{
                  display: 'flex',
                  flexDirection: 'column',
                  gap: tokens.space8,
                  borderTop: `1px solid ${tokens.line}`,
                  paddingTop: tokens.space8,
                }}
              >
                <span className="void-type-small" style={{ color: tokens.subtle }}>
                  Insert a replacement on a track. The missing slot and its
                  saved state stay untouched until you remove it explicitly.
                </span>
                <label className="void-type-small" style={fieldLabel}>
                  Track
                  <select
                    value={replaceTrack}
                    onChange={(e) => setReplaceTrack(e.target.value)}
                    style={fieldStyle}
                  >
                    <option value="">pick a track…</option>
                    {tracks.map((t) => (
                      <option key={t.trackId} value={t.trackId}>
                        {t.name ?? t.trackId}
                      </option>
                    ))}
                  </select>
                </label>
                <div style={{ display: 'flex', gap: tokens.space8 }}>
                  <label className="void-type-small" style={fieldLabel}>
                    Format
                    <select
                      value={replaceFormat}
                      onChange={(e) => setReplaceFormat(e.target.value)}
                      style={fieldStyle}
                    >
                      <option value="vst3">vst3</option>
                      <option value="clap">clap</option>
                      <option value="vst2">vst2</option>
                      <option value="lv2">lv2</option>
                      <option value="builtin">builtin</option>
                    </select>
                  </label>
                  <label className="void-type-small" style={{ ...fieldLabel, flex: 1 }}>
                    Plugin uid
                    <input
                      value={replaceUid}
                      onChange={(e) => setReplaceUid(e.target.value)}
                      placeholder="e.g. com.vendor.plugin"
                      style={fieldStyle}
                    />
                  </label>
                </div>
                <div style={{ display: 'flex', gap: tokens.space8 }}>
                  <ActionButton
                    variant="primary"
                    size="sm"
                    loading={busy}
                    disabled={!replaceTrack || !replaceUid.trim()}
                    onClick={() => void replace(slot)}
                  >
                    Insert replacement
                  </ActionButton>
                  <ActionButton variant="ghost" size="sm" onClick={() => setReplacing(null)}>
                    Cancel
                  </ActionButton>
                </div>
              </div>
            ) : null}
          </div>
        ))}

        {notice ? (
          <p className="void-type-small" style={{ color: tokens.mint, margin: 0 }}>{notice}</p>
        ) : null}
        {opError ? (
          <p className="void-type-small" style={{ color: tokens.danger, margin: 0 }}>{opError}</p>
        ) : null}

        <RecoveryNote>
          Bypass is visible state, never a silent discard: there is no bypass
          command on the wire, so a bypassed slot stays bypassed until the
          engine resolves it. Removing a missing instance is explicit and
          undoable as part of the command journal.
        </RecoveryNote>
      </RecoveryMain>

      <RecoveryRail title="What you can do now.">
        <RecoveryStep n="01" title="Locate">
          Install or rescan the plugin — the control surface has no rescan
          command, so this happens outside this screen.
        </RecoveryStep>
        <RecoveryStep n="02" title="Replace">
          Insert a different plugin on the track you pick. The missing slot
          keeps its saved state until removed.
        </RecoveryStep>
        <RecoveryStep n="03" title="Bypass">
          A bypassed slot is reported as-is by the engine; no silent
          substitute is ever inserted.
        </RecoveryStep>
        <RecoveryNote>
          Diagnostic exports redact private paths, keys and content.
        </RecoveryNote>
      </RecoveryRail>
    </RecoveryBody>
  );
}

const fieldLabel: React.CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  gap: 4,
  color: tokens.subtle,
};

const fieldStyle: React.CSSProperties = {
  background: tokens.raised,
  color: tokens.text,
  border: `1px solid ${tokens.line}`,
  borderRadius: tokens.radius8,
  padding: '6px 8px',
  font: 'inherit',
};
