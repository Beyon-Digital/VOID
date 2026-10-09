// S09 left column — visual assets + layer list.
//
// VISUAL ASSETS: real rows from the ASSET_LIST read view filtered to
// image/video media types. Selecting an asset previews it as a layer only
// when the visual command channel exists — today the honest note names
// the gap instead of pretending a stack edit happened.

import * as React from 'react';
import { ActionButton, AssetRow, tokens } from 'void-ui';
import {
  orderedChannelLayers,
  visualAssetRows,
  REASON_NO_CHANNEL,
  VISUAL_COMMAND_CHANNEL,
} from './model';
import { useAssetListLoaded, useVisuals, visualsStore } from './stores';
import { getClient } from '../../../client';
import { TextInput } from 'void-ui';

const label: React.CSSProperties = {
  fontFamily: tokens.mono,
  fontSize: 10,
  letterSpacing: '0.14em',
  textTransform: 'uppercase',
  color: tokens.textMuted,
};

const divider: React.CSSProperties = { height: 1, background: tokens.line, margin: `${tokens.space8} 0` };

export const LibraryColumn: React.FC = () => {
  const { items, attached, loaded } = useAssetListLoaded();
  const layers = useVisuals((s) => s.layers);
  const order = useVisuals((s) => s.order);
  const selected = useVisuals((s) => s.selectedLayerIds);
  const focusChannel = useVisuals((s) => s.focusedChannel);

  const [note, setNote] = React.useState<string | null>(null);
  const [importOpen, setImportOpen] = React.useState(false);
  const [importPath, setImportPath] = React.useState('');
  const [importBusy, setImportBusy] = React.useState(false);

  /** rev-2 (NEEDS §17): IngestAssetOp stages a host-side visual file
   * into the container; the row appears when ASSET_LIST repaints. */
  const importVisual = async () => {
    const rel = importPath.trim();
    if (!rel) return;
    setImportBusy(true);
    setNote(null);
    try {
      const ext = rel.split('.').pop()?.toLowerCase() ?? '';
      const r = await getClient().sendCommand({
        IngestAssetOp: {
          rel_path: rel,
          media_type: ext === 'png' || ext === 'jpg' || ext === 'jpeg' || ext === 'webp'
            ? 'image'
            : ext === 'mp4' || ext === 'mov' || ext === 'webm'
              ? 'video'
              : 'video',
        },
      });
      if (r.status === 'REJECTED') {
        setNote(`import rejected: ${r.message || r.error || 'no detail'}`);
      } else {
        setNote('ingest sent — the asset appears once the coordinator stages it');
        setImportPath('');
        setImportOpen(false);
      }
    } catch (e) {
      setNote(String(e instanceof Error ? e.message : e));
    } finally {
      setImportBusy(false);
    }
  };
  const assets = React.useMemo(() => visualAssetRows(items), [items]);
  const allLayers = React.useMemo(
    () => [...orderedChannelLayers({ layers, order }, 'preview'), ...orderedChannelLayers({ layers, order }, 'program')],
    [layers, order],
  );

  return (
    <nav aria-label="Visual assets and layers" style={{
      width: 224,
      flex: '0 0 auto',
      display: 'flex',
      flexDirection: 'column',
      borderRight: `1px solid ${tokens.line}`,
      background: tokens.surface,
      minHeight: 0,
      overflow: 'hidden',
    }}>
      <div style={{ padding: `${tokens.space12} ${tokens.space12} ${tokens.space8}` }}>
        <span style={label}>Visual assets</span>
        <div role="list" aria-label="Visual assets" style={{ marginTop: tokens.space8, display: 'flex', flexDirection: 'column', gap: 2 }}>
          {!attached ? (
            <p style={{ ...label, textTransform: 'none', letterSpacing: 0, margin: 0 }}>
              engine detached — asset list unavailable
            </p>
          ) : !loaded ? (
            <p style={{ ...label, textTransform: 'none', letterSpacing: 0, margin: 0 }}>reading assets…</p>
          ) : assets.length === 0 ? (
            <p style={{ ...label, textTransform: 'none', letterSpacing: 0, margin: 0 }}>
              no image or video assets in this project
            </p>
          ) : (
            assets.map((a) => (
              <AssetRow
                key={a.objectId}
                assetId={a.objectId}
                name={a.displayName}
                kind={(a.mediaType ?? 'asset').toUpperCase()}
                missing={a.state === 'missing'}
                meta={a.assetId ? `id ${a.assetId.slice(0, 8)}` : undefined}
                aria-label={`Visual asset ${a.displayName}`}
                onActivate={() =>
                  // Attaching an asset to the layer stack is a real action
                  // on the draft visual wire — the shipped protocol has no
                  // voidvis op, so the honest answer names the gap.
                  setNote(`attach to layer unavailable — ${REASON_NO_CHANNEL}`)
                }
              />
            ))
          )}
        </div>
        {note ? (
          <p role="status" style={{ ...label, textTransform: 'none', letterSpacing: 0, marginTop: tokens.space8 }}>
            {note}
          </p>
        ) : null}
      </div>

      <div style={divider} />

      <div style={{ padding: `0 ${tokens.space12}`, flex: 1, minHeight: 0, overflow: 'auto' }}>
        <span style={label}>Layers</span>
        <div role="listbox" aria-label="Visual layers" style={{ marginTop: tokens.space8, display: 'flex', flexDirection: 'column', gap: 2 }}>
          {allLayers.length === 0 ? (
            <p style={{ ...label, textTransform: 'none', letterSpacing: 0, margin: 0 }}>
              no visual layers — the scene is empty
            </p>
          ) : (
            allLayers.map((l) => (
              <button
                key={l.layerId}
                role="option"
                aria-selected={selected.includes(l.layerId)}
                onClick={() => {
                  visualsStore.getState().actions.selectLayers([l.layerId]);
                  visualsStore.getState().actions.focusChannel(l.channel);
                }}
                style={{
                  display: 'flex',
                  alignItems: 'center',
                  gap: tokens.space8,
                  padding: `5px ${tokens.space8}`,
                  borderRadius: tokens.radius8,
                  border: `1px solid ${selected.includes(l.layerId) ? tokens.accent : 'transparent'}`,
                  background: selected.includes(l.layerId) ? tokens.accentSoft : 'transparent',
                  color: l.visible ? tokens.text : tokens.textMuted,
                  fontFamily: tokens.sans,
                  fontSize: 12,
                  textAlign: 'left',
                  cursor: 'pointer',
                }}
              >
                <span
                  aria-label={l.visible ? 'visible' : 'hidden'}
                  title={VISUAL_COMMAND_CHANNEL ? 'toggle visibility' : `visibility read-only — ${REASON_NO_CHANNEL}`}
                  style={{ opacity: l.visible ? 1 : 0.35 }}
                >
                  ●
                </span>
                <span style={{ flex: 1, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  {l.name}
                </span>
                <span style={{ ...label, textTransform: 'uppercase' }}>
                  {l.channel === 'preview' ? 'PVW' : 'PGM'}
                </span>
              </button>
            ))
          )}
        </div>
      </div>

      <div style={{ padding: tokens.space12, display: 'flex', flexDirection: 'column', gap: tokens.space8 }}>
        <ActionButton
          variant="secondary"
          size="sm"
          disabled={!attached}
          title={attached ? 'IngestAssetOp — stage a host file into the container' : 'engine detached'}
          onClick={() => setImportOpen((v) => !v)}
        >
          Import visual
        </ActionButton>
        {importOpen ? (
          <div style={{ display: 'flex', flexDirection: 'column', gap: 4 }}>
            <TextInput
              id="vis-import-path"
              value={importPath}
              onChange={(ev) => setImportPath(ev.target.value)}
              placeholder="/path/to/visual.mp4"
              aria-label="Visual file path"
            />
            <ActionButton
              variant="primary"
              size="sm"
              disabled={importPath.trim() === '' || importBusy}
              onClick={() => void importVisual()}
            >
              Ingest file
            </ActionButton>
          </div>
        ) : null}
        <ActionButton
          variant="ghost"
          size="sm"
          onClick={() => {
            try {
              window.location.hash = '/arrange';
            } catch {
              /* no DOM */
            }
          }}
        >
          ← Back to song
        </ActionButton>
        <span style={{ ...label, letterSpacing: 0, textTransform: 'none' }}>
          focus: {focusChannel}
        </span>
      </div>
    </nav>
  );
};
