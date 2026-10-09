// S09 inspector — selected layer view state + generation + blackout.
//
// Parameter knobs show the layer's real transform values but editing is
// disabled until the visual command channel is bridged (the SetLayer*
// ops ride the draft voidvis wire). Beat sync reads the channel's real
// transition quantize. The audio-reactivity contract is still a proposed
// dependency (analysis.bindings) — it renders as an honest note, not a
// fake binding table.

import * as React from 'react';
import { ActionButton, Field, ParameterKnob, TextInput, tokens } from 'void-ui';
import { formatBarBeat } from 'void-ui';
import {
  JOB_SUBMIT_CHANNEL,
  layerKindLabel,
  REASON_NO_JOB_SUBMIT,
} from './model';
import { outputIntentStore, useOutputIntent, useVisGen, useVisuals } from './stores';

const label: React.CSSProperties = {
  fontFamily: tokens.mono,
  fontSize: 10,
  letterSpacing: '0.14em',
  textTransform: 'uppercase',
  color: tokens.textMuted,
};

const value: React.CSSProperties = {
  fontFamily: tokens.mono,
  fontSize: 11,
  color: tokens.text,
};

function Row({ k, v }: { k: string; v: string }) {
  return (
    <div style={{ display: 'flex', justifyContent: 'space-between', gap: tokens.space8 }}>
      <span style={label}>{k}</span>
      <span style={value}>{v}</span>
    </div>
  );
}

export const InspectorPane: React.FC = () => {
  const layers = useVisuals((s) => s.layers);
  const selected = useVisuals((s) => s.selectedLayerIds);
  const transition = useVisuals((s) => s.transitions.preview);
  const blackout = useOutputIntent((s) => s.blackout);
  const records = useVisGen((s) => s.records);
  const recordOrder = useVisGen((s) => s.order);

  const layer = selected.length > 0 ? layers[selected[0]] ?? null : null;
  const [prompt, setPrompt] = React.useState('');
  const [genNote, setGenNote] = React.useState<string | null>(null);

  const deg = layer ? (layer.rotationRad * 180) / Math.PI : 0;

  return (
    <aside
      aria-label="Layer inspector"
      style={{
        width: 320,
        flex: '0 0 auto',
        display: 'flex',
        flexDirection: 'column',
        gap: tokens.space12,
        borderLeft: `1px solid ${tokens.line}`,
        background: tokens.surface,
        padding: tokens.space12,
        overflow: 'auto',
        minHeight: 0,
      }}
    >
      <div style={{ display: 'flex', alignItems: 'center', gap: tokens.space8 }}>
        <span
          style={{
            fontFamily: tokens.mono,
            fontSize: 9,
            padding: '2px 6px',
            borderRadius: tokens.radius4,
            background: tokens.violetSoft,
            color: tokens.violet,
            letterSpacing: '0.1em',
          }}
        >
          {layer ? layerKindLabel(layer.kind) : 'NO LAYER'}
        </span>
        <span style={{ fontFamily: tokens.sans, fontSize: 14, color: tokens.text }}>
          {layer ? layer.name : 'Nothing selected'}
        </span>
      </div>
      <span style={{ ...label, textTransform: 'none', letterSpacing: 0 }}>
        {layer ? `Layer ${String(layer.index + 1).padStart(2, '0')} · selected · ${layer.channel}` : 'select a layer in the list or timeline'}
      </span>

      {layer ? (
        <>
          <div style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
            <Row k="Blend" v={layer.blend} />
            <Row k="Opacity" v={`${Math.round(layer.opacity * 100)}%`} />
            <Row
              k="Beat sync"
              v={transition ? transition.quantize.replace('_', ' ') : 'immediate'}
            />
            <Row
              k="Trim"
              v={
                layer.inTicks >= 0 || layer.outTicks >= 0
                  ? `${formatBarBeat(layer.inTicks >= 0 ? layer.inTicks : 0)} → ${
                      layer.outTicks >= 0 ? formatBarBeat(layer.outTicks) : 'end'
                    }`
                  : 'full range'
              }
            />
            {layer.assetId ? <Row k="Asset" v={layer.assetId.slice(0, 12)} /> : null}
            {layer.generatorPreset ? <Row k="Preset" v={layer.generatorPreset} /> : null}
          </div>

          <div style={{ display: 'flex', gap: tokens.space8, justifyContent: 'space-between' }}>
            <ParameterKnob
              label="Scale"
              value={layer.scaleX}
              min={0}
              max={4}
              disabled
              format={(v) => `${v.toFixed(2)}×`}
              aria-label="Scale (read-only — visual channel not bridged)"
            />
            <ParameterKnob
              label="Opacity"
              value={layer.opacity}
              min={0}
              max={1}
              disabled
              format={(v) => `${Math.round(v * 100)}%`}
              aria-label="Opacity (read-only — visual channel not bridged)"
            />
            <ParameterKnob
              label="Rotation"
              value={deg}
              min={-180}
              max={180}
              disabled
              format={(v) => `${v.toFixed(0)}°`}
              aria-label="Rotation (read-only — visual channel not bridged)"
            />
          </div>
        </>
      ) : null}

      <section aria-label="Audio reactivity" style={{ display: 'flex', flexDirection: 'column', gap: 6 }}>
        <span style={label}>Audio reactivity</span>
        <span style={{ fontFamily: tokens.sans, fontSize: 11, color: tokens.textMuted }}>
          No analysis bindings on this layer — audio-reactivity bindings are a
          proposed contract (analysis.bindings), not wired into the engine yet.
        </span>
        <span style={{ fontFamily: tokens.sans, fontSize: 11, color: tokens.textMuted }}>
          Bounded analysis follows the native engine. Late frames never hold a
          musical cue.
        </span>
      </section>

      <section
        aria-label="Create a visual asset"
        style={{
          display: 'flex',
          flexDirection: 'column',
          gap: tokens.space8,
          border: `1px solid ${tokens.violet}`,
          borderRadius: tokens.radius12,
          padding: tokens.space12,
          background: tokens.violetSoft,
        }}
      >
        <span style={{ ...label, color: tokens.violet }}>Create a visual asset</span>
        <Field label="Prompt">
          {(id) => (
            <TextInput
              id={id}
              value={prompt}
              placeholder="Describe the visual to generate…"
              onChange={(e) => setPrompt(e.target.value)}
            />
          )}
        </Field>
        <ActionButton
          variant="secondary"
          size="sm"
          disabled={!prompt.trim() || !JOB_SUBMIT_CHANNEL}
          title={JOB_SUBMIT_CHANNEL ? 'submit a visual-generation job' : `unavailable — ${REASON_NO_JOB_SUBMIT}`}
          onClick={() => {
            if (!JOB_SUBMIT_CHANNEL) {
              setGenNote(`generate unavailable — ${REASON_NO_JOB_SUBMIT}`);
              return;
            }
          }}
        >
          Generate preview
        </ActionButton>
        {genNote ? (
          <p role="status" style={{ margin: 0, fontFamily: tokens.mono, fontSize: 10, color: tokens.textMuted }}>
            {genNote}
          </p>
        ) : null}
        {recordOrder.length > 0 ? (
          <div role="list" aria-label="Generation records" style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            {recordOrder.map((id) => {
              const r = records[id];
              if (!r) return null;
              return (
                <span key={id} style={{ fontFamily: tokens.mono, fontSize: 10, color: tokens.text }}>
                  {r.status} · {r.description ?? r.recordId.slice(0, 8)}
                  {r.error ? ` — ${r.error}` : ''}
                </span>
              );
            })}
          </div>
        ) : null}
      </section>

      <section aria-label="Output safety" style={{ marginTop: 'auto' }}>
        <ActionButton
          variant="danger"
          size="sm"
          fullWidth
          aria-pressed={blackout}
          onClick={() => outputIntentStore.getState().actions.setBlackout(!blackout)}
        >
          {blackout ? 'Release blackout' : 'Blackout output'}
        </ActionButton>
      </section>
    </aside>
  );
};
