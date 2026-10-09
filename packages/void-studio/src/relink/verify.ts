// Relink candidate verification (W11, mirrors crates/void-assets link.rs).
//
// `relink()` in the coordinator accepts candidate bytes only when their
// sha256 equals the expected content address; anything else requires the
// user's explicit replacement choice. The WebView computes the same
// digest with WebCrypto so the UI can show the real outcome BEFORE any
// op is attempted — "relink" (hash match, original restored) vs
// "replace" (different content, explicit consent required).

import type { AttachAssetOp, PersistentOp } from 'void-client';
import type { AssetRow } from './model';

/** sha256 of `bytes` as lowercase hex — the content address format. */
export async function sha256Hex(bytes: ArrayBuffer | Uint8Array): Promise<string> {
  const buf = bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes);
  const digest = await crypto.subtle.digest('SHA-256', buf as BufferSource);
  return Array.from(new Uint8Array(digest))
    .map((b) => b.toString(16).padStart(2, '0'))
    .join('');
}

export interface CandidateVerification {
  sha256: string;
  /** 'relink' = content matches the expectation; 'replace' = new media. */
  outcome: 'relink' | 'replace';
  /** Replace requires the user's explicit confirmation (link.rs rule). */
  requiresExplicitReplace: boolean;
}

/** Verify `bytes` against the row's expected content address. */
export async function verifyCandidate(
  row: AssetRow,
  bytes: ArrayBuffer | Uint8Array,
): Promise<CandidateVerification> {
  if (!row.expectedSha256) {
    throw new Error(`asset row ${row.objectId} carries no expected sha256`);
  }
  const sha256 = await sha256Hex(bytes);
  const outcome = sha256 === row.expectedSha256 ? 'relink' : 'replace';
  return { sha256, outcome, requiresExplicitReplace: outcome === 'replace' };
}

/**
 * Build the re-register op for a verified candidate. AttachAssetOp is the
 * honest wire action: it registers an already-imported immutable blob
 * with the engine. The coordinator still owns placing the bytes under
 * assets/sha256/ — see NEEDS.md §rev2; the op's real receipt (APPLIED or
 * ASSET_MISSING/etc.) is what the UI reports.
 */
export function attachRelinkOp(
  row: AssetRow,
  verification: CandidateVerification,
  fields: { assetId?: string; relPath?: string; mediaType?: string } = {},
): PersistentOp {
  const op: AttachAssetOp = {
    asset_id: fields.assetId ?? row.assetId ?? verification.sha256,
    sha256: verification.sha256,
    media_type: fields.mediaType ?? row.mediaType,
    rel_path: fields.relPath ?? row.relPath ?? `${verification.sha256}`,
  };
  return { AttachAssetOp: op };
}
