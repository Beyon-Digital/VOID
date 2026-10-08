// Missing-asset rows for the relink recovery surface (W11 item 2, T19 /
// CONTRACTS §4: a missing asset resolves to a visible relink placeholder,
// never silently substituted audio).
//
// The parser mirrors crates/void-assets MediaLink's serde shape —
// `{state:"present"|"missing", expected_sha256, display_name, detail}` —
// and tolerates flat summary variants (`missing:true`, `present:false`).
// An ASSET_LIST row that does not parse into either state is not an asset
// row we can act on: it's returned as `unknown`, never forced missing.

import type { ReadItem } from 'void-client';

export interface AssetRow {
  /** ReadItem.object_id — the view's identity for this asset row. */
  objectId: string;
  /** asset_id if the row carries one (AttachAssetOp target). */
  assetId?: string;
  state: 'present' | 'missing';
  /** The content address the project expects (missing rows). */
  expectedSha256?: string;
  displayName: string;
  detail?: string;
  /** Project-relative blob path when the row reports one. */
  relPath?: string;
  mediaType?: string;
}

function asObj(v: unknown): Record<string, unknown> | null {
  return typeof v === 'object' && v !== null ? (v as Record<string, unknown>) : null;
}

function str(v: unknown): string | undefined {
  return typeof v === 'string' && v.length > 0 ? v : undefined;
}

/**
 * Parse one ASSET_LIST ReadItem (or a decoded summary object).
 * Returns null when the payload cannot be read as an asset row at all.
 */
export function parseAssetItem(item: ReadItem | unknown): AssetRow | null {
  let p: Record<string, unknown> | null;
  try {
    p = asObj(
      typeof item === 'object' && item !== null && 'summary_json' in (item as object)
        ? JSON.parse((item as ReadItem).summary_json)
        : item,
    );
  } catch {
    return null;
  }
  if (!p) return null;

  const objectId = str((item as ReadItem | null)?.object_id) ?? str(p.object_id) ?? str(p.asset_id);
  const assetId = str(p.asset_id) ?? str(p.assetId);
  const displayName =
    str(p.display_name) ?? str(p.displayName) ?? str(p.name) ?? assetId ?? objectId ?? 'asset';
  const relPath = str(p.rel_path) ?? str(p.relPath) ?? str(p.path);
  const mediaType = str(p.media_type) ?? str(p.mediaType) ?? str(p.type);
  const detail = str(p.detail) ?? str(p.message);

  // MediaLink serde tag wins; fall back to boolean-ish flat markers.
  const tag = str(p.state) ?? str(p.status) ?? str(p.link_state);
  const missingFlag = p.missing === true || p.present === false;
  const expected =
    str(p.expected_sha256) ?? str(p.expectedSha256) ?? str(p.expected) ?? str(p.sha256);

  if (tag === 'missing' || missingFlag) {
    if (!expected) return null; // a missing row without its expected hash is not actionable
    return {
      objectId: objectId ?? expected,
      assetId,
      state: 'missing',
      expectedSha256: expected,
      displayName,
      detail,
      relPath,
      mediaType,
    };
  }
  if (tag === 'present' || str(p.sha256)) {
    return {
      objectId: objectId ?? str(p.sha256) ?? displayName,
      assetId,
      state: 'present',
      expectedSha256: str(p.sha256) ?? expected,
      displayName,
      detail,
      relPath,
      mediaType,
    };
  }
  return null;
}

/** Missing rows only — what the relink panel lists. */
export function missingAssetRows(items: readonly (ReadItem | unknown)[]): AssetRow[] {
  return items.map(parseAssetItem).filter((r): r is AssetRow => r !== null && r.state === 'missing');
}
