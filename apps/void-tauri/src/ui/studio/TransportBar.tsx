import * as React from 'react';
import {
  ActionButton,
  StatusBadge,
  TransportControls,
  formatBarBeat,
  tokens,
} from 'void-ui';
import {
  describeReceiptError,
  editorStore,
  receiptFailed,
  setLiveCycle,
  setLoopRange,
  transportBarState,
  useStudio,
} from 'void-studio';
import { getClient } from '../client';
import { useProjectSummary } from './useStudioData';

const newGesture = () => crypto.randomUUID();

export interface TransportBarProps {
  onOpenExport: () => void;
}

/**
 * Transport bar (S01): play/stop/panic + cycle against the REAL client,
 * musical position readout, tempo from PROJECT_SUMMARY, honest key/meter
 * placeholder (the wire ships neither), engine status badge, export button.
 */
export const TransportBar: React.FC<TransportBarProps> = ({ onOpenExport }) => {
  const clock = useStudio((s) => s.telemetry.clock);
  const attached = useStudio((s) => s.engine.attached);
  const viewport = useStudio((s) => s.viewport);
  const summary = useProjectSummary();
  const [busy, setBusy] = React.useState(false);
  const bar = transportBarState(clock, attached);

  const run = (fn: () => Promise<unknown>) => {
    setBusy(true);
    return fn()
      .catch((e) => editorStore.getState().actions.setEditError(String(e)))
      .finally(() => setBusy(false));
  };

  const cycleLabel = bar.cycle.active
    ? `${formatBarBeat(bar.cycle.startTicks)} → ${formatBarBeat(bar.cycle.endTicks)}`
    : 'off';

  const toggleCycle = () =>
    run(async () => {
      if (bar.cycle.active) {
        const out = await setLoopRange(
          getClient(),
          { startTicks: bar.cycle.startTicks, endTicks: bar.cycle.endTicks, enabled: false },
          { transactionId: newGesture() },
        );
        if (receiptFailed(out.receipt)) {
          editorStore.getState().actions.setEditError(describeReceiptError(out.receipt));
        }
      } else {
        const tx = newGesture();
        await setLiveCycle(getClient(), viewport.startTicks, viewport.endTicks);
        const out = await setLoopRange(
          getClient(),
          { startTicks: viewport.startTicks, endTicks: viewport.endTicks, enabled: true },
          { transactionId: tx },
        );
        if (receiptFailed(out.receipt)) {
          editorStore.getState().actions.setEditError(describeReceiptError(out.receipt));
        }
      }
    });

  const transportBadge: 'playing' | 'recording' | 'ready' | 'disconnected' = !attached
    ? 'disconnected'
    : bar.transport === 'PLAYING'
      ? 'playing'
      : bar.transport === 'RECORDING'
        ? 'recording'
        : 'ready';

  return (
    <div
      role="region"
      aria-label="Transport"
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: tokens.space16,
        height: 64,
        padding: `0 ${tokens.space16}`,
        borderBottom: `1px solid ${tokens.line}`,
        background: tokens.surface,
        flexShrink: 0,
      }}
    >
      <TransportControls
        transport={bar.transport}
        positionText={
          bar.timelineSample !== null ? `pos ${bar.timelineSample} samples` : 'no clock'
        }
        cycle={{ active: bar.cycle.active, label: cycleLabel }}
        busy={busy}
        disabled={!attached}
        onPlay={() => run(() => getClient().play())}
        onStop={() => run(() => getClient().stop())}
        onPanic={() => run(() => getClient().panic())}
        onToggleCycle={toggleCycle}
      />

      <div
        className="void-type-time"
        aria-label="Musical position"
        style={{ color: tokens.text, minWidth: 120 }}
      >
        {bar.cycle.active
          ? `${formatBarBeat(bar.cycle.startTicks)}`
          : bar.timelineSample !== null
            ? bar.timelineSample
            : '—'}
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
        <span className="void-type-micro" style={{ color: tokens.subtle }}>
          TEMPO
        </span>
        <span className="void-type-numeric" style={{ color: tokens.text }}>
          {summary.bpm !== null ? `${summary.bpm}` : '—'}
        </span>
      </div>

      <div style={{ display: 'flex', flexDirection: 'column', gap: 2 }}>
        <span className="void-type-micro" style={{ color: tokens.subtle }}>
          KEY · METER
        </span>
        <span className="void-type-numeric" style={{ color: tokens.subtle }} title="Key and meter are not exposed by any read view yet">
          —
        </span>
      </div>

      <div style={{ marginLeft: 'auto', display: 'flex', alignItems: 'center', gap: tokens.space12 }}>
        <StatusBadge status={transportBadge} />
        <ActionButton size="sm" variant="secondary" onClick={onOpenExport}>
          Export
        </ActionButton>
      </div>
    </div>
  );
};
