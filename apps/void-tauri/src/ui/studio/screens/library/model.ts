// S16 Library — pure row derivation (node-testable, no React).
// Installed instruments/presets come from void-studio descriptor tables;
// collections come from the engine's ASSET_LIST read view (parsed by
// relink/model). Audition preview is NOT exposed by the engine — the
// screen reports that honestly instead of faking a player.

import type { ReadItem } from 'void-client';
import {
  BUILTIN_INSTRUMENTS,
  BUILTIN_PRESETS,
  parseAssetItem,
  presetsFor,
  type InstrumentDescriptor,
  type InstrumentPreset,
} from 'void-studio';

export type LibCategory = 'all' | 'instruments' | 'presets' | 'audio';

export interface LibraryRow {
  id: string;
  name: string;
  /** Table TYPE column — playable MIDI / preset / media kind. */
  typeLabel: string;
  /** Table SOURCE column — Built-in / Imported / Generated / —. */
  sourceLabel: string;
  /** Table STATUS column — ready / missing / —. */
  statusLabel: 'ready' | 'missing' | 'unknown';
  /** Instrument path — loadable via loadInstrument. */
  descriptor?: InstrumentDescriptor;
  preset?: InstrumentPreset;
  /** Engine asset path — insertable via InsertAudioClipOp when assetId exists. */
  assetId?: string;
  assetObjectId?: string;
  assetSha256?: string;
  mediaType?: string;
  missing?: boolean;
}

export function instrumentRows(): LibraryRow[] {
  const rows: LibraryRow[] = [];
  for (const d of BUILTIN_INSTRUMENTS) {
    rows.push({
      id: `inst:${d.pluginUid}`,
      name: d.name,
      typeLabel: `Playable · MIDI`,
      sourceLabel: 'Built-in',
      statusLabel: 'ready',
      descriptor: d,
    });
    for (const p of presetsFor(d.pluginUid)) {
      rows.push({
        id: `preset:${p.id}`,
        name: p.name,
        typeLabel: `${d.name} preset`,
        sourceLabel: 'Built-in',
        statusLabel: 'ready',
        descriptor: d,
        preset: p,
      });
    }
  }
  return rows;
}

/** ASSET_LIST items → rows. Unparseable items are skipped, never forced. */
export function assetRows(items: readonly ReadItem[]): LibraryRow[] {
  const rows: LibraryRow[] = [];
  for (const it of items) {
    const a = parseAssetItem(it);
    if (!a) continue;
    const missing = a.state === 'missing';
    rows.push({
      id: `asset:${a.objectId}`,
      name: a.displayName,
      typeLabel: a.mediaType ? `Audio · ${a.mediaType}` : 'Audio',
      sourceLabel: 'Imported',
      statusLabel: missing ? 'missing' : 'ready',
      assetId: a.assetId,
      assetObjectId: a.objectId,
      assetSha256: a.expectedSha256,
      mediaType: a.mediaType,
      missing,
    });
  }
  return rows;
}

export function filterRows(
  rows: readonly LibraryRow[],
  category: LibCategory,
  query: string,
): LibraryRow[] {
  const q = query.trim().toLowerCase();
  return rows.filter((r) => {
    if (category === 'instruments' && r.descriptor === undefined) return false;
    if (category === 'presets' && r.preset === undefined) return false;
    if (category === 'audio' && r.assetObjectId === undefined) return false;
    return q === '' || r.name.toLowerCase().includes(q);
  });
}

/** Provenance line for the details inspector — real hash or honest gap. */
export function provenanceOf(row: LibraryRow): string | null {
  if (!row.assetSha256) return null;
  const s = row.assetSha256;
  return s.length > 12 ? `sha256 ${s.slice(0, 12)}…` : `sha256 ${s}`;
}
