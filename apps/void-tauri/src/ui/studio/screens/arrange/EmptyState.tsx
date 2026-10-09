// S27 Arrange / empty new project (Figma 12:2479).
//
// Shown only when the open project is provably empty: TRACK_LIST read and
// zero tracks (the caller gates on that). Every choice runs a real op —
// AddTrackOp (+ InsertPluginOp for an instrument, same as the template
// apply path) — then routes to the surface that continues the flow. No
// seeded content; the drop hint is honest about the missing ingest op.

import * as React from 'react';
import { tokens } from 'void-ui';
import { BUILTIN_INSTRUMENTS, insertInstrumentOp, useStudio } from 'void-studio';
import { receiptOk } from 'void-client';
import type { InstrumentDescriptor } from 'void-studio';
import { getClient } from '../../../client';

function navigate(id: string) {
  window.location.hash = `/${id}`;
}

const choiceCard: React.CSSProperties = {
  display: 'flex',
  flexDirection: 'column',
  gap: 4,
  alignItems: 'flex-start',
  padding: tokens.space16,
  borderRadius: tokens.radius12,
  border: `1px solid ${tokens.line}`,
  background: tokens.surface,
  color: tokens.text,
  cursor: 'pointer',
  textAlign: 'left',
  minWidth: 180,
};

export function EmptyState() {
  const projectId = useStudio((s) => s.projectId);
  const [busy, setBusy] = React.useState<string | null>(null);
  const [error, setError] = React.useState('');
  const [instrument, setInstrument] = React.useState<InstrumentDescriptor>(BUILTIN_INSTRUMENTS[0]);

  const addAudioTrack = async () => {
    setBusy('audio');
    setError('');
    try {
      const receipt = await getClient().addTrack({
        trackId: crypto.randomUUID(),
        kind: 'AUDIO',
        name: 'Audio 1',
      });
      if (receiptOk(receipt)) navigate('record');
      else setError(`AddTrackOp rejected (${receipt.error})${receipt.message ? ` — ${receipt.message}` : ''}`);
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(null);
    }
  };

  const addInstrumentTrack = async () => {
    setBusy('instrument');
    setError('');
    try {
      const c = getClient();
      const trackId = crypto.randomUUID();
      const add = await c.addTrack({ trackId, kind: 'INSTRUMENT', name: instrument.name });
      if (!receiptOk(add)) {
        setError(`AddTrackOp rejected (${add.error})${add.message ? ` — ${add.message}` : ''}`);
        return;
      }
      const ins = await c.sendCommand(insertInstrumentOp(trackId, instrument, crypto.randomUUID()));
      if (receiptOk(ins)) navigate('compose');
      else setError(`InsertPluginOp rejected (${ins.error})${ins.message ? ` — ${ins.message}` : ''}`);
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(null);
    }
  };

  const addMidiTrack = async () => {
    setBusy('midi');
    setError('');
    try {
      const receipt = await getClient().addTrack({
        trackId: crypto.randomUUID(),
        kind: 'MIDI',
        name: 'MIDI 1',
      });
      if (receiptOk(receipt)) navigate('compose');
      else setError(`AddTrackOp rejected (${receipt.error})${receipt.message ? ` — ${receipt.message}` : ''}`);
    } catch (e) {
      setError(String(e instanceof Error ? e.message : e));
    } finally {
      setBusy(null);
    }
  };

  return (
    <div
      style={{
        flex: 1,
        display: 'flex',
        flexDirection: 'column',
        alignItems: 'center',
        justifyContent: 'center',
        gap: tokens.space16,
        padding: tokens.space24,
        minWidth: 0,
      }}
    >
      <p
        style={{
          margin: 0,
          fontSize: 11,
          fontWeight: 500,
          letterSpacing: '0.08em',
          textTransform: 'uppercase',
          color: tokens.subtle,
          fontFamily: tokens.mono,
        }}
      >
        {projectId ? 'untitled session' : 'no project'}
      </p>
      <h2 style={{ margin: 0, fontSize: 24, fontWeight: 700, color: tokens.text, fontFamily: tokens.fontDisplay }}>
        This project is empty.
      </h2>
      <p style={{ margin: 0, fontSize: 13, color: tokens.muted }}>
        Choose an input, arm a track, and capture a performance.
      </p>

      <div role="group" aria-label="First-track choices" style={{ display: 'flex', gap: tokens.space12, flexWrap: 'wrap', justifyContent: 'center' }}>
        <button type="button" style={choiceCard} disabled={busy !== null} onClick={() => void addAudioTrack()}>
          <span style={{ fontSize: 13, fontWeight: 600 }}>{busy === 'audio' ? 'Adding…' : 'Record audio'}</span>
          <span style={{ fontSize: 11, color: tokens.muted }}>Sing or play into a take.</span>
        </button>
        <div style={{ ...choiceCard, padding: 0, border: 'none', background: 'none', cursor: 'default' }}>
          <button type="button" style={choiceCard} disabled={busy !== null} onClick={() => void addInstrumentTrack()}>
            <span style={{ fontSize: 13, fontWeight: 600 }}>{busy === 'instrument' ? 'Adding…' : 'Play an instrument'}</span>
            <span style={{ fontSize: 11, color: tokens.muted }}>Keys, drums, anything midi.</span>
            <span style={{ display: 'flex', gap: 4, marginTop: 4 }}>
              {BUILTIN_INSTRUMENTS.map((d) => (
                <span
                  key={d.pluginUid}
                  role="button"
                  tabIndex={0}
                  aria-pressed={instrument.pluginUid === d.pluginUid}
                  onClick={(e) => {
                    e.stopPropagation();
                    setInstrument(d);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ') {
                      e.stopPropagation();
                      setInstrument(d);
                    }
                  }}
                  style={{
                    fontSize: 10,
                    padding: '2px 8px',
                    borderRadius: tokens.radius8,
                    border: `1px solid ${instrument.pluginUid === d.pluginUid ? tokens.accent : tokens.line}`,
                    color: instrument.pluginUid === d.pluginUid ? tokens.accent : tokens.muted,
                    background: tokens.raised,
                  }}
                >
                  {d.name}
                </span>
              ))}
            </span>
          </button>
        </div>
        <button type="button" style={choiceCard} disabled={busy !== null} onClick={() => void addMidiTrack()}>
          <span style={{ fontSize: 13, fontWeight: 600 }}>{busy === 'midi' ? 'Adding…' : 'Draw a phrase'}</span>
          <span style={{ fontSize: 11, color: tokens.muted }}>Turn a contour into notes you can still edit.</span>
        </button>
      </div>

      {error ? (
        <p role="alert" style={{ margin: 0, fontSize: 12, color: tokens.danger }}>
          {error}
        </p>
      ) : null}

      <p style={{ margin: 0, fontSize: 11, color: tokens.subtle, textAlign: 'center', maxWidth: 460 }}>
        Or drag in an audio file. No AI model, internet connection or account is required.
        <br />
        File ingest is not on the wire yet — assets must already live inside the project container (NEEDS §17).
      </p>
    </div>
  );
}
