// SoundLibrary — S01 left rail (w224). Instruments + presets come from
// void-studio's descriptor tables; collections show only what the engine
// actually exposes (ASSET_LIST). Audition preview playback is NOT in the
// protocol — the card says so instead of faking it.

import * as React from 'react';
import { ActionButton, AssetRow, StatusBadge, TextInput, injectVoidStyles, tokens } from 'void-ui';
import {
  BUILTIN_INSTRUMENTS,
  BUILTIN_PRESETS,
  describeReceiptError,
  editorStore,
  loadInstrument,
  loadViewPage,
  makeViewKey,
  presetsFor,
  receiptFailed,
  studioStore,
  useStudio,
  useStore,
  type InstrumentDescriptor,
  type InstrumentPreset,
} from 'void-studio';
import { getClient } from '../../../client';
import { useEditor } from '../../useStudioData';
import {
  instrumentBrowseStore,
  newGesture,
  useTrackRows,
} from './data';

function useBrowse() {
  const store = instrumentBrowseStore();
  return {
    store,
    state: useStore(store, (s) => s),
  };
}

/** Rows: builtin instruments, then presets (grouped under their parent). */
interface LibRow {
  id: string;
  name: string;
  kind: string;
  meta?: string;
  descriptor?: InstrumentDescriptor;
  preset?: InstrumentPreset;
}

function buildRows(filter: string): LibRow[] {
  const q = filter.trim().toLowerCase();
  const rows: LibRow[] = [];
  for (const d of BUILTIN_INSTRUMENTS) {
    rows.push({
      id: `inst:${d.pluginUid}`,
      name: d.name,
      kind: 'INSTRUMENT',
      meta: `${d.params.length} params`,
      descriptor: d,
    });
    for (const p of presetsFor(d.pluginUid)) {
      rows.push({
        id: `preset:${p.id}`,
        name: p.name,
        kind: 'PRESET',
        meta: d.name,
        descriptor: d,
        preset: p,
      });
    }
  }
  return q ? rows.filter((r) => r.name.toLowerCase().includes(q)) : rows;
}

export const SoundLibrary: React.FC = () => {
  React.useEffect(() => injectVoidStyles(), []);
  const { store: browse, state } = useBrowse();
  const attached = useStudio((s) => s.engine.attached);
  const views = useStudio((s) => s.views);
  const selection = useStudio((s) => s.selection);
  const clipSelTrackId = useEditor((s) => s.clipSelection.trackId);
  const selectedTrackId = clipSelTrackId ?? selection.trackId;
  const { tracks } = useTrackRows();
  const [loading, setLoading] = React.useState<string | null>(null);

  // ASSET_LIST is the only collection the engine actually exposes.
  const assetEntry = views[makeViewKey('ASSET_LIST')];
  const assetCount = assetEntry ? assetEntry.items.length : null;
  React.useEffect(() => {
    if (!attached) return;
    void loadViewPage(studioStore, getClient(), 'ASSET_LIST').catch(() => undefined);
  }, [attached]);

  const rows = React.useMemo(() => buildRows(state.filterText), [state.filterText]);
  const selectedRow = rows.find((r) => r.id === state.browseUid) ?? null;
  const targetTrack = tracks.find((t) => t.id === selectedTrackId) ?? null;

  const load = (row: LibRow) => {
    if (!attached || !targetTrack || !row.descriptor) return;
    setLoading(row.id);
    const tx = newGesture();
    const refresh = () =>
      loadViewPage(studioStore, getClient(), 'PLUGIN_LIST', {
        trackId: targetTrack.id,
      });
    void loadInstrument(
      {
        client: getClient(),
        store: browse,
        refreshPlugins: refresh,
        transactionId: tx,
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
        }
      })
      .catch((e) => editorStore.getState().actions.setEditError(String(e)))
      .finally(() => setLoading(null));
  };

  return (
    <aside
      role="complementary"
      aria-label="Sound library"
      style={{
        width: 224,
        flexShrink: 0,
        display: 'flex',
        flexDirection: 'column',
        gap: tokens.space12,
        padding: tokens.space16,
        background: tokens.surface,
        borderRight: `1px solid ${tokens.line}`,
        overflowY: 'auto',
      }}
    >
      <div className="void-type-micro" style={{ color: tokens.subtle }}>
        SOUNDS
      </div>
      <TextInput
        aria-label="Search sounds"
        placeholder="Search sounds…"
        value={state.filterText}
        onChange={(e) => browse.getState().actions.setFilter(e.target.value)}
      />
      <div style={{ display: 'flex', gap: tokens.space8, alignItems: 'center' }}>
        <StatusBadge status="ready" label={`Installed · ${BUILTIN_INSTRUMENTS.length}`} />
        <span className="void-type-micro" style={{ color: tokens.subtle }}>
          {BUILTIN_PRESETS.length} presets
        </span>
      </div>

      <div className="void-type-micro" style={{ color: tokens.subtle }}>
        INSTRUMENTS
      </div>
      <div role="list" aria-label="Instruments and presets">
        {rows.map((r) => (
          <AssetRow
            key={r.id}
            assetId={r.id}
            name={r.name}
            kind={r.kind}
            meta={r.meta}
            selected={state.browseUid === r.id}
            onSelect={(id) => browse.getState().actions.setBrowseUid(id)}
            onActivate={(id) => {
              browse.getState().actions.setBrowseUid(id);
              const row = rows.find((x) => x.id === id);
              if (row) load(row);
            }}
            aria-label={`${r.name} — double-click to load on selected track`}
          />
        ))}
        {rows.length === 0 ? (
          <span className="void-type-micro" style={{ color: tokens.subtle }}>
            no instruments match the filter
          </span>
        ) : null}
      </div>

      <div
        aria-hidden
        style={{ height: 1, background: tokens.line, margin: `${tokens.space8} 0` }}
      />
      <div className="void-type-micro" style={{ color: tokens.subtle }}>
        COLLECTIONS
      </div>
      <div role="list" aria-label="Collections">
        <AssetRow
          assetId="collection:imported"
          name="Imported assets"
          kind="ASSETS"
          meta={assetCount === null ? '—' : `${assetCount}`}
          onSelect={() => undefined}
          aria-label={`Imported assets — ${assetCount === null ? 'count not loaded' : `${assetCount} assets`}`}
        />
        {(['Favorites', 'Recently used', 'My recordings'] as const).map((n) => (
          <div
            key={n}
            title="The engine does not expose this collection yet"
            style={{
              display: 'flex',
              alignItems: 'center',
              padding: `6px ${tokens.space8}`,
              color: tokens.subtle,
              fontSize: 12,
              fontFamily: tokens.sans,
            }}
          >
            <span style={{ flex: 1 }}>{n}</span>
            <span
              className="void-type-micro"
              style={{ color: tokens.subtle }}
            >
              n/a
            </span>
          </div>
        ))}
      </div>

      <ActionButton
        variant="subtle"
        size="sm"
        disabled
        title="Sound authoring ops are not in the command surface yet"
        style={{ justifyContent: 'flex-start' }}
      >
        + Create a sound
      </ActionButton>

      <div
        role="status"
        aria-label="Audition card"
        style={{
          marginTop: 'auto',
          padding: tokens.space12,
          background: tokens.accentSoft,
          borderRadius: tokens.radius8,
          display: 'flex',
          flexDirection: 'column',
          gap: tokens.space8,
        }}
      >
        <span className="void-type-micro" style={{ color: tokens.subtle }}>
          {selectedRow ? selectedRow.name.toUpperCase() : 'NO SOUND SELECTED'}
        </span>
        <span className="void-type-small" style={{ color: tokens.text }}>
          Audition on selection
        </span>
        <span className="void-type-micro" style={{ color: tokens.subtle }}>
          Preview playback isn’t exposed by the engine; loading writes the real
          instrument onto the selected track.
        </span>
        <ActionButton
          size="sm"
          variant="primary"
          disabled={
            !attached || !selectedRow || !targetTrack || loading !== null
          }
          loading={loading !== null}
          onClick={() => selectedRow && load(selectedRow)}
        >
          {targetTrack
            ? `Load on ${targetTrack.name}`
            : 'Select a track to load'}
        </ActionButton>
      </div>
    </aside>
  );
};
