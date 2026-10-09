// Arrange screen — timeline of tracks and clips (real read-view content).
// Compact policy (≤1280): the track column collapses into a drawer that
// opens over the timeline; the timeline (selection scope) stays visible.

import * as React from 'react';
import { IconButton, tokens } from 'void-ui';
import { loadViewPage, makeViewKey, studioStore, useStudio } from 'void-studio';
import { TimelinePane, TrackListColumn } from '../../../shell';
import { getClient } from '../../../client';
import { useStudioCompactContext } from '../../useStudioData';
import { EmptyState } from './EmptyState';

export default function ArrangeScreen() {
  const compact = useStudioCompactContext();
  const [tracksOpen, setTracksOpen] = React.useState(false);
  const projectId = useStudio((s) => s.projectId);
  const attached = useStudio((s) => s.engine.attached);
  const trackEntry = useStudio((s) => s.views[makeViewKey('TRACK_LIST')]);

  // S27 gate: keep TRACK_LIST fresh; the empty state only renders once a
  // completed page proves the project has zero tracks — never on a guess.
  React.useEffect(() => {
    if (!projectId || !attached) return;
    void loadViewPage(studioStore, getClient(), 'TRACK_LIST').catch(() => undefined);
  }, [projectId, attached]);

  if (trackEntry?.done && trackEntry.items.length === 0) {
    return <EmptyState />;
  }

  if (!compact) {
    return (
      <div style={{ display: 'flex', flex: 1, minWidth: 0, minHeight: 0 }}>
        <TrackListColumn />
        <TimelinePane />
      </div>
    );
  }

  return (
    <div style={{ display: 'flex', flex: 1, minWidth: 0, minHeight: 0, position: 'relative' }}>
      <div style={{ position: 'absolute', top: 8, left: 8, zIndex: 20 }}>
        <IconButton
          icon="☰"
          aria-label="Open tracks drawer"
          variant="solid"
          pressed={tracksOpen}
          onClick={() => setTracksOpen((v) => !v)}
        />
      </div>
      <TimelinePane />
      {tracksOpen ? (
        <div
          role="dialog"
          aria-label="Tracks"
          onKeyDown={(e) => {
            if (e.key === 'Escape') {
              e.stopPropagation();
              setTracksOpen(false);
            }
          }}
          style={{
            position: 'absolute',
            top: 0,
            bottom: 0,
            left: 0,
            zIndex: 30,
            display: 'flex',
            background: tokens.surface,
            borderRight: `1px solid ${tokens.line}`,
            boxShadow: `4px 0 16px ${tokens.ink}`,
          }}
        >
          <TrackListColumn />
        </div>
      ) : null}
    </div>
  );
}
