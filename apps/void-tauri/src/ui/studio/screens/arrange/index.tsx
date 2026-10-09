// Arrange / song workspace — S01 (1600×1000), S22 (≤1280 compact: library
// + inspector become drawers, transport & selection stay visible), S23
// (daylight — every color below flows through void-ui tokens).
//
// Data truth: TRACK_LIST/CLIP_LIST/PLUGIN_LIST/ASSET_LIST read views and
// the command surface (ClipEditor/NoteEditor/loadInstrument) — never
// seeded fixtures. Panels and viewport are view state in studioStore.

import * as React from 'react';
import { globalEscapeStack, injectVoidStyles, tokens } from 'void-ui';
import {
  editorStore,
  loadViewPage,
  makeViewKey,
  studioStore,
  useStudio,
} from 'void-studio';
import { getClient } from '../../../client';
import { useStudioCompactContext } from '../../useStudioData';
import { EmptyState } from './EmptyState';
import { SoundLibrary } from './SoundLibrary';
import { ArrangementPane } from './ArrangementPane';
import { RegionInspector } from './RegionInspector';

function Drawer(props: {
  side: 'left' | 'right';
  label: string;
  onClose: () => void;
  children: React.ReactNode;
}) {
  const { onClose } = props;
  // Register on the shared escape stack so Escape closes the drawer even when
  // focus is outside it (the shell consumes Escape before other handlers).
  React.useEffect(() => globalEscapeStack.push(onClose), [onClose]);
  return (
    <div
      role="dialog"
      aria-label={props.label}
      aria-modal="false"
      onKeyDown={(e) => {
        if (e.key === 'Escape') {
          e.stopPropagation();
          props.onClose();
        }
      }}
      style={{
        position: 'absolute',
        top: 0,
        bottom: 0,
        [props.side]: 0,
        zIndex: 30,
        display: 'flex',
        boxShadow:
          props.side === 'left'
            ? '8px 0 24px rgba(0,0,0,0.4)'
            : '-8px 0 24px rgba(0,0,0,0.4)',
      }}
    >
      {props.children}
      <button
        type="button"
        aria-label={`Close ${props.label}`}
        onClick={props.onClose}
        style={{
          position: 'absolute',
          top: 8,
          [props.side === 'left' ? 'right' : 'left']: 8,
          width: 24,
          height: 24,
          borderRadius: 4,
          border: `1px solid ${tokens.line}`,
          background: tokens.raised,
          color: tokens.text,
          cursor: 'pointer',
        }}
      >
        ×
      </button>
    </div>
  );
}

export default function ArrangeScreen() {
  React.useEffect(() => injectVoidStyles(), []);
  const compact = useStudioCompactContext();
  const attached = useStudio((s) => s.engine.attached);
  const projectId = useStudio((s) => s.projectId);
  const trackEntry = useStudio((s) => s.views[makeViewKey('TRACK_LIST')]);
  const [drawer, setDrawer] = React.useState<'library' | 'inspector' | null>(
    null,
  );

  // Prime the track list when the engine attaches / project changes.
  React.useEffect(() => {
    if (!attached) return;
    void loadViewPage(studioStore, getClient(), 'TRACK_LIST').catch((e) =>
      editorStore.getState().actions.setEditError(String(e)),
    );
  }, [attached, projectId]);

  // S27 gate: the empty state only renders once a completed TRACK_LIST page
  // proves the project has zero tracks — never on a guess.
  if (attached && trackEntry?.done && trackEntry.items.length === 0) {
    return <EmptyState />;
  }

  return (
    <div
      style={{
        display: 'flex',
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        position: 'relative',
        background: tokens.bg,
      }}
    >
      {!compact ? <SoundLibrary /> : null}
      <ArrangementPane
        compact={compact}
        onOpenLibrary={() => setDrawer((d) => (d === 'library' ? null : 'library'))}
        onOpenInspector={() =>
          setDrawer((d) => (d === 'inspector' ? null : 'inspector'))
        }
      />
      {!compact ? <RegionInspector /> : null}

      {compact && drawer === 'library' ? (
        <Drawer side="left" label="Sound library" onClose={() => setDrawer(null)}>
          <SoundLibrary />
        </Drawer>
      ) : null}
      {compact && drawer === 'inspector' ? (
        <Drawer
          side="right"
          label="Region inspector"
          onClose={() => setDrawer(null)}
        >
          <RegionInspector />
        </Drawer>
      ) : null}
    </div>
  );
}
