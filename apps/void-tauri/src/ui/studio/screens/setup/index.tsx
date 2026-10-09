// S15 Setup / audio and MIDI (Figma 4:1408).
//
// Real data: engine attach state, the live ClockSnapshot (sample rate) and
// the recording phase inside PROJECT_SUMMARY. There is no device-enumeration
// view on the wire yet (docs/engine/NEEDS.md §8 INPUT_DEVICE_LIST), so device
// fields render the honest state — '—' with the reason — rather than a fake
// picker (UI-T09). Theme toggle is real (useVoidTheme).

import * as React from 'react';
import { ActionButton, StatusBadge, tokens, useVoidTheme } from 'void-ui';
import { parseRecordingSummary, useStudio } from 'void-studio';
import { useProjectSummary } from '../../useStudioData';

function navigate(id: string) {
  window.location.hash = `/${id}`;
}

const label: React.CSSProperties = {
  fontSize: 11,
  fontWeight: 500,
  letterSpacing: '0.08em',
  textTransform: 'uppercase',
  color: tokens.subtle,
  margin: '0 0 10px',
};

const card: React.CSSProperties = {
  padding: tokens.space16,
  borderRadius: tokens.radius8,
  border: `1px solid ${tokens.line}`,
  background: tokens.surface,
};

function DeviceRow(props: { label: string; value: string; hint: string }) {
  return (
    <div
      style={{
        display: 'flex',
        alignItems: 'baseline',
        gap: tokens.space12,
        padding: `${tokens.space8} 0`,
        borderBottom: `1px solid ${tokens.line}`,
      }}
    >
      <span style={{ width: 140, flexShrink: 0, fontSize: 12, color: tokens.muted }}>{props.label}</span>
      <span style={{ flex: 1, fontSize: 13, color: tokens.text, fontFamily: tokens.mono }}>{props.value}</span>
      <span style={{ fontSize: 11, color: tokens.subtle, maxWidth: 320, textAlign: 'right' }}>{props.hint}</span>
    </div>
  );
}

export default function SetupScreen() {
  const attached = useStudio((s) => s.engine.attached);
  const engineState = useStudio((s) => s.engine.state);
  const clock = useStudio((s) => s.telemetry.clock);
  const summary = useProjectSummary();
  const recording = parseRecordingSummary(summary.recording ?? undefined);
  const theme = useVoidTheme();

  const recordingActive = recording?.phase === 'recording' || recording?.phase === 'stopping';
  const noDeviceView = 'Device enumeration has no wire view yet (NEEDS §8 INPUT_DEVICE_LIST)';

  return (
    <div style={{ display: 'flex', flex: 1, minWidth: 0, minHeight: 0 }}>
      {/* Settings nav — only Audio & MIDI is built in this lane; the rest are
          honest placeholders pending their own lanes. */}
      <nav
        aria-label="Settings"
        style={{
          width: 200,
          flexShrink: 0,
          display: 'flex',
          flexDirection: 'column',
          gap: 2,
          padding: tokens.space16,
          borderRight: `1px solid ${tokens.line}`,
          background: tokens.surface,
        }}
      >
        <p style={label}>Settings</p>
        {['Audio & MIDI', 'Appearance', 'Keyboard shortcuts', 'Models & storage', 'Privacy'].map((x, i) => (
          <button
            key={x}
            type="button"
            disabled={i !== 0}
            title={i === 0 ? undefined : 'not built yet'}
            style={{
              display: 'block',
              textAlign: 'left',
              width: '100%',
              padding: `${tokens.space8} ${tokens.space12}`,
              borderRadius: tokens.radius8,
              border: 'none',
              background: i === 0 ? tokens.raised : 'none',
              color: i === 0 ? tokens.text : tokens.subtle,
              fontSize: 13,
              cursor: i === 0 ? 'pointer' : 'default',
            }}
          >
            {x}
          </button>
        ))}
      </nav>

      <div style={{ flex: 1, minWidth: 0, overflowY: 'auto', padding: tokens.space24, display: 'flex', flexDirection: 'column', gap: tokens.space16 }}>
        <header>
          <h1 style={{ margin: 0, fontSize: 24, fontWeight: 700, color: tokens.text, fontFamily: tokens.fontDisplay }}>
            Hear every decision.
          </h1>
          <p style={{ margin: `${tokens.space4} 0 0`, fontSize: 12, color: tokens.muted }}>
            Device settings belong to the native audio engine. Changes apply when recording is stopped.
          </p>
        </header>

        {recordingActive ? (
          <div role="alert" style={{ ...card, borderColor: tokens.danger }}>
            <p style={{ margin: 0, fontSize: 12, color: tokens.danger }}>
              A recording is in progress ({recording?.phase}) — playback pauses only for an explicit device change.
            </p>
          </div>
        ) : null}

        <section aria-label="Audio" style={card}>
          <p style={label}>Audio</p>
          <DeviceRow
            label="Output device"
            value={attached ? `Engine attached · ${engineState}` : '—'}
            hint={attached ? 'Engine-reported device names need NEEDS §8' : 'No engine attached'}
          />
          <DeviceRow label="Input device" value="—" hint={noDeviceView} />
          <DeviceRow
            label="Sample rate"
            value={clock ? `${clock.sample_rate / 1000} kHz` : '—'}
            hint={clock ? 'live from the engine clock' : 'engine clock not streaming'}
          />
          <DeviceRow label="Buffer size" value="—" hint="no buffer-size field on the wire (NEEDS §8)" />
          <p style={{ margin: `${tokens.space12} 0 0`, fontSize: 11, color: tokens.subtle }}>
            Smaller buffer, more real-time work. Measure round-trip latency with your device — this build does not claim
            a latency benchmark.
          </p>
          <div style={{ marginTop: tokens.space8 }}>
            <ActionButton variant="secondary" size="sm" disabled title="no test-tone op on the wire">
              Test output
            </ActionButton>
          </div>
        </section>

        <section aria-label="MIDI" style={card}>
          <p style={label}>MIDI</p>
          <div style={{ display: 'flex', alignItems: 'center', gap: tokens.space12 }}>
            <StatusBadge status="unavailable" label="No devices reported" />
            <p style={{ margin: 0, fontSize: 12, color: tokens.muted }}>
              MIDI device enumeration is not on the wire yet (NEEDS §8). The manual path stays open: instruments play
              from step input and the piano roll.
            </p>
          </div>
        </section>

        <section aria-label="Permissions" style={card}>
          <p style={label}>Permissions</p>
          <div style={{ display: 'flex', flexDirection: 'column', gap: tokens.space8 }}>
            <div style={{ display: 'flex', gap: tokens.space12, alignItems: 'baseline' }}>
              <span style={{ width: 120, fontSize: 12, color: tokens.muted }}>Microphone</span>
              <StatusBadge status="queued" label="Not requested" />
              <span style={{ fontSize: 11, color: tokens.subtle }}>
                Required only when recording a microphone — requested by the engine at arm time.
              </span>
            </div>
            <div style={{ display: 'flex', gap: tokens.space12, alignItems: 'baseline' }}>
              <span style={{ width: 120, fontSize: 12, color: tokens.muted }}>Camera</span>
              <StatusBadge status="queued" label="Not requested" />
              <span style={{ fontSize: 11, color: tokens.subtle }}>
                Camera is never required for pointer gestures. No automatic upload or model training.
              </span>
            </div>
            <div style={{ display: 'flex', gap: tokens.space12, alignItems: 'baseline' }}>
              <span style={{ width: 120, fontSize: 12, color: tokens.muted }}>Cloud upload</span>
              <StatusBadge status="queued" label="Off" />
              <span style={{ fontSize: 11, color: tokens.subtle }}>Nothing uploads. Local first, yours by default.</span>
            </div>
          </div>
        </section>

        <section aria-label="Appearance" style={card}>
          <p style={label}>Appearance</p>
          <div style={{ display: 'flex', alignItems: 'center', gap: tokens.space12 }}>
            <ActionButton variant="secondary" size="sm" onClick={() => theme.toggleTheme()}>
              Daylight theme
            </ActionButton>
            <span style={{ fontSize: 11, color: tokens.subtle }}>current: {theme.theme}</span>
          </div>
        </section>

        <div style={{ display: 'flex', gap: tokens.space8 }}>
          <ActionButton variant="primary" onClick={() => navigate('arrange')}>
            Apply &amp; return
          </ActionButton>
          <p style={{ margin: 0, alignSelf: 'center', fontSize: 11, color: tokens.subtle }}>
            No pending device changes — fields above are read-only until the engine exposes configure ops.
          </p>
        </div>
      </div>
    </div>
  );
}
