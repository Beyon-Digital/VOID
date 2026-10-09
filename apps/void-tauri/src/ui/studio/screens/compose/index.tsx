// Compose screen — piano roll + the proposal lifecycle
// (S02 ghost preview / S25 auditioning / S26 partial accept /
//  S03 accepted editable notes / S20 stale suggestion).
//
// The roll swaps to the suggestion variant whenever a live proposal
// record exists: real notes stay locked, ghost notes are the preview.
// Accepted notes return to the ordinary editable roll — they are
// regular document notes once committed (S03).

import * as React from 'react';
import { IconButton, tokens } from 'void-ui';
import { PianoRollPane, TrackListColumn } from '../../../shell';
import {
  makeViewKey,
  parseClipItem,
  parseNoteItem,
  studioStore,
  useStudio,
  NoteEditor,
  clipEndTicks,
  ghostNotesOf,
} from 'void-studio';
import type { ClipView } from 'void-studio';
import type { NoteView } from 'void-studio';
import type { GhostNote } from 'void-studio/src/proposals/ghost';
import { getClient } from '../../../client';
import { useEditor, useStudioCompactContext } from '../../useStudioData';
import { Eyebrow, ReasonNote } from '../../uip4/chrome';
import { PROPOSAL_OPS, REASONS } from '../../uip4/flags';
import {
  proposalsStore,
  useFeatureStores,
  useProposals,
} from '../../uip4/runtime';
import { SuggestionPanel } from '../../uip4/SuggestionPanel';
import { SuggestionRoll } from '../../uip4/SuggestionRoll';

let _noteEditor: NoteEditor | null = null;
const noteEd = () =>
  (_noteEditor ??= new NoteEditor(getClient(), () => crypto.randomUUID()));

/** The currently selected clip's ClipView (from CLIP_LIST cache). */
function useSelectedClip(): ClipView | null {
  const clipSel = useEditor((s) => s.clipSelection);
  const views = useStudio((s) => s.views);
  return React.useMemo(() => {
    if (!clipSel.trackId || clipSel.clipIds.length === 0) return null;
    const entry = views[makeViewKey('CLIP_LIST', clipSel.trackId)];
    for (const it of entry?.items ?? []) {
      const c = parseClipItem(it);
      if (c && c.clipId === clipSel.clipIds[0]) return c;
    }
    return null;
  }, [clipSel, views]);
}

/** NOTE_RANGE items for the selected clip (real notes). */
function useClipNotes(clip: ClipView | null): NoteView[] {
  const views = useStudio((s) => s.views);
  const attached = useStudio((s) => s.engine.attached);
  const key = clip
    ? `${clip.trackId}|${clip.clipId}|${clip.startTicks}|${clip.lengthTicks}`
    : '';
  React.useEffect(() => {
    if (!clip || !attached) return;
    void noteEd()
      .loadClipNotes(studioStore, clip)
      .catch(() => undefined);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, attached]);
  return React.useMemo(() => {
    if (!clip) return [];
    const entry =
      views[makeViewKey('NOTE_RANGE', clip.trackId, clip.startTicks, clipEndTicks(clip))];
    const out: NoteView[] = [];
    for (const it of entry?.items ?? []) {
      const n = parseNoteItem(it);
      if (n && n.clipId === clip.clipId) out.push(n);
    }
    return out;
  }, [clip, views]);
}

export default function ComposeScreen() {
  const compact = useStudioCompactContext();
  const [tracksOpen, setTracksOpen] = React.useState(false);
  const [panelOpen, setPanelOpen] = React.useState(false);
  useFeatureStores();
  const clip = useSelectedClip();
  const notes = useClipNotes(clip);

  // The proposal the inspector is showing: the selection's record,
  // else the newest record.
  const order = useProposals((s) => s.order);
  const records = useProposals((s) => s.records);
  const selection = useProposals((s) => s.selection);
  const ghostNotes = useProposals((s) => s.ghostNotes);
  const activeId =
    selection?.proposalId ?? order[order.length - 1] ?? undefined;
  const activeRec = activeId ? records[activeId] : undefined;

  const live =
    !!activeRec &&
    (activeRec.status === 'ready' ||
      activeRec.status === 'pending' ||
      activeRec.status === 'stale');

  // Ghost notes for the roll: store ghosts for ready records; a stale
  // record still paints its preview (locked, non-interactive — S20).
  const ghosts: GhostNote[] = React.useMemo(() => {
    if (!activeRec) return [];
    if (activeRec.status === 'ready') return ghostNotes;
    if (activeRec.status === 'stale' && selection === null) {
      const cand =
        activeRec.candidates.find(
          (c) => c.rank === activeRec.candidates[0]?.rank,
        ) ?? activeRec.candidates[0];
      if (!cand) return [];
      return ghostNotesOf(cand, activeRec.context?.lockedRanges ?? []).map(
        (g) => ({ ...g, locked: true }),
      );
    }
    return [];
  }, [activeRec, ghostNotes, selection]);

  const onToggleGhost = React.useCallback(
    (index: number) =>
      proposalsStore.getState().actions.toggleIndex(index),
    [],
  );

  const roll = live ? (
    <SuggestionRoll
      notes={notes}
      ghosts={ghosts}
      selectedIndices={selection?.indices ?? []}
      onToggleGhost={onToggleGhost}
      aria-label="Piano roll — suggestion preview"
    />
  ) : (
    <PianoRollPane />
  );

  const statusStrip = live && activeRec ? (
    <div
      style={{
        display: 'flex',
        alignItems: 'center',
        gap: 8,
        padding: '6px 10px',
        borderTop: `1px solid ${tokens.line}`,
        background: tokens.surface,
      }}
    >
      {activeRec.status === 'ready' ? (
        <>
          <Eyebrow>Preview only — no changes committed</Eyebrow>
          {!PROPOSAL_OPS.audition ? <ReasonNote>{REASONS.audition}</ReasonNote> : null}
        </>
      ) : activeRec.status === 'pending' ? (
        <Eyebrow>Generating…</Eyebrow>
      ) : (
        <Eyebrow>
          Stale preview — context changed; accept is refused
        </Eyebrow>
      )}
    </div>
  ) : null;

  const center = (
    <div
      style={{
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        display: 'flex',
        flexDirection: 'column',
      }}
    >
      {roll}
      {statusStrip}
    </div>
  );

  const inspector = (
    <aside
      aria-label="Suggestion"
      style={{
        width: 264,
        flexShrink: 0,
        borderLeft: `1px solid ${tokens.line}`,
        overflowY: 'auto',
        padding: '12px 10px',
        background: tokens.surface,
      }}
    >
      <SuggestionPanel proposalId={activeId} />
    </aside>
  );

  if (!compact) {
    return (
      <div style={{ display: 'flex', flex: 1, minWidth: 0, minHeight: 0 }}>
        <TrackListColumn />
        {center}
        {inspector}
      </div>
    );
  }

  return (
    <div
      style={{
        display: 'flex',
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        position: 'relative',
      }}
    >
      <div
        style={{
          position: 'absolute',
          top: 8,
          left: 8,
          right: 8,
          zIndex: 20,
          display: 'flex',
          justifyContent: 'space-between',
        }}
      >
        <IconButton
          icon="☰"
          aria-label="Open tracks drawer"
          variant="solid"
          pressed={tracksOpen}
          onClick={() => setTracksOpen((v) => !v)}
        />
        <IconButton
          icon="◧"
          aria-label="Open suggestion panel"
          variant="solid"
          pressed={panelOpen}
          onClick={() => setPanelOpen((v) => !v)}
        />
      </div>
      {center}
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
            inset: '0 auto 0 0',
            width: 260,
            zIndex: 30,
            background: tokens.surface,
            boxShadow: '4px 0 24px rgba(0,0,0,0.4)',
            display: 'flex',
          }}
        >
          <TrackListColumn />
        </div>
      ) : null}
      {panelOpen ? (
        <div
          role="dialog"
          aria-label="Suggestion"
          onKeyDown={(e) => {
            if (e.key === 'Escape') {
              e.stopPropagation();
              setPanelOpen(false);
            }
          }}
          style={{
            position: 'absolute',
            inset: '0 0 0 auto',
            width: 280,
            zIndex: 30,
            background: tokens.surface,
            boxShadow: '-4px 0 24px rgba(0,0,0,0.4)',
            overflowY: 'auto',
            padding: '12px 10px',
          }}
        >
          <SuggestionPanel proposalId={activeId} />
        </div>
      ) : null}
    </div>
  );
}
