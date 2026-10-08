import { describe, expect, it } from 'vitest';
import { missingAssetRows, parseAssetItem } from './model';
import { attachRelinkOp, sha256Hex, verifyCandidate } from './verify';

const item = (summary: unknown, object_id = 'a1') => ({
  object_id,
  summary_json: JSON.stringify(summary),
});

// The coordinator's MediaLink serde shape (crates/void-assets link.rs).
const MISSING = {
  state: 'missing',
  expected_sha256: 'ab12',
  display_name: 'kick.wav',
  detail: 'moved off disk',
};

describe('parseAssetItem', () => {
  it('parses the MediaLink serde shape', () => {
    const r = parseAssetItem(item(MISSING));
    expect(r).toMatchObject({
      state: 'missing',
      expectedSha256: 'ab12',
      displayName: 'kick.wav',
      detail: 'moved off disk',
    });
    const p = parseAssetItem(item({ state: 'present', sha256: 'cd34', rel_path: 'assets/sha256/cd34.wav' }));
    expect(p).toMatchObject({ state: 'present', expectedSha256: 'cd34', relPath: 'assets/sha256/cd34.wav' });
  });

  it('parses flat missing markers', () => {
    const r = parseAssetItem(item({ asset_id: 'as1', missing: true, expected_sha256: 'ef56', name: 'vox.wav' }));
    expect(r).toMatchObject({ state: 'missing', assetId: 'as1', expectedSha256: 'ef56' });
  });

  it('rejects rows it cannot honestly classify', () => {
    expect(parseAssetItem(item({ unrelated: true }))).toBeNull();
    expect(parseAssetItem(item({ state: 'missing' }))).toBeNull(); // no expected hash → not actionable
    expect(parseAssetItem({ summary_json: 'broken' })).toBeNull();
  });

  it('missingAssetRows filters present + unparseable rows', () => {
    const rows = missingAssetRows([
      item(MISSING),
      item({ state: 'present', sha256: 'x' }),
      item({ junk: 1 }),
    ]);
    expect(rows).toHaveLength(1);
    expect(rows[0].displayName).toBe('kick.wav');
  });
});

describe('verifyCandidate', () => {
  it('hashes bytes and classifies relink vs replace', async () => {
    const bytes = new TextEncoder().encode('hello void');
    const sha = await sha256Hex(bytes);
    expect(sha).toMatch(/^[0-9a-f]{64}$/);
    const row = parseAssetItem(item({ ...MISSING, expected_sha256: sha }))!;
    const v = await verifyCandidate(row, bytes);
    expect(v.outcome).toBe('relink');
    expect(v.requiresExplicitReplace).toBe(false);

    const other = await verifyCandidate(row, new TextEncoder().encode('different'));
    expect(other.outcome).toBe('replace');
    expect(other.requiresExplicitReplace).toBe(true);
  });
});

describe('attachRelinkOp', () => {
  it('builds AttachAssetOp with the verified sha and row paths', async () => {
    const row = parseAssetItem(
      item({ ...MISSING, expected_sha256: 'ab12', asset_id: 'as9', rel_path: 'assets/sha256/ab12.wav', media_type: 'wav' }),
    )!;
    const op = attachRelinkOp(row, { sha256: 'ab12', outcome: 'relink', requiresExplicitReplace: false });
    expect(op).toEqual({
      AttachAssetOp: {
        asset_id: 'as9',
        sha256: 'ab12',
        media_type: 'wav',
        rel_path: 'assets/sha256/ab12.wav',
      },
    });
  });
});
