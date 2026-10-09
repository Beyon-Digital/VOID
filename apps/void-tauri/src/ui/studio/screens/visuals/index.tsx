// S09 — Visuals / preview and program (figma 4:808).
//
// Layer/timeline visual composition on void-studio/src/visuals +
// visual-gen + av. Preview and program are separate channels; the
// program output starts DISARMED and needs an explicit target pick +
// arm (UI-T23). Renderer failure is surfaced, never hidden (UI-T24).
// Native audio timing drives visual scheduling — the screen reads
// ClockSnapshot telemetry and never keeps its own scheduling clock.
//
// Wire reality of this build (all named honestly in the UI):
//   - voidvis ops are not in the shipped protocol → layer edits read-only
//   - no visual snapshot/frame feed → layers arrive when the feed lands
//   - display enumeration deferred → only the offscreen target is real
//   - submit_job not shipped → AV export validates real specs, cannot launch
//
// Compact (≤1280): library and inspector collapse into drawers toggled
// from the title row; monitors + timeline stay mounted.

import * as React from 'react';
import { ActionButton, IconButton, StatusBadge, tokens } from 'void-ui';
import { useStudio } from 'void-studio';
import { useStudioCompactContext } from '../../useStudioData';
import { AvExportDialog } from './AvExportDialog';
import { InspectorPane } from './InspectorPane';
import { LibraryColumn } from './LibraryColumn';
import { PreviewMonitor } from './PreviewMonitor';
import { ProgramMonitor } from './ProgramMonitor';
import { clockLine, deriveOutputView, outputStateLabel } from './model';
import { avPanelStore, outputIntentStore, useOutputIntent, useVisuals } from './stores';
import { VisualTimelinePane } from './VisualTimelinePane';

type BadgeStatus = 'ready' | 'armed' | 'playing' | 'unavailable' | 'error';

function outputBadge(state: string): { status: BadgeStatus; label: string } {
  switch (state) {
    case 'live':
      return { status: 'playing', label: 'Program live' };
    case 'armed':
      return { status: 'armed', label: 'Output armed' };
    case 'displaySelected':
      return { status: 'ready', label: 'Target selected' };
    case 'blackout':
      return { status: 'error', label: 'Blackout' };
    case 'unavailable':
      return { status: 'unavailable', label: 'Output unavailable' };
    default:
      return { status: 'ready', label: 'Preview only' };
  }
}

export default function VisualsScreen() {
  const compact = useStudioCompactContext();
  const attached = useStudio((s) => s.engine.attached);
  const clock = useStudio((s) => s.telemetry.clock);
  const intent = useOutputIntent((s) => s);
  const lastProgramFrame = useVisuals((s) => s.lastFrame.program);
  const [libOpen, setLibOpen] = React.useState(false);
  const [inspectorOpen, setInspectorOpen] = React.useState(false);

  const view = deriveOutputView({ attached, intent, lastProgramFrame });
  const badge = outputBadge(view.state);

  const titleRow = (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: tokens.space12,
        padding: `${tokens.space8} ${tokens.space12}`,
      }}
    >
      {compact ? (
        <IconButton
          icon="☰"
          aria-label="Open visual library drawer"
          variant="solid"
          size="sm"
          pressed={libOpen}
          onClick={() => setLibOpen((v) => !v)}
        />
      ) : null}
      <h1 style={{ margin: 0, fontFamily: tokens.fontDisplay, fontSize: 18, fontWeight: 600, color: tokens.text }}>
        Make the sound visible.
      </h1>
      <StatusBadge status={badge.status} label={badge.label} title={view.reason} />
      <div style={{ flex: 1 }} />
      {compact ? (
        <IconButton
          icon="⚙"
          aria-label="Open layer inspector drawer"
          variant="solid"
          size="sm"
          pressed={inspectorOpen}
          onClick={() => setInspectorOpen((v) => !v)}
        />
      ) : null}
    </div>
  );

  const center = (
    <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column', minHeight: 0 }}>
      {titleRow}
      <div
        style={{
          display: 'flex',
          gap: tokens.space12,
          padding: `0 ${tokens.space12}`,
          flex: '0 0 auto',
          minHeight: 280,
        }}
      >
        <PreviewMonitor />
        <ProgramMonitor />
      </div>
      <div style={{ padding: `${tokens.space12} ${tokens.space12} 0` }}>
        <VisualTimelinePane />
      </div>
      <div
        style={{
          display: 'flex',
          alignItems: 'center',
          gap: tokens.space12,
          padding: tokens.space12,
          marginTop: 'auto',
        }}
      >
        <span
          title="The native audio clock drives the visual schedule — never an independent JS clock."
          style={{ fontFamily: tokens.mono, fontSize: 10, color: tokens.textMuted }}
        >
          Audio clock → visual frames. Never the other way around. {clockLine(clock)}
        </span>
        <div style={{ flex: 1 }} />
        {/* UI-T23: blackout stays reachable in every layout. */}
        <ActionButton
          variant="danger"
          size="sm"
          aria-pressed={intent.blackout}
          onClick={() => outputIntentStore.getState().actions.setBlackout(!intent.blackout)}
        >
          {intent.blackout ? 'Release blackout' : 'Blackout output'}
        </ActionButton>
        <ActionButton
          variant="primary"
          size="sm"
          onClick={() => avPanelStore.getState().actions.setOpen(true)}
        >
          Export audio + visuals
        </ActionButton>
      </div>
      {view.state === 'blackout' ? (
        <p
          role="alert"
          style={{
            margin: `0 ${tokens.space12} ${tokens.space8}`,
            fontFamily: tokens.mono,
            fontSize: 10,
            color: tokens.danger,
          }}
        >
          {outputStateLabel('blackout')} — {view.reason}
        </p>
      ) : null}
    </div>
  );

  const drawer = (
    labelText: string,
    open: boolean,
    close: () => void,
    side: 'left' | 'right',
    child: React.ReactNode,
  ) =>
    open ? (
      <div
        role="dialog"
        aria-label={labelText}
        onKeyDown={(e) => {
          if (e.key === 'Escape') {
            e.stopPropagation();
            close();
          }
        }}
        style={{
          position: 'absolute',
          top: 0,
          bottom: 0,
          [side]: 0,
          zIndex: 30,
          display: 'flex',
          background: tokens.surface,
          [side === 'left' ? 'borderRight' : 'borderLeft']: `1px solid ${tokens.line}`,
          boxShadow: side === 'left' ? `4px 0 16px rgba(0,0,0,0.5)` : `-4px 0 16px rgba(0,0,0,0.5)`,
        }}
      >
        {child}
      </div>
    ) : null;

  return (
    <div style={{ display: 'flex', flex: 1, minWidth: 0, minHeight: 0, position: 'relative' }}>
      {compact ? (
        <>
          {center}
          {drawer('Visual library', libOpen, () => setLibOpen(false), 'left', <LibraryColumn />)}
          {drawer('Layer inspector', inspectorOpen, () => setInspectorOpen(false), 'right', <InspectorPane />)}
        </>
      ) : (
        <>
          <LibraryColumn />
          {center}
          <InspectorPane />
        </>
      )}
      <AvExportDialog />
    </div>
  );
}
