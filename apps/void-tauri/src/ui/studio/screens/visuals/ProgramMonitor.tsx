// S09 program monitor — the only surface that can carry live output.
//
// UI-T23: the output starts DISARMED; arming requires an explicit target
// pick + an explicit arm action, and changing the target drops the arm.
// UI-T24: renderer/GPU failure is surfaced — armed without frames reads
// exactly that, drop counters show up verbatim, blackout is always one
// click away.

import * as React from 'react';
import { ActionButton, tokens } from 'void-ui';
import { useStudio, type VisualOutputTarget } from 'void-studio';
import {
  deriveOutputView,
  frameMetaLine,
  framesDroppedWhileArmed,
  outputStateLabel,
  REASON_NO_CHANNEL,
  REASON_NO_DISPLAY_ENUM,
  VISUAL_COMMAND_CHANNEL,
} from './model';
import { outputIntentStore, useOutputIntent, useVisuals, visualsStore } from './stores';

interface DisplayOption {
  target: VisualOutputTarget;
  label: string;
  available: boolean;
  reason?: string;
}

/** The only route the engine implements is offscreen render+readback —
 * the winit display path is deferred (docs/visual/README.md). */
function displayOptions(): DisplayOption[] {
  return [
    { target: 'offscreen', label: 'Offscreen render (readback)', available: true },
    { target: 'window', label: 'Display / window output', available: false, reason: REASON_NO_DISPLAY_ENUM },
  ];
}

export const ProgramMonitor: React.FC = () => {
  // attached is studio (engine) state — the monitor distinguishes
  // 'engine detached' from 'visual channel down' deliberately.
  const attached = useStudio((s) => s.engine.attached);
  const intent = useOutputIntent((s) => s);
  const lastFrame = useVisuals((s) => s.lastFrame.program);
  const counters = useVisuals((s) => s.dropCounters);
  const programLayers = useVisuals((s) => s.order.program.length);
  const focused = useVisuals((s) => s.focusedChannel === 'program');
  const [pickerOpen, setPickerOpen] = React.useState(false);

  const view = deriveOutputView({ attached, intent, lastProgramFrame: lastFrame });
  const showBlack = view.state === 'blackout' || view.state === 'previewOnly' || view.state === 'displaySelected' || view.state === 'armed';

  const arm = () => {
    if (!intent.selectedTarget) return;
    // The route op rides the voidvis draft wire — not bridged into the
    // shipped shell. The arm is real view state; the note says what the
    // native side never received.
    const note = VISUAL_COMMAND_CHANNEL ? null : `route op not sent — ${REASON_NO_CHANNEL}`;
    outputIntentStore.getState().actions.setArmed(true, note);
  };

  const disarm = () => outputIntentStore.getState().actions.setArmed(false);

  return (
    <section
      aria-label="Program output monitor"
      onClick={() => visualsStore.getState().actions.focusChannel('program')}
      style={{
        width: 272,
        flex: '0 0 auto',
        display: 'flex',
        flexDirection: 'column',
        border: `1px solid ${focused ? tokens.accent : tokens.line}`,
        borderRadius: tokens.radius12,
        background: tokens.surface,
        overflow: 'hidden',
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: tokens.space8, padding: `${tokens.space8} ${tokens.space12}` }}>
        <span style={{ fontFamily: tokens.mono, fontSize: 10, letterSpacing: '0.14em', color: tokens.textMuted }}>
          PROGRAM
        </span>
        <span style={{ fontFamily: tokens.mono, fontSize: 10, color: tokens.textMuted }}>
          {programLayers} layer{programLayers === 1 ? '' : 's'}
        </span>
      </div>

      <div
        role="img"
        aria-label={`Program output — ${outputStateLabel(view.state)}. ${view.reason}`}
        style={{
          flex: 1,
          minHeight: 200,
          margin: `0 ${tokens.space12}`,
          borderRadius: tokens.radius8,
          border: `1px solid ${tokens.line}`,
          background: showBlack ? '#000' : tokens.raised,
          display: 'flex',
          flexDirection: 'column',
          alignItems: 'center',
          justifyContent: 'center',
          gap: tokens.space8,
          padding: tokens.space12,
          textAlign: 'center',
        }}
      >
        <span
          style={{
            fontFamily: tokens.mono,
            fontSize: 11,
            letterSpacing: '0.12em',
            color:
              view.state === 'live'
                ? tokens.mint
                : view.state === 'blackout'
                  ? tokens.danger
                  : view.state === 'armed'
                    ? tokens.orange
                    : tokens.textMuted,
          }}
        >
          {outputStateLabel(view.state)}
        </span>
        <span style={{ fontFamily: tokens.sans, fontSize: 11, color: tokens.textMuted, maxWidth: 220 }}>
          {view.reason}
        </span>
        {lastFrame ? (
          <span style={{ fontFamily: tokens.mono, fontSize: 9, color: tokens.textMuted, maxWidth: 220, overflowWrap: 'anywhere' }}>
            {frameMetaLine(lastFrame)}
          </span>
        ) : null}
      </div>

      {intent.armed && framesDroppedWhileArmed(counters) ? (
        <p role="alert" style={{ margin: `${tokens.space4} ${tokens.space12} 0`, fontFamily: tokens.mono, fontSize: 10, color: tokens.orange }}>
          renderer pressure — dropped {counters.droppedFrames} frames, {counters.droppedClocks} clocks, {counters.skippedRenders} renders skipped
        </p>
      ) : null}
      {intent.note ? (
        <p role="status" style={{ margin: `${tokens.space4} ${tokens.space12} 0`, fontFamily: tokens.mono, fontSize: 10, color: tokens.textMuted }}>
          {intent.note}
        </p>
      ) : null}

      <div style={{ padding: tokens.space12, display: 'flex', flexDirection: 'column', gap: tokens.space8 }}>
        {pickerOpen ? (
          <div role="listbox" aria-label="Output targets" style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            {displayOptions().map((d) => (
              <button
                key={d.target}
                role="option"
                aria-selected={intent.selectedTarget === d.target}
                disabled={!d.available}
                title={d.available ? undefined : d.reason}
                onClick={() => {
                  outputIntentStore.getState().actions.selectTarget(d.target);
                  setPickerOpen(false);
                }}
                style={{
                  display: 'flex',
                  flexDirection: 'column',
                  alignItems: 'flex-start',
                  gap: 2,
                  padding: `6px ${tokens.space8}`,
                  borderRadius: tokens.radius8,
                  border: `1px solid ${intent.selectedTarget === d.target ? tokens.accent : tokens.line}`,
                  background: tokens.raised,
                  color: d.available ? tokens.text : tokens.textMuted,
                  fontFamily: tokens.sans,
                  fontSize: 12,
                  textAlign: 'left',
                  cursor: d.available ? 'pointer' : 'not-allowed',
                }}
              >
                <span>{d.label}</span>
                {!d.available ? (
                  <span style={{ fontFamily: tokens.mono, fontSize: 9, color: tokens.textMuted }}>{d.reason}</span>
                ) : null}
              </button>
            ))}
          </div>
        ) : null}
        <div style={{ display: 'flex', gap: tokens.space8 }}>
          <ActionButton
            variant="secondary"
            size="sm"
            fullWidth
            aria-expanded={pickerOpen}
            onClick={() => setPickerOpen((v) => !v)}
          >
            {intent.selectedTarget ? `Target: ${intent.selectedTarget}` : 'Choose display'}
          </ActionButton>
          {intent.armed ? (
            <ActionButton variant="secondary" size="sm" fullWidth onClick={disarm}>
              Disarm
            </ActionButton>
          ) : (
            <ActionButton
              variant="secondary"
              size="sm"
              fullWidth
              disabled={!intent.selectedTarget}
              title={intent.selectedTarget ? 'Arm program output' : 'pick an output target first'}
              onClick={arm}
            >
              Arm output
            </ActionButton>
          )}
        </div>
        <span style={{ fontFamily: tokens.sans, fontSize: 10, color: tokens.textMuted, textAlign: 'center' }}>
          {view.state === 'previewOnly'
            ? 'Nothing is sent to a display until you arm an output. Rehearse in preview first.'
            : 'Program output follows the armed target — preview never goes live by itself.'}
        </span>
      </div>
    </section>
  );
};
