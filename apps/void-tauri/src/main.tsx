import React, { useEffect, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';

interface EngineStatus {
  attached: boolean;
  worker_id?: string;
  engine_epoch?: string;
  state?: string;
}

// Minimal shell: engine lifecycle + status. Lane C replaces this with the
// void-studio workspace mounted on packages/void-client transports.
function Shell() {
  const [status, setStatus] = useState<EngineStatus>({ attached: false });
  const [telemetry, setTelemetry] = useState<Record<string, unknown> | null>(null);
  const [log, setLog] = useState<string[]>([]);

  const refresh = () =>
    invoke<EngineStatus>('engine_status').then(setStatus).catch(() => {});

  useEffect(() => {
    refresh();
    const id = setInterval(refresh, 2000);
    const unT = listen<Record<string, unknown>>('void://telemetry', (e) =>
      setTelemetry(e.payload),
    );
    const unC = listen<Record<string, unknown>>('void://control', (e) =>
      setLog((l) => [JSON.stringify(e.payload).slice(0, 200), ...l].slice(0, 8)),
    );
    const unL = listen('void://engine-lost', () => {
      setStatus({ attached: false });
      setLog((l) => ['engine lost (control channel closed)', ...l]);
    });
    return () => {
      clearInterval(id);
      unT.then((f) => f());
      unC.then((f) => f());
      unL.then((f) => f());
    };
  }, []);

  return (
    <div
      style={{
        fontFamily: 'ui-monospace, monospace',
        background: '#0a0a0a',
        color: '#ddd',
        height: '100vh',
        padding: 24,
        boxSizing: 'border-box',
      }}
    >
      <h1 style={{ fontSize: 18, margin: 0 }}>VOID Studio — Tauri shell</h1>
      <p style={{ color: '#888', fontSize: 12 }}>
        Engine:{' '}
        {status.attached
          ? `attached (${status.worker_id} · epoch ${status.engine_epoch} · ${status.state})`
          : 'not attached'}
      </p>
      <p style={{ color: '#888', fontSize: 12 }}>
        Telemetry: {telemetry ? JSON.stringify(telemetry).slice(0, 160) : '—'}
      </p>
      <div>
        {log.map((l, i) => (
          <div key={i} style={{ fontSize: 11, color: '#6a9' }}>
            {l}
          </div>
        ))}
      </div>
      <p style={{ color: '#555', fontSize: 11, marginTop: 32 }}>
        Engine worker lands via lane A (native/void-engine). Commands route
        through send_command/send_transport/read_view.
      </p>
    </div>
  );
}

createRoot(document.getElementById('root')!).render(
  <React.StrictMode>
    <Shell />
  </React.StrictMode>,
);
