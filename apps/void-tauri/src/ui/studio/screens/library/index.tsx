// S16 Library / sounds and assets — browse installed assets and audition
// before inserting into an explicit target (design map §S16).
//
// Truth: ASSET_LIST is the only collection the engine exposes; instruments
// and presets come from void-studio's descriptor tables. Audition preview
// playback is not in the protocol — the details panel says so rather than
// faking a player. Insert paths are real ops only (loadInstrument on the
// selected track; InsertAudioClipOp for present audio assets).

import * as React from 'react';
import {
  ActionButton,
  AssetRow,
  StatusBadge,
  TextInput,
  injectVoidStyles,
  tokens,
} from 'void-ui';
import {
  describeReceiptError,
  editorStore,
  loadInstrument,
  loadViewPage,
  makeViewKey,
  receiptFailed,
  studioStore,
  useStudio,
  useStore,
} from 'void-studio';
import { getClient } from '../../../client';
import { useEditor, useStudioCompactContext } from '../../useStudioData';
import { instrumentBrowseStore } from '../arrange/data';
import {
  assetRows,
  filterRows,
  instrumentRows,
  provenanceOf,
  type LibCategory,
  type LibraryRow,
} from './model';

function navigate(id: string) {
  window.location.hash = `/${id}`;
}

const CATEGORIES: readonly { id: LibCategory; label: string }[] = [
  { id: 'all', label: 'All' },
  { id: 'instruments', label: 'Instruments' },
  { id: 'presets', label: 'Presets' },
  { id: 'audio', label: 'Audio' },
];

export default function LibraryScreen() {
  React.useEffect(() => injectVoidStyles(), []);
  const compact = useStudioCompactContext();
  const browse = instrumentBrowseStore();
  const browseState = useStore(browse, (s) => s);
  const attached = useStudio((s) => s.engine.attached);
  const views = useStudio((s) => s.views);
  const selection = useStudio((s) => s.selection);
  const clipSelTrackId = useEditor((s) => s.clipSelection.trackId);
  const trackEntry = useStudio((s) => s.views[makeViewKey('TRACK_LIST')]);
  const selectedTrackId = clipSelTrackId ?? selection.trackId;

  const [category, setCategory] = React.useState<LibCategory>('all');
  const [selectedId, setSelectedId] = React.useState<string | null>(null);
  const [busy, setBusy] = React.useState<string | null>(null);
  const [flash, setFlash] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (!attached) return;
    const client = getClient();
    void loadViewPage(studioStore, client, 'ASSET_LIST').catch(() => undefined);
    void loadViewPage(studioStore, client, 'TRACK_LIST').catch(() => undefined);
  }, [attached]);

  const assetEntry = views[makeViewKey('ASSET_LIST')];
  const rows = React.useMemo(
    () =>
      filterRows(
        [...instrumentRows(), ...assetRows(assetEntry?.items ?? [])],
        category,
        browseState.filterText,
      ),
    [category, browseState.filterText, assetEntry],
  );
  const selected = rows.find((r) => r.id === selectedId) ?? null;
  const targetTrack = React.useMemo(() => {
    const items = trackEntry?.items ?? [];
    for (const it of items) {
      try {
        const v = JSON.parse(it.summary_json) as Record<string, unknown>;
        const id = String(v.track_id ?? v.id ?? it.object_id);
        if (id === selectedTrackId) {
          return { id, name: String(v.name ?? id) };
        }
      } catch {
        /* unparseable row — not a track we can name */
      }
    }
    return selectedTrackId ? { id: selectedTrackId, name: selectedTrackId } : null;
  }, [trackEntry, selectedTrackId]);

  const loadOnTrack = (row: LibraryRow) => {
    if (!attached || !row.descriptor || !targetTrack) return;
    setBusy(row.id);
    setFlash(null);
    void loadInstrument(
      {
        client: getClient(),
        store: browse,
        refreshPlugins: () =>
          loadViewPage(studioStore, getClient(), 'PLUGIN_LIST', {
            trackId: targetTrack.id,
          }),
        transactionId: crypto.randomUUID(),
      },
      targetTrack.id,
      row.descriptor,
      crypto.randomUUID(),
      0,
      row.preset,
    )
      .then((outs) => {
        const failed = outs.find((o) => receiptFailed(o.receipt));
        if (failed) {
          editorStore
            .getState()
            .actions.setEditError(describeReceiptError(failed.receipt));
        } else {
          setFlash(`Loaded ${row.name} on ${targetTrack.name} — one undo reverts it.`);
        }
      })
      .catch((e) => editorStore.getState().actions.setEditError(String(e)))
      .finally(() => setBusy(null));
  };

  const placeClip = async (row: LibraryRow) => {
    if (!attached || !row.assetId || !targetTrack || row.missing) return;
    setBusy(row.id);
    setFlash(null);
    const tx = crypto.randomUUID();
    try {
      const insert = await getClient().sendCommand(
        {
          InsertAudioClipOp: {
            clip_id: crypto.randomUUID(),
            track_id: targetTrack.id,
            asset_id: row.assetId,
            start_ticks: '0',
            length_ticks: '3840000',
            offset_ticks: '0',
          },
        },
        { transactionId: tx },
      );
      if (insert.status !== 'APPLIED' && insert.status !== 'DUPLICATE') {
        setFlash(`clip insert rejected: ${insert.status}`);
      } else {
        setFlash(`Placed ${row.name} on ${targetTrack.name} — one undo reverts it.`);
      }
    } catch (e) {
      setFlash(String(e));
    } finally {
      setBusy(null);
    }
  };

  const statusOf = (r: LibraryRow) =>
    r.statusLabel === 'missing'
      ? 'error'
      : r.statusLabel === 'ready'
        ? 'ready'
        : 'unavailable';

  const table = (
    <section aria-label="Asset search results" style={{ flex: 1, minWidth: 0 }}>
      <div
        className="void-type-label"
        style={{ color: tokens.text, marginBottom: tokens.space8 }}
      >
        Find the sound in your head.
      </div>
      <div
        className="void-type-small"
        style={{ color: tokens.subtle, marginBottom: tokens.space12 }}
      >
        Installed sounds first. Import your own, or create something new.
      </div>
      <TextInput
        aria-label="Search library"
        placeholder="Search sounds…"
        value={browseState.filterText}
        onChange={(e) => browse.getState().actions.setFilter(e.target.value)}
      />
      <div
        role="group"
        aria-label="Asset filters"
        style={{
          display: 'flex',
          gap: tokens.space8,
          margin: `${tokens.space8} 0 ${tokens.space12}`,
          flexWrap: 'wrap',
        }}
      >
        {CATEGORIES.map((c) => (
          <ActionButton
            key={c.id}
            size="sm"
            variant={category === c.id ? 'primary' : 'subtle'}
            aria-pressed={category === c.id}
            onClick={() => setCategory(c.id)}
          >
            {c.label}
          </ActionButton>
        ))}
      </div>

      <div
        className="void-type-micro"
        role="row"
        aria-hidden
        style={{
          display: 'grid',
          gridTemplateColumns: '1fr 160px 120px 100px',
          gap: tokens.space8,
          color: tokens.subtle,
          padding: `0 ${tokens.space8} ${tokens.space8}`,
          borderBottom: `1px solid ${tokens.line}`,
        }}
      >
        <span>NAME</span>
        <span>TYPE</span>
        <span>SOURCE</span>
        <span>STATUS</span>
      </div>
      <div
        role="list"
        aria-label="Library assets"
        style={{ overflowY: 'auto', maxHeight: compact ? 420 : 560 }}
      >
        {rows.map((r) => (
          <div
            key={r.id}
            style={{
              display: 'grid',
              gridTemplateColumns: '1fr 160px 120px 100px',
              gap: tokens.space8,
              alignItems: 'center',
              padding: `${tokens.space4} ${tokens.space8}`,
            }}
          >
            <AssetRow
              assetId={r.id}
              name={r.name}
              kind={r.preset ? 'PRESET' : r.descriptor ? 'INSTRUMENT' : 'AUDIO'}
              missing={r.missing}
              selected={selectedId === r.id}
              onSelect={(id) => setSelectedId(id)}
              onActivate={(id) => {
                setSelectedId(id);
                const row = rows.find((x) => x.id === id);
                if (row?.descriptor) loadOnTrack(row);
                else if (row?.assetId) void placeClip(row);
              }}
            />
            <span className="void-type-micro" style={{ color: tokens.subtle }}>
              {r.typeLabel}
            </span>
            <span className="void-type-micro" style={{ color: tokens.subtle }}>
              {r.sourceLabel}
            </span>
            <StatusBadge status={statusOf(r)} label={r.statusLabel} />
          </div>
        ))}
        {rows.length === 0 ? (
          <div
            className="void-type-small"
            style={{ color: tokens.subtle, padding: tokens.space16 }}
          >
            {attached
              ? 'Nothing matches — the engine exposes no more assets.'
              : 'Engine not attached — installed instruments list once it connects.'}
          </div>
        ) : null}
      </div>
    </section>
  );

  const details = (
    <aside
      role="complementary"
      aria-label="Asset details"
      style={{
        width: compact ? '100%' : 264,
        flexShrink: 0,
        padding: tokens.space16,
        background: tokens.surface,
        borderLeft: compact ? 'none' : `1px solid ${tokens.line}`,
        borderTop: compact ? `1px solid ${tokens.line}` : 'none',
        display: 'flex',
        flexDirection: 'column',
        gap: tokens.space12,
      }}
    >
      <div className="void-type-micro" style={{ color: tokens.subtle }}>
        ASSET DETAILS
      </div>
      {selected ? (
        <>
          <StatusBadge status={statusOf(selected)} label={selected.statusLabel} />
          <div className="void-type-title" style={{ color: tokens.text }}>
            {selected.name}
          </div>
          <div className="void-type-small" style={{ color: tokens.subtle }}>
            {selected.typeLabel} · {selected.sourceLabel}
          </div>
          <div className="void-type-numeric" style={{ color: tokens.subtle }}>
            {provenanceOf(selected) ?? 'no content hash on this row'}
          </div>
          {selected.missing ? (
            <div className="void-type-small" style={{ color: tokens.danger }}>
              File missing on disk — the relink surface carries the expected
              hash; nothing is silently substituted.
            </div>
          ) : null}
          <div className="void-type-micro" style={{ color: tokens.subtle }}>
            Audition preview isn’t exposed by the engine — inserting writes the
            real instrument or clip onto the selected track.
          </div>
          {selected.descriptor ? (
            <ActionButton
              size="sm"
              variant="primary"
              disabled={!attached || !targetTrack || busy !== null}
              loading={busy === selected.id}
              onClick={() => loadOnTrack(selected)}
            >
              {targetTrack ? `Load on ${targetTrack.name}` : 'Select a track first'}
            </ActionButton>
          ) : null}
          {selected.assetId && !selected.missing ? (
            <ActionButton
              size="sm"
              variant="primary"
              disabled={!attached || !targetTrack || busy !== null}
              loading={busy === selected.id}
              onClick={() => void placeClip(selected)}
            >
              {targetTrack ? `Place clip on ${targetTrack.name}` : 'Select a track first'}
            </ActionButton>
          ) : null}
          {!targetTrack ? (
            <div className="void-type-micro" style={{ color: tokens.subtle }}>
              Pick a track in Arrange to enable insertion.
            </div>
          ) : null}
        </>
      ) : (
        <div className="void-type-small" style={{ color: tokens.subtle }}>
          Select an asset to see its provenance and insert target.
        </div>
      )}
      {flash ? (
        <div role="status" className="void-type-small" style={{ color: tokens.text }}>
          {flash}
        </div>
      ) : null}
      <div style={{ marginTop: 'auto', display: 'flex', flexDirection: 'column', gap: tokens.space8 }}>
        <ActionButton
          size="sm"
          variant="subtle"
          onClick={() => navigate('generate')}
        >
          Create something new
        </ActionButton>
        <ActionButton
          size="sm"
          variant="subtle"
          disabled
          title="Asset import is not exposed on this surface yet"
        >
          Import your own
        </ActionButton>
      </div>
    </aside>
  );

  return (
    <div
      style={{
        display: 'flex',
        flex: 1,
        minWidth: 0,
        minHeight: 0,
        flexDirection: compact ? 'column' : 'row',
        background: tokens.bg,
        overflowY: 'auto',
      }}
    >
      {table}
      {details}
    </div>
  );
}
