// Arrange screen — timeline of tracks and clips (real read-view content).
// Compact policy (≤1280): the track column collapses into a drawer that
// opens over the timeline; the timeline (selection scope) stays visible.

import * as React from 'react';
import { IconButton, tokens } from 'void-ui';
import { TimelinePane, TrackListColumn } from '../../../shell';
import { useStudioCompactContext } from '../../useStudioData';

export default function ArrangeScreen() {
  const compact = useStudioCompactContext();
  const [tracksOpen, setTracksOpen] = React.useState(false);

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
