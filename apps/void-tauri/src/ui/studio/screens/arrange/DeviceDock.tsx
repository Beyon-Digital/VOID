// DeviceDock — S01 bottom dock (h300, compact h248). Contextual to the
// selected track's device chain (PLUGIN_LIST view): instrument identity,
// its first param group on ParameterKnobs (one commit = one undo
// transaction via setInstrumentParam), the slot chain, and a piano-roll
// mode that embeds the real note editor for the selected MIDI clip.

import * as React from 'react';
import {
  ActionButton,
  DeviceSlot,
  ParameterKnob,
  StatusBadge,
  injectVoidStyles,
  tokens,
} from 'void-ui';
import {
  descriptorByUid,
  describeReceiptError,
  editorStore,
  loadViewPage,
  receiptFailed,
  removeInstrument,
  setInstrumentParam,
  studioStore,
  useStudio,
  type InstrumentDescriptor,
} from 'void-studio';
import { getClient } from '../../../client';
import { useEditor } from '../../useStudioData';
import {
  instrumentBrowseStore,
  newGesture,
  useSelectedClip,
  useTrackPlugins,
} from './data';
import { PianoRollPane } from '../../../shell';

type DockMode = 'instrument' | 'piano-roll' | 'gesture';

const binding = (trackId: string) => ({
  client: getClient(),
  store: instrumentBrowseStore(),
  refreshPlugins: () =>
    loadViewPage(studioStore, getClient(), 'PLUGIN_LIST', { trackId }),
  transactionId: newGesture(),
});

export function DeviceDock({ compact }: { compact: boolean }) {
  React.useEffect(() => injectVoidStyles(), []);
  const attached = useStudio((s) => s.engine.attached);
  const selection = useStudio((s) => s.selection);
  const clipSel = useEditor((s) => s.clipSelection);
  const clip = useSelectedClip();
  const trackId = clip?.trackId ?? clipSel.trackId ?? selection.trackId;
  const { slots, loaded } = useTrackPlugins(trackId);
  const [mode, setMode] = React.useState<DockMode>('instrument');

  const instrumentSlot = slots[0] ?? null;
  const descriptor: InstrumentDescriptor | undefined = instrumentSlot
    ? descriptorByUid(instrumentSlot.pluginUid)
    : undefined;

  const dockH = compact ? 248 : 300;

  const modeBtn = (m: DockMode, label: string) => (
    <ActionButton
      key={m}
      size="sm"
      variant={mode === m ? 'primary' : 'secondary'}
      aria-pressed={mode === m}
      onClick={() => setMode(m)}
    >
      {label}
    </ActionButton>
  );

  const onKnobCommit = (
    desc: InstrumentDescriptor,
    instanceId: string,
    paramId: string,
    commit: { from: number; to: number },
  ) => {
    if (!attached || !trackId) return;
    const tx = newGesture();
    void setInstrumentParam(
      { ...binding(trackId), transactionId: tx },
      desc,
      instanceId,
      paramId,
      commit.to,
    )
      .then((out) => {
        if (receiptFailed(out.receipt)) {
          editorStore
            .getState()
            .actions.setEditError(describeReceiptError(out.receipt));
        }
      })
      .catch((e) => editorStore.getState().actions.setEditError(String(e)));
  };

  const onRemoveSlot = (instanceId: string) => {
    if (!attached || !trackId) return;
    void removeInstrument(binding(trackId), instanceId)
      .then((out) => {
        if (receiptFailed(out.receipt)) {
          editorStore
            .getState()
            .actions.setEditError(describeReceiptError(out.receipt));
        }
      })
      .catch((e) => editorStore.getState().actions.setEditError(String(e)));
  };

  return (
    <div
      role="region"
      aria-label="Device dock"
      style={{
        height: dockH,
        flexShrink: 0,
        borderTop: `1px solid ${tokens.line}`,
        background: tokens.surface,
        display: 'flex',
        flexDirection: 'column',
        overflow: 'hidden',
      }}
    >
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: tokens.space12,
          padding: `${tokens.space8} ${tokens.space16}`,
        }}
      >
        <span className="void-type-strong" style={{ color: tokens.text }}>
          {descriptor?.name ?? instrumentSlot?.name ?? 'NO DEVICE'}
        </span>
        {modeBtn('instrument', 'Instrument')}
        {modeBtn('piano-roll', 'Piano roll')}
        {modeBtn('gesture', 'Gesture')}
        <span
          className="void-type-micro"
          style={{ marginLeft: 'auto', color: tokens.subtle }}
        >
          {trackId ? `track ${trackId}` : 'no track selected'}
        </span>
      </div>

      {mode === 'instrument' ? (
        <div
          style={{
            flex: 1,
            minHeight: 0,
            display: 'flex',
            gap: tokens.space16,
            padding: `0 ${tokens.space16} ${tokens.space12}`,
          }}
        >
          {/* identity card */}
          <div
            style={{
              width: 220,
              flexShrink: 0,
              display: 'flex',
              flexDirection: 'column',
              gap: tokens.space8,
              padding: tokens.space12,
              background: tokens.raised,
              borderRadius: tokens.radius8,
            }}
          >
            {instrumentSlot ? (
              <>
                <span
                  className="void-type-heading"
                  style={{ color: tokens.text, fontSize: 28 }}
                >
                  {descriptor?.name ?? instrumentSlot.pluginUid}
                </span>
                <span className="void-type-micro" style={{ color: tokens.subtle }}>
                  {instrumentSlot.pluginUid} · slot {instrumentSlot.slotIndex}
                </span>
                <ActionButton
                  size="sm"
                  variant="secondary"
                  disabled
                  title="Per-device editors open with the rack surface; params edit below"
                >
                  Open editor
                </ActionButton>
              </>
            ) : (
              <>
                <span className="void-type-strong" style={{ color: tokens.subtle }}>
                  {loaded ? 'No instrument on this track' : 'Reading device chain…'}
                </span>
                <span className="void-type-micro" style={{ color: tokens.subtle }}>
                  {trackId
                    ? 'Load one from the sound library.'
                    : 'Select a track to see its chain.'}
                </span>
              </>
            )}
          </div>

          {/* param knobs — first 4 descriptor params, values from PLUGIN_LIST */}
          <div
            role="group"
            aria-label="Instrument parameters"
            style={{
              display: 'flex',
              alignItems: 'center',
              gap: tokens.space16,
              flexShrink: 0,
            }}
          >
            {descriptor && instrumentSlot
              ? descriptor.params.slice(0, 4).map((p) => (
                  <ParameterKnob
                    key={p.id}
                    label={p.label}
                    unit={p.unit}
                    value={
                      instrumentSlot.paramValues[p.id] ?? p.defaultValue
                    }
                    min={p.min}
                    max={p.max}
                    step={p.step}
                    defaultValue={p.defaultValue}
                    disabled={!attached}
                    onCommit={(c) =>
                      onKnobCommit(descriptor, instrumentSlot.instanceId, p.id, c)
                    }
                    aria-label={`${descriptor.name} ${p.label}`}
                  />
                ))
              : null}
          </div>

          {/* device chain */}
          <div
            style={{
              flex: 1,
              minWidth: 0,
              display: 'flex',
              flexDirection: 'column',
              gap: tokens.space8,
            }}
          >
            <span className="void-type-micro" style={{ color: tokens.subtle }}>
              DEVICE CHAIN
            </span>
            <div style={{ display: 'flex', gap: tokens.space8, flexWrap: 'wrap' }}>
              {slots.length === 0 ? (
                <span className="void-type-micro" style={{ color: tokens.subtle }}>
                  {loaded ? 'empty chain' : '—'}
                </span>
              ) : (
                slots.map((s) => (
                  <DeviceSlot
                    key={s.instanceId}
                    index={s.slotIndex}
                    name={descriptorByUid(s.pluginUid)?.name ?? s.name ?? s.pluginUid}
                    category={s.pluginUid}
                    powered={s.bypassed === false ? true : s.bypassed === true ? false : undefined}
                    onRemove={
                      attached ? () => onRemoveSlot(s.instanceId) : undefined
                    }
                  />
                ))
              )}
            </div>
            <span className="void-type-micro" style={{ color: tokens.subtle }}>
              {trackId ? `${trackId} → master` : '—'}
            </span>
            {!attached ? (
              <StatusBadge status="unavailable" label="engine detached — read-only" />
            ) : null}
          </div>
        </div>
      ) : null}

      {mode === 'piano-roll' ? (
        <div style={{ flex: 1, minHeight: 0, overflow: 'hidden' }}>
          {clip && clip.kind === 'MIDI' ? (
            <PianoRollPane />
          ) : (
            <p
              className="void-type-micro"
              style={{ padding: tokens.space16, color: tokens.subtle }}
            >
              select a MIDI clip on the timeline to edit its notes
            </p>
          )}
        </div>
      ) : null}

      {mode === 'gesture' ? (
        <div
          style={{
            flex: 1,
            padding: tokens.space16,
            display: 'flex',
            flexDirection: 'column',
            gap: tokens.space8,
          }}
        >
          <span className="void-type-small" style={{ color: tokens.subtle }}>
            Gesture capture isn’t wired in the arrange workspace — it lives on
            the compose surface.
          </span>
          <ActionButton
            size="sm"
            variant="secondary"
            style={{ alignSelf: 'flex-start' }}
            onClick={() => {
              window.location.hash = '#/compose';
            }}
          >
            Open compose
          </ActionButton>
        </div>
      ) : null}
    </div>
  );
}
