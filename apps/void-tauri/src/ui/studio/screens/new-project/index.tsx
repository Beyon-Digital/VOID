// S14 New project / starting point (Figma 4:1308).
//
// Real data: BUILTIN_TEMPLATES from void-studio/src/templates; create runs
// the real one-transaction applyProjectTemplate (create → attach → open in
// one transactionId) then records the reopen token. Readiness card reads
// real engine attach state — never claims devices that were not reported.

import * as React from 'react';
import { ActionButton, Field, StatusBadge, TemplateCard, TextInput, tokens } from 'void-ui';
import { BUILTIN_TEMPLATES, createTranslator, en, useStudio } from 'void-studio';
import { createProjectFromTemplate, slugifyName } from '../projects/lifecycle';

const t = createTranslator(en);

function navigate(id: string) {
  window.location.hash = `/${id}`;
}

const cardLabel: React.CSSProperties = {
  fontSize: 11,
  fontWeight: 500,
  letterSpacing: '0.08em',
  textTransform: 'uppercase',
  color: tokens.subtle,
  margin: '0 0 10px',
};

export default function NewProjectScreen() {
  const attached = useStudio((s) => s.engine.attached);
  const clock = useStudio((s) => s.telemetry.clock);
  const [selectedId, setSelectedId] = React.useState(BUILTIN_TEMPLATES[0]?.id ?? '');
  const [name, setName] = React.useState('');
  const [dir, setDir] = React.useState('');
  const [bpm, setBpm] = React.useState('');
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState('');

  const tpl = BUILTIN_TEMPLATES.find((x) => x.id === selectedId) ?? BUILTIN_TEMPLATES[0];
  const effectiveBpm = bpm.trim() !== '' && Number.isFinite(Number(bpm)) && Number(bpm) > 0 ? Number(bpm) : null;

  const create = async () => {
    if (!tpl) return;
    setBusy(true);
    setError('');
    const projectName = name.trim() || t(tpl.nameKey);
    const containerDir = dir.trim() || `./projects/${slugifyName(projectName)}.void`;
    // One transaction, still the real apply path — tempo is the only field
    // the form overrides, via the template descriptor itself.
    const effective = effectiveBpm !== null && effectiveBpm !== tpl.initialBpm ? { ...tpl, initialBpm: effectiveBpm } : tpl;
    const r = await createProjectFromTemplate({ template: effective, name: projectName, containerDir });
    setBusy(false);
    if (!r.ok) setError(r.error ?? 'create failed');
    else navigate('arrange');
  };

  const readiness: { label: string; detail: string }[] = [
    attached
      ? {
          label: 'Audio output ready',
          detail:
            clock && clock.sample_rate
              ? `Engine attached · ${clock.sample_rate / 1000} kHz`
              : 'Engine attached',
        }
      : {
          label: 'No audio engine attached',
          detail: 'Create is unavailable until an engine is running — check the dev engine panel.',
        },
    {
      label: 'Input permission optional',
      detail: 'Required only when recording a microphone',
    },
    {
      label: 'AI is optional',
      detail: 'Manual composition is always available. No samples or models download automatically.',
    },
  ];

  return (
    <div style={{ flex: 1, minWidth: 0, overflowY: 'auto', padding: tokens.space24 }}>
      <header style={{ marginBottom: tokens.space24 }}>
        <p style={cardLabel}>New project</p>
        <h1 style={{ margin: 0, fontSize: 28, fontWeight: 700, color: tokens.text, fontFamily: tokens.fontDisplay }}>
          Start with your first move.
        </h1>
        <p style={{ margin: `${tokens.space8} 0 0`, color: tokens.muted, fontSize: 13 }}>
          Choose a starting point. You can add every other instrument and track later.
        </p>
      </header>

      <div style={{ display: 'flex', gap: tokens.space24, alignItems: 'flex-start', flexWrap: 'wrap' }}>
        {/* Starting points */}
        <section aria-label="Starting points" style={{ width: 300, flexShrink: 0 }}>
          <p style={cardLabel}>Starting points</p>
          <div role="list" style={{ display: 'flex', flexDirection: 'column', gap: tokens.space8 }}>
            {BUILTIN_TEMPLATES.map((x) => (
              <TemplateCard
                key={x.id}
                name={t(x.nameKey)}
                description={t(x.descriptionKey)}
                specLine={`${x.tracks.length} tracks · ${x.initialBpm} BPM · ${x.sampleRate / 1000} kHz${
                  x.timeSignature ? ` · ${x.timeSignature.numerator}/${x.timeSignature.denominator}` : ''
                }`}
                selected={x.id === selectedId}
                busy={busy && x.id === selectedId}
                disabled={busy}
                actionLabel="Create"
                onSelect={() => setSelectedId(x.id)}
                onUse={() => {
                  setSelectedId(x.id);
                  void create();
                }}
              />
            ))}
          </div>
        </section>

        {/* Project fields */}
        <section aria-label="Project fields" style={{ flex: 1, minWidth: 280 }}>
          <p style={cardLabel}>Project identity</p>
          <div style={{ display: 'flex', flexDirection: 'column', gap: tokens.space16, maxWidth: 420 }}>
            <Field label="Project name">
              {(id) => (
                <TextInput id={id} value={name} onChange={(e) => setName(e.target.value)} placeholder="untitled" />
              )}
            </Field>
            <Field label="Save location" hint="container directory on this device">
              {(id) => (
                <TextInput
                  id={id}
                  value={dir}
                  onChange={(e) => setDir(e.target.value)}
                  placeholder={`./projects/${slugifyName(name || 'untitled')}.void`}
                />
              )}
            </Field>
            <div style={{ display: 'flex', gap: tokens.space12 }}>
              <Field label="Tempo" hint={`BPM — template default ${tpl?.initialBpm ?? '—'}`} style={{ flex: 1 }}>
                {(id) => (
                  <TextInput
                    id={id}
                    value={bpm}
                    onChange={(e) => setBpm(e.target.value)}
                    placeholder={tpl ? String(tpl.initialBpm) : ''}
                    inputMode="decimal"
                  />
                )}
              </Field>
              <Field label="Time signature" style={{ flex: 1 }}>
                {(id) => (
                  <TextInput
                    id={id}
                    value={tpl?.timeSignature ? `${tpl.timeSignature.numerator} / ${tpl.timeSignature.denominator}` : '4 / 4'}
                    readOnly
                    disabled
                    title="from the selected template"
                  />
                )}
              </Field>
            </div>
            <p style={{ margin: 0, fontSize: 11, color: tokens.subtle }}>
              Start composing immediately. Connect hardware only for the workflows that need it.
            </p>
            <div style={{ display: 'flex', gap: tokens.space8 }}>
              <ActionButton
                variant="primary"
                loading={busy}
                disabled={!attached || busy || !tpl}
                title={attached ? undefined : 'engine not attached'}
                onClick={() => void create()}
              >
                Create project
              </ActionButton>
              <ActionButton variant="ghost" disabled={busy} onClick={() => navigate('projects')}>
                Cancel
              </ActionButton>
            </div>
            {error ? (
              <p role="alert" style={{ margin: 0, fontSize: 12, color: tokens.danger }}>
                {error}
              </p>
            ) : null}
          </div>
        </section>

        {/* Readiness */}
        <aside aria-label="Readiness" style={{ width: 260, flexShrink: 0 }}>
          <p style={cardLabel}>Readiness</p>
          <div
            style={{
              display: 'flex',
              flexDirection: 'column',
              gap: tokens.space12,
              padding: tokens.space16,
              borderRadius: tokens.radius8,
              border: `1px solid ${tokens.line}`,
              background: tokens.surface,
            }}
          >
            <StatusBadge status={attached ? 'ready' : 'unavailable'} label={attached ? 'Your studio is ready' : 'No engine'} />
            {readiness.map((r) => (
              <div key={r.label}>
                <p style={{ margin: 0, fontSize: 13, fontWeight: 600, color: tokens.text }}>{r.label}</p>
                <p style={{ margin: '2px 0 0', fontSize: 11, color: tokens.muted }}>{r.detail}</p>
              </div>
            ))}
            <ActionButton variant="ghost" size="sm" onClick={() => navigate('setup')}>
              Configure audio & MIDI
            </ActionButton>
          </div>
        </aside>
      </div>
    </div>
  );
}
