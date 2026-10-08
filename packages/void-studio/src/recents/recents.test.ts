import { describe, expect, it } from 'vitest';
import { memoryStore } from './persistence';
import { createRecentsStore, entriesFromJson, RECENTS_MAX } from './recents';

const base = { projectId: 'p1', name: 'Song', containerDir: '/tmp/song.void' };
const stored = { ...base, kind: 'created' as const, lastOpenedAt: '2026-10-08T00:00:00Z' };

describe('entriesFromJson', () => {
  it('parses the stored envelope and drops malformed rows', () => {
    const raw = JSON.stringify({
      v: 1,
      entries: [stored, { bogus: true }, { ...stored, containerDir: '/other.void', kind: 'opened' }],
    });
    const out = entriesFromJson(raw);
    expect(out).toHaveLength(2);
    expect(out[0].containerDir).toBe('/tmp/song.void');
    expect(out[1].kind).toBe('opened');
  });

  it('tolerates corrupt payloads', () => {
    expect(entriesFromJson(null)).toEqual([]);
    expect(entriesFromJson('not json')).toEqual([]);
    expect(entriesFromJson('{"entries":42}')).toEqual([]);
    expect(entriesFromJson('[]')).toEqual([]);
  });
});

describe('recents store', () => {
  it('records real opens, most recent first, deduped by containerDir', () => {
    const kv = memoryStore();
    const s = createRecentsStore(kv);
    s.getState().actions.recordOpen({ ...base, kind: 'created' });
    s.getState().actions.recordOpen({ ...base, containerDir: '/b.void', projectId: 'p2', name: 'B', kind: 'opened' });
    s.getState().actions.recordOpen({ ...base, kind: 'opened', lastOpenedAt: '2026-01-01T00:00:00Z' });
    const entries = s.getState().entries;
    expect(entries).toHaveLength(2);
    expect(entries[0].containerDir).toBe('/tmp/song.void'); // re-opened → front
    expect(entries[0].kind).toBe('opened');
    expect(entries[0].lastOpenedAt).toBe('2026-01-01T00:00:00Z');
  });

  it('caps at RECENTS_MAX', () => {
    const s = createRecentsStore(memoryStore());
    for (let i = 0; i < RECENTS_MAX + 5; i++) {
      s.getState().actions.recordOpen({
        projectId: `p${i}`,
        name: `s${i}`,
        containerDir: `/d${i}.void`,
        kind: 'created',
      });
    }
    expect(s.getState().entries).toHaveLength(RECENTS_MAX);
    expect(s.getState().entries[0].projectId).toBe(`p${RECENTS_MAX + 4}`);
  });

  it('persists through the KeyValueStore and rehydrates in a new store', () => {
    const kv = memoryStore();
    const s1 = createRecentsStore(kv);
    s1.getState().actions.recordOpen({ ...base, kind: 'created' });
    const s2 = createRecentsStore(kv);
    expect(s2.getState().entries).toHaveLength(1);
    expect(s2.getState().entries[0].projectId).toBe('p1');
  });

  it('remove and clear persist', () => {
    const kv = memoryStore();
    const s = createRecentsStore(kv);
    s.getState().actions.recordOpen({ ...base, kind: 'created' });
    s.getState().actions.remove('/tmp/song.void');
    expect(s.getState().entries).toHaveLength(0);
    s.getState().actions.recordOpen({ ...base, kind: 'created' });
    s.getState().actions.clear();
    expect(s.getState().entries).toHaveLength(0);
    expect(createRecentsStore(kv).getState().entries).toHaveLength(0);
  });

  it('surfaces persistence errors in state instead of throwing', () => {
    const failing = {
      getItem: () => null,
      setItem: () => {
        throw new Error('quota');
      },
      removeItem: () => undefined,
    };
    const s = createRecentsStore(failing);
    s.getState().actions.recordOpen({ ...base, kind: 'created' });
    expect(s.getState().persistError).toContain('quota');
    expect(s.getState().entries).toHaveLength(1); // still tracked in-memory
  });
});
