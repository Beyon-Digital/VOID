// Dev component gallery — every Signal component family × every variant in
// BOTH themes, for screenshot verification. Dev route: #/dev/gallery.
// Renders real components only; the small demo values are static fixtures
// inside the gallery, not production UI.

import * as React from 'react';
import {
  ActionButton,
  AssetRow,
  ChannelFader,
  DeviceSlot,
  Field,
  IconButton,
  ParameterKnob,
  SceneCell,
  StatusBadge,
  TextInput,
  TimelineClip,
  Toast,
  TrackHeader,
  WorkspaceTab,
  VoidThemeProvider,
  tokens,
  type StatusBadgeStatus,
} from 'void-ui';
import type { SceneCellPhase } from 'void-ui';

const Row: React.FC<{ title: string; children: React.ReactNode }> = ({ title, children }) => (
  <section style={{ marginBottom: tokens.space24 }}>
    <h3
      className="void-type-label"
      style={{ color: tokens.subtle, margin: `0 0 ${tokens.space8}`, textTransform: 'uppercase' }}
    >
      {title}
    </h3>
    <div
      style={{
        display: 'flex',
        flexWrap: 'wrap',
        gap: tokens.space12,
        alignItems: 'center',
        position: 'relative',
        minHeight: 48,
      }}
    >
      {children}
    </div>
  </section>
);

function LiveKnob() {
  const [v, setV] = React.useState(0.4);
  return (
    <ParameterKnob value={v} min={0} max={1} unit="%" label="amount" onCommit={(tx) => setV(tx.to)} onPreview={setV} />
  );
}
function LiveFader() {
  const [v, setV] = React.useState(-6);
  return (
    <ChannelFader value={v} min={-60} max={6} unit="dB" label="ch 1" meter={0.6} onCommit={(tx) => setV(tx.to)} onPreview={setV} height={140} />
  );
}
function LiveEntry() {
  const [v, setV] = React.useState('Untitled');
  return <TextInput aria-label="project name" value={v} onChange={(e) => setV(e.target.value)} />;
}

const BADGES: StatusBadgeStatus[] = [
  'ready',
  'armed',
  'recording',
  'queued',
  'playing',
  'unavailable',
  'error',
  'saved',
  'dirty',
  'processing',
  'disconnected',
];

const SCENES: SceneCellPhase[] = ['stopped', 'pending', 'playing', 'stopping'];

function GalleryGrid() {
  return (
    <div style={{ fontFamily: tokens.sans }}>
      <Row title="ActionButton">
        <ActionButton variant="primary">Export</ActionButton>
        <ActionButton variant="secondary">Save</ActionButton>
        <ActionButton variant="ghost">Ghost</ActionButton>
        <ActionButton variant="danger">Delete</ActionButton>
        <ActionButton variant="subtle">Subtle</ActionButton>
        <ActionButton variant="primary" size="sm">Small</ActionButton>
        <ActionButton variant="primary" disabled>Disabled</ActionButton>
        <ActionButton variant="secondary" loading>Loading</ActionButton>
      </Row>
      <Row title="IconButton">
        <IconButton icon="▶" aria-label="Play" />
        <IconButton icon="◼" aria-label="Stop" variant="solid" />
        <IconButton icon="✕" aria-label="Delete" variant="danger" pressed />
        <IconButton icon="⚙" aria-label="Settings" size="sm" />
        <IconButton icon="＋" aria-label="Add" size="lg" pressed />
        <IconButton icon="?" aria-label="Help" disabled />
      </Row>
      <Row title="WorkspaceTab">
        <div role="tablist" aria-label="demo tabs" style={{ display: 'flex' }}>
          <WorkspaceTab tabId="a" title="Arrange" shortcut="⌃1" active onClick={() => undefined} />
          <WorkspaceTab tabId="c" title="Compose" shortcut="⌃2" onClick={() => undefined} />
          <WorkspaceTab tabId="p" title="Perform" badge={3} onClick={() => undefined} />
        </div>
      </Row>
      <Row title="StatusBadge">
        {BADGES.map((s) => (
          <StatusBadge key={s} status={s} />
        ))}
      </Row>
      <Row title="Field + TextInput">
        <Field label="Project name" hint="required" style={{ width: 220 }}>
          {(id) => <span id={id} style={{ display: 'contents' }}><LiveEntry /></span>}
        </Field>
        <Field label="Level" unit="dB" error="out of range" style={{ width: 200 }}>
          {(id) => <TextInput id={id} aria-label="level" defaultValue="-6.0" invalid />}
        </Field>
        <Field label="Inline field" inline style={{ width: 260 }}>
          {(id) => <TextInput id={id} aria-label="inline" defaultValue="value" />}
        </Field>
      </Row>
      <Row title="TrackHeader">
        <div style={{ display: 'flex', flexDirection: 'column', gap: 4, width: 224 }}>
          <TrackHeader trackId="t1" name="Drums" kind="audio" selected onSelect={() => undefined} />
          <TrackHeader trackId="t2" name="Bass" kind="midi" muted soloed={false} onSelect={() => undefined} />
          <TrackHeader trackId="t3" name="Vox" kind="audio" soloed onSelect={() => undefined} />
        </div>
      </Row>
      <Row title="TimelineClip (audio / midi / ghost / selected)">
        <div style={{ position: 'relative', width: 560, height: 48 }}>
          <TimelineClip clipId="c1" name="chorus.wav" variant="audio" x={0} w={160} height={44} />
          <TimelineClip clipId="c2" name="lead.mid" variant="midi" x={170} w={120} height={44} />
          <TimelineClip clipId="c3" name="proposed" variant="ghost" x={300} w={100} height={44} />
          <TimelineClip clipId="c4" name="kick.wav" variant="audio" selected x={410} w={140} height={44} onSelect={() => undefined} />
        </div>
      </Row>
      <Row title="ParameterKnob">
        <LiveKnob />
        <ParameterKnob value={0.75} min={0} max={1} label="mix" size={32} disabled={false} />
        <ParameterKnob value={2.4} min={-12} max={12} unit="st" label="pitch" onCommit={() => undefined} />
        <ParameterKnob value={0.5} min={0} max={1} label="disabled" disabled />
      </Row>
      <Row title="ChannelFader (continuous)">
        <LiveFader />
        <ChannelFader value={0} min={-60} max={6} unit="dB" label="unity" height={140} />
        <ChannelFader value={-24} min={-60} max={6} unit="dB" label="off" height={140} disabled />
      </Row>
      <Row title="DeviceSlot">
        <div style={{ display: 'flex', flexDirection: 'column', gap: 6, width: 280 }}>
          <DeviceSlot index={0} name="Serum 2" category="instrument" powered onSelect={() => undefined} onTogglePower={() => undefined} onRemove={() => undefined} />
          <DeviceSlot index={1} name="EQ Eight" category="effect" powered={false} onSelect={() => undefined} />
          <DeviceSlot index={2} name="OldPlugin.vst3" category="effect" failed onSelect={() => undefined} />
          <DeviceSlot index={3} onAdd={() => undefined} />
        </div>
      </Row>
      <Row title="AssetRow">
        <div style={{ display: 'flex', flexDirection: 'column', gap: 2, width: 320 }}>
          <AssetRow assetId="a1" name="kick_909.wav" kind="AUDIO" meta="0:01.2" onSelect={() => undefined} />
          <AssetRow assetId="a2" name="arp_line.mid" kind="MIDI" meta="8 bars" selected onSelect={() => undefined} />
          <AssetRow assetId="a3" name="vocal_take.flac" kind="AUDIO" meta="missing" missing onSelect={() => undefined} />
        </div>
      </Row>
      <Row title="SceneCell">
        <div style={{ display: 'grid', gridTemplateColumns: 'repeat(4, 96px)', gap: 8 }}>
          {SCENES.map((phase, i) => (
            <SceneCell
              key={phase}
              slotId={`s${i}`}
              sceneId={`sc${i}`}
              trackId="t1"
              contentKind={i === 3 ? 'empty' : 'clip'}
              label={phase}
              phase={phase}
            />
          ))}
        </div>
      </Row>
      <Row title="Toast">
        <Toast toast={{ id: 't1', variant: 'info', title: 'Project opened', message: 'demo.voidproj' }} onDismiss={() => undefined} />
        <Toast toast={{ id: 't2', variant: 'success', title: 'Saved locally' }} onDismiss={() => undefined} />
        <Toast toast={{ id: 't3', variant: 'warning', title: 'Queued', message: 'launch at next bar' }} onDismiss={() => undefined} />
        <Toast toast={{ id: 't4', variant: 'error', title: 'Save failed', message: 'disk full' }} onDismiss={() => undefined} />
      </Row>
    </div>
  );
}

export default function DevGalleryScreen() {
  return (
    <div style={{ flex: 1, display: 'flex', overflow: 'hidden' }}>
      {(['dark', 'daylight'] as const).map((theme) => (
        <VoidThemeProvider
          key={theme}
          theme={theme}
          style={{ flex: 1, overflowY: 'auto', padding: 20 }}
        >
          <h2 className="void-type-heading" style={{ margin: `0 0 ${tokens.space16}` }}>
            {theme}
          </h2>
          <GalleryGrid />
        </VoidThemeProvider>
      ))}
    </div>
  );
}
