// Comp screen — S06 audition + commit.
//
// Nondestructive comping on `void-studio/src/takes` (W17):
//   - take lanes render the folder's takes; a segment's take can be
//     picked (one row) or cycled (at a timeline position);
//   - "Commit comp" runs planCompApply + applyCompPlan: every Remove/Insert
//     op goes out under ONE transactionId, so undo is exactly
//     `client.undo(transactionId)` — one comp edit, one undo (UI-T11);
//   - source takes are NEVER mutated — applyCompPlan only emits
//     clip-level ops; the folder stays untouched;
//   - audition is display state: the screen marks the take under the
//     cursor and says so — a preview engine layer is NEEDS §12, not faked.
//
// Folders: TAKE_LIST (protocol minor 1) → the shared take store; rows
// the engine drops are removed, never kept as ghosts. When the view is
// empty the screen says exactly that instead of seeding demo takes.

import * as React from 'react';
import {
  ActionButton,
  StatusBadge,
  TakeLane,
  tokens,
  injectVoidStyles,
} from 'void-ui';
import {
  describeReceiptError,
  parseClipItem,
  planCompApply,
  applyCompPlan,
  receiptFailed,
  setSegmentTake,
  useStudio,
  useTakes,
  takeStudioStore,
  validateComp,
  CompApplyError,
  type ClipView,
  type CompSpec,
} from 'void-studio';
import { getClient } from '../../../client';
import { useStudioCompactContext } from '../../useStudioData';
import {
  loadAllPages,
  loadTakeFolders,
  parseTrackRow,
  type TrackRow,
} from '../shared/views';

const LANE_H = 40;

export default function CompScreen() {
  React.useEffect(() => injectVoidStyles(), []);
  const compact = useStudioCompactContext();
  const projectId = useStudio((s) => s.projectId);
  const revision = useStudio((s) => s.revision);
  const engineAttached = useStudio((s) => s.engine.attached);

  const folders = useTakes((s) => s.folders);
  const comps = useTakes((s) => s.comps);
  const openCompId = useTakes((s) => s.openCompId);
  const selection = useTakes((s) => s.selection);
  const auditionTakeId = useTakes((s) => s.auditionTakeId);
  const lastApply = useTakes((s) => s.lastApply);
  const lastError = useTakes((s) => s.lastError);
  const actions = useTakes((s) => s.actions);

  const [trackNames, setTrackNames] = React.useState<Record<string, string>>({});
  const [targetClips, setTargetClips] = React.useState<ClipView[]>([]);
  const [busy, setBusy] = React.useState(false);
  const [notice, setNotice] = React.useState('');
  const [appliedDetail, setAppliedDetail] = React.useState<string | null>(null);

  const openComp: CompSpec | undefined = openCompId ? comps[openCompId] : undefined;
  const openFolder = openComp
    ? Object.values(folders).find((f) => f.trackId === openComp.trackId)
    : undefined;

  // TAKE_LIST → shared take store (same source the record lanes use).
  React.useEffect(() => {
    if (!projectId || !engineAttached) return;
    void loadTakeFolders().catch(() => undefined);
  }, [projectId, engineAttached, revision]);

  // Track names for lane headers (TRACK_LIST).
  React.useEffect(() => {
    if (!projectId || !engineAttached) return;
    let cancelled = false;
    loadAllPages('TRACK_LIST')
      .then((p) => {
        if (cancelled) return;
        const m: Record<string, string> = {};
        for (const it of p.items) {
          const r = parseTrackRow(it);
          if (r) m[r.trackId] = r.name ?? r.trackId;
        }
        setTrackNames(m);
      })
      .catch(() => undefined);
    return () => {
      cancelled = true;
    };
  }, [projectId, engineAttached]);

  // The comp's timeline target = the clips overlapping its region on its
  // track — straight from CLIP_LIST, re-read before every commit.
  const refreshClips = React.useCallback(async () => {
    if (!openComp) return;
    const p = await loadAllPages('CLIP_LIST', { trackId: openComp.trackId });
    const regionStart = BigInt(openComp.regionStartTicks);
    const regionEnd = regionStart + BigInt(openComp.regionLengthTicks);
    setTargetClips(
      p.items
        .map(parseClipItem)
        .filter((c): c is ClipView => c !== null)
        .filter((c) => {
          const s = BigInt(c.startTicks);
          const e = s + BigInt(c.lengthTicks);
          return s < regionEnd && e > regionStart;
        }),
    );
  }, [openComp]);

  React.useEffect(() => {
    void refreshClips().catch(() => undefined);
  }, [refreshClips, revision]);

  const commit = async () => {
    if (!openComp || !openFolder) return;
    const problems = validateComp(openComp, openFolder);
    if (problems.length > 0) {
      actions.setError(problems.join('; '));
      return;
    }
    setBusy(true);
    setNotice('');
    setAppliedDetail(null);
    try {
      await refreshClips();
      const plan = planCompApply(
        openComp,
        openFolder,
        { clips: targetClips },
        () => crypto.randomUUID(),
      );
      const receipts = await applyCompPlan(getClient(), plan, refreshClips);
      actions.markApplied(openComp.compId, plan.transactionId);
      setAppliedDetail(
        `comp committed: ${plan.insertedClipIds.length} clip(s) in, ` +
          `${plan.removedClipIds.length} removed · ${plan.seamFades.length} seam fades` +
          (plan.midiSegmentClipIds.length > 0
            ? ` · ${plan.midiSegmentClipIds.length} MIDI segment(s) inserted as empty clips (note transfer has no wire op)`
            : '') +
          ` · ${receipts.length} ops · one undo`,
      );
      await refreshClips();
    } catch (e) {
      if (e instanceof CompApplyError) {
        const last = e.receipts[e.receipts.length - 1];
        actions.setError(
          last && receiptFailed(last) ? describeReceiptError(last) : e.message,
        );
      } else {
        actions.setError(String(e));
      }
    } finally {
      setBusy(false);
    }
  };

  const undo = async () => {
    if (!lastApply) return;
    setBusy(true);
    setNotice('');
    try {
      const r = await getClient().undo(lastApply.transactionId);
      if (receiptFailed(r)) {
        actions.setError(describeReceiptError(r));
      } else {
        setNotice('comp edit undone — takes were never modified');
        setAppliedDetail(null);
        await refreshClips();
      }
    } catch (e) {
      actions.setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  if (!projectId || !engineAttached) {
    return (
      <Centered>
        <p className="void-type-body" style={{ color: tokens.subtle, maxWidth: 420, textAlign: 'center' }}>
          {!projectId
            ? 'no project open — comping needs a loaded project'
            : 'engine detached — comp edits need the engine attached'}
        </p>
      </Centered>
    );
  }

  const folderList = Object.values(folders).sort((a, b) =>
    (trackNames[a.trackId] ?? a.trackId).localeCompare(trackNames[b.trackId] ?? b.trackId),
  );

  return (
    <div style={{ display: 'flex', flex: 1, minWidth: 0, minHeight: 0 }}>
      {/* left: folder/context rail */}
      {!compact && (
        <aside
          aria-label="Take folders"
          style={{
            width: 240,
            flexShrink: 0,
            borderRight: `1px solid ${tokens.line}`,
            padding: tokens.space16,
            display: 'flex',
            flexDirection: 'column',
            gap: tokens.space8,
            overflowY: 'auto',
          }}
        >
          <span className="void-type-title" style={{ color: tokens.text }}>
            Take folders
          </span>
          {folderList.length === 0 ? (
            <p className="void-type-small" style={{ color: tokens.subtle }}>
              No take folders in this project yet. Takes land here when loop
              recording produces multiple passes — the take-folder view is
              not on the wire (protocol major.1), so nothing is fabricated.
            </p>
          ) : (
            folderList.map((f) => (
              <button
                key={f.folderId}
                type="button"
                onClick={() => {
                  // Open the comp bound to this folder (or its first one).
                  const spec =
                    Object.values(comps).find((c) => c.trackId === f.trackId) ??
                    undefined;
                  if (spec) actions.openComp(spec.compId);
                  else actions.openComp(null);
                }}
                style={{
                  textAlign: 'left',
                  background: tokens.raised,
                  border: `1px solid ${tokens.line}`,
                  borderRadius: tokens.radius8,
                  padding: tokens.space8,
                  color: tokens.text,
                  cursor: 'pointer',
                  font: 'inherit',
                  fontSize: 12,
                }}
              >
                {trackNames[f.trackId] ?? f.trackId}
                <div className="void-type-small" style={{ color: tokens.subtle }}>
                  {f.takes.length} takes
                </div>
              </button>
            ))
          )}
          <p className="void-type-small" style={{ color: tokens.subtle, marginTop: 'auto' }}>
            Every take is kept. Comping never modifies source audio.
          </p>
        </aside>
      )}

      {/* main: comp editor */}
      <div style={{ flex: 1, minWidth: 0, display: 'flex', flexDirection: 'column' }}>
        <div
          style={{
            display: 'flex',
            alignItems: 'center',
            gap: tokens.space12,
            padding: `${tokens.space12} ${tokens.space16}`,
            borderBottom: `1px solid ${tokens.line}`,
          }}
        >
          <span className="void-type-title" style={{ color: tokens.text }}>
            Take the best of every take.
          </span>
          <span style={{ flex: 1 }} />
          {openComp ? (
            <>
              <ActionButton
                variant="primary"
                size="sm"
                loading={busy}
                onClick={() => void commit()}
              >
                Commit comp
              </ActionButton>
              <ActionButton
                variant="secondary"
                size="sm"
                disabled={!lastApply || lastApply.compId !== openComp.compId || busy}
                onClick={() => void undo()}
              >
                Undo comp
              </ActionButton>
            </>
          ) : null}
        </div>

        {!openComp || !openFolder ? (
          <Centered>
            <p className="void-type-body" style={{ color: tokens.subtle, maxWidth: 460, textAlign: 'center' }}>
              {folderList.length === 0
                ? 'Nothing to comp — this project has no take folders on the wire yet.'
                : 'Pick a take folder on the left to open its comp.'}
            </p>
          </Centered>
        ) : (
          <>
            {/* comp summary lane */}
            <div
              aria-label="Comp summary"
              style={{
                padding: `${tokens.space8} ${tokens.space16}`,
                borderBottom: `1px solid ${tokens.line}`,
              }}
            >
              <div className="void-type-small" style={{ color: tokens.subtle, marginBottom: 4 }}>
                Comp · {openComp.segments.length} chosen phrase{openComp.segments.length === 1 ? '' : 's'}
              </div>
              <div
                style={{
                  position: 'relative',
                  height: 22,
                  background: tokens.raised,
                  borderRadius: tokens.radius4,
                  overflow: 'hidden',
                }}
              >
                {openComp.segments.map((seg) => {
                  const total = BigInt(openComp.regionLengthTicks);
                  const start =
                    (Number(BigInt(seg.startTicks) - BigInt(openComp.regionStartTicks)) /
                      Number(total)) *
                    100;
                  const width = (Number(BigInt(seg.lengthTicks)) / Number(total)) * 100;
                  const lane = openFolder.takes.findIndex((t) => t.takeId === seg.takeId);
                  const sel = selection?.compId === openComp.compId && selection.segmentId === seg.segmentId;
                  return (
                    <button
                      key={seg.segmentId}
                      type="button"
                      onClick={() =>
                        actions.selectSegment(
                          openComp.compId,
                          sel ? null : seg.segmentId,
                        )
                      }
                      title={`segment from take ${lane >= 0 ? lane + 1 : '?'}`}
                      style={{
                        position: 'absolute',
                        left: `${start}%`,
                        width: `${width}%`,
                        top: 3,
                        bottom: 3,
                        background: sel ? tokens.accent : tokens.accentSoft,
                        border: sel ? `1px solid ${tokens.accent}` : `1px solid transparent`,
                        borderRadius: tokens.radius4,
                        cursor: 'pointer',
                      }}
                    />
                  );
                })}
              </div>
            </div>

            {/* take lanes */}
            <div style={{ flex: 1, overflowY: 'auto', position: 'relative', padding: tokens.space16 }}>
              {openFolder.takes
                .slice()
                .sort((a, b) => a.laneIndex - b.laneIndex)
                .map((take) => {
                  const regionLen = BigInt(openComp.regionLengthTicks);
                  const rel = BigInt(take.regionStartTicks) - BigInt(openComp.regionStartTicks);
                  const startFrac = Math.max(0, Number(rel) / Number(regionLen));
                  const widthFrac = Math.min(1 - startFrac, Number(take.regionLengthTicks) / Number(regionLen));
                  const comped = openComp.segments.some((s) => s.takeId === take.takeId);
                  const auditioning = auditionTakeId === take.takeId;
                  return (
                    <div key={take.takeId} style={{ position: 'relative', height: LANE_H, marginBottom: 4 }}>
                      <TakeLane
                        takeId={take.takeId}
                        laneIndex={take.laneIndex}
                        name={
                          `take ${take.laneIndex + 1}` +
                          (take.kind === 'MIDI' ? ' · midi' : '') +
                          (auditioning ? ' · auditioning' : '')
                        }
                        top={4}
                        height={LANE_H - 8}
                        startFrac={startFrac}
                        widthFrac={widthFrac}
                        incomplete={!take.complete}
                        comped={comped}
                        onPick={(takeId) => {
                          // Pick this take for the selected segment, if any.
                          const sel = takeStudioStore.getState().selection;
                          if (sel && sel.compId === openComp.compId) {
                            const spec = takeStudioStore.getState().comps[openComp.compId];
                            if (spec) {
                              actions.upsertComp(setSegmentTake(spec, sel.segmentId, takeId));
                              return;
                            }
                          }
                          actions.audition(auditioning ? null : takeId);
                        }}
                        onCycle={(takeId, dir) => {
                          const t = openFolder.takes.find((x) => x.takeId === takeId);
                          if (t) {
                            actions.cycleAt(
                              openComp.compId,
                              (BigInt(t.regionStartTicks) + BigInt(t.regionLengthTicks) / 2n).toString(),
                              dir,
                            );
                          }
                        }}
                        aria-label={`Take ${take.laneIndex + 1}`}
                      />
                    </div>
                  );
                })}
            </div>

            <footer
              style={{
                padding: `${tokens.space8} ${tokens.space16}`,
                borderTop: `1px solid ${tokens.line}`,
                display: 'flex',
                gap: tokens.space16,
                alignItems: 'center',
                flexWrap: 'wrap',
              }}
            >
              <span className="void-type-small" style={{ color: tokens.subtle }}>
                One comp edit. One undo. All {openFolder.takes.length} takes remain available.
              </span>
              {appliedDetail ? (
                <StatusBadge status="saved" label={appliedDetail} hideDot />
              ) : null}
              {notice ? (
                <span className="void-type-small" style={{ color: tokens.mint }}>{notice}</span>
              ) : null}
              {lastError ? (
                <span className="void-type-small" style={{ color: tokens.danger }}>{lastError}</span>
              ) : null}
              <span className="void-type-small" style={{ color: tokens.subtle }}>
                {targetClips.length} clip(s) overlap the comp region on this track
              </span>
            </footer>
          </>
        )}
      </div>
    </div>
  );
}

const Centered: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <div
    style={{
      flex: 1,
      display: 'flex',
      alignItems: 'center',
      justifyContent: 'center',
      padding: tokens.space24,
    }}
  >
    {children}
  </div>
);
