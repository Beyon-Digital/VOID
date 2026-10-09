import { describe, expect, it } from 'vitest';
import {
  assetRows,
  filterRows,
  instrumentRows,
  provenanceOf,
} from './model';

const presentAsset = {
  object_id: 'a1',
  summary_json: JSON.stringify({
    asset_id: 'asset-1',
    display_name: 'Velvet keys loop',
    sha256: 'deadbeefcafe1234',
    media_type: 'wav',
    state: 'present',
  }),
};

const missingAsset = {
  object_id: 'a2',
  summary_json: JSON.stringify({
    display_name: 'Lost take',
    state: 'missing',
    expected_sha256: 'feedface99887766',
  }),
};

describe('instrumentRows', () => {
  it('lists real builtin descriptors with presets grouped', () => {
    const rows = instrumentRows();
    expect(rows.length).toBeGreaterThan(0);
    expect(rows.some((r) => r.descriptor && !r.preset)).toBe(true);
    expect(rows.every((r) => r.sourceLabel === 'Built-in')).toBe(true);
  });
});

describe('assetRows', () => {
  it('parses present and missing ASSET_LIST rows with provenance', () => {
    const rows = assetRows([presentAsset, missingAsset] as never);
    expect(rows).toHaveLength(2);
    const [p, m] = rows;
    expect(p.name).toBe('Velvet keys loop');
    expect(p.statusLabel).toBe('ready');
    expect(p.assetId).toBe('asset-1');
    expect(m.statusLabel).toBe('missing');
    expect(m.missing).toBe(true);
  });

  it('skips unparseable items instead of forcing them', () => {
    expect(
      assetRows([{ object_id: 'x', summary_json: '{"nope":1}' }] as never),
    ).toHaveLength(0);
  });
});

describe('filterRows', () => {
  const rows = [...instrumentRows(), ...assetRows([presentAsset] as never)];

  it('audio category keeps only engine assets', () => {
    const out = filterRows(rows, 'audio', '');
    expect(out.every((r) => r.assetObjectId !== undefined)).toBe(true);
    expect(out).toHaveLength(1);
  });

  it('presets category keeps only preset rows', () => {
    const out = filterRows(rows, 'presets', '');
    expect(out.every((r) => r.preset !== undefined)).toBe(true);
  });

  it('query matches case-insensitively on name', () => {
    expect(filterRows(rows, 'all', 'VELVET')).toHaveLength(1);
    expect(filterRows(rows, 'all', 'zzz-not-found')).toHaveLength(0);
  });
});

describe('provenanceOf', () => {
  it('truncates real hashes and stays honest when absent', () => {
    const [p] = assetRows([presentAsset] as never);
    expect(provenanceOf(p)).toBe('sha256 deadbeefcafe…');
    expect(provenanceOf(instrumentRows()[0])).toBeNull();
  });
});
