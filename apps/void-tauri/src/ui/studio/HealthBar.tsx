import * as React from 'react';
import { tokens } from 'void-ui';
import { useStudio } from 'void-studio';

/**
 * Health bar (S04 bottom strip) — truthful states only:
 * engine attached/detached + worker, save outcome, audio device sample rate
 * when the clock reports one. Anything not exposed by the wire renders '—'
 * or is omitted; never a fabricated CPU/memory reading.
 */
export const HealthBar: React.FC = () => {
  const engine = useStudio((s) => s.engine);
  const clock = useStudio((s) => s.telemetry.clock);
  const save = useStudio((s) => s.telemetry.save);

  const engineText = engine.attached
    ? `engine attached${engine.workerId ? ` · ${engine.workerId}` : ''}${engine.state ? ` · ${engine.state}` : ''}`
    : 'engine detached';
  const audioText =
    clock && clock.sample_rate > 0
      ? `${(clock.sample_rate / 1000).toFixed(clock.sample_rate % 1000 === 0 ? 0 : 1)} kHz`
      : '—';
  const saveText = !save
    ? 'no save yet'
    : save.status === 'SAVE_DURABLE'
      ? 'Saved locally'
      : save.status === 'SAVE_FAILED'
        ? 'Save failed'
        : '—';

  return (
    <footer
      role="contentinfo"
      aria-label="Session health"
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: tokens.space16,
        height: 28,
        padding: `0 ${tokens.space16}`,
        borderTop: `1px solid ${tokens.line}`,
        background: tokens.surface,
        color: tokens.subtle,
        fontFamily: tokens.fontNumeric,
        fontSize: 11,
        flexShrink: 0,
      }}
    >
      <span role="status" style={{ color: engine.attached ? tokens.mint : tokens.subtle }}>
        {engineText}
      </span>
      <span aria-hidden style={{ color: tokens.line }}>
        ·
      </span>
      <span role="status" title="Audio device sample rate from the last clock snapshot">
        {audioText}
      </span>
      <span aria-hidden style={{ color: tokens.line }}>
        ·
      </span>
      <span
        role="status"
        style={{ color: save?.status === 'SAVE_FAILED' ? tokens.danger : 'inherit' }}
      >
        {saveText}
      </span>
    </footer>
  );
};
