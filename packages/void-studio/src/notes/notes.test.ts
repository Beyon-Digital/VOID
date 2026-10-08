import { describe, expect, it } from 'vitest';
import { memoryStore } from '../recents/persistence';
import {
  NOTE_OPS_AVAILABLE,
  proposedSetProjectNoteOp,
  proposedSetTrackNoteOp,
} from './dto';
import {
  createNoteDraftsStore,
  draftsFromJson,
  projectNoteKey,
  trackNoteKey,
} from './drafts';
import { parseNoteField, parseProjectInfo } from './parse';

const item = (summary: unknown, object_id = 'o1') => ({
  object_id,
  summary_json: JSON.stringify(summary),
});

describe('note op DTOs (proposed, rev2)', () => {
  it('reports the ops unavailable on protocol major.1', () => {
    expect(NOTE_OPS_AVAILABLE).toBe(false);
  });

  it('builders emit the intended tagged payloads', () => {
    expect(proposedSetProjectNoteOp('hello')).toEqual({
      SetProjectNoteOp: { text: 'hello' },
    });
    expect(proposedSetTrackNoteOp('t1', 'snare')).toEqual({
      SetTrackNoteOp: { track_id: 't1', text: 'snare' },
    });
  });
});

describe('parseProjectInfo', () => {
  it('parses flat and nested summaries defensively', () => {
    const flat = parseProjectInfo(
      item({ project_id: 'p1', name: 'Song', sample_rate: 48000, bpm: 128, note: 'v2' }),
    );
    expect(flat).toMatchObject({
      projectId: 'p1',
      name: 'Song',
      sampleRate: 48000,
      bpm: 128,
      note: 'v2',
    });
    const nested = parseProjectInfo(
      item({ project: { name: 'S', time_signature: { numerator: 6, denominator: 8 }, track_count: 4 } }),
    );
    expect(nested?.timeSignature).toBe('6/8');
    expect(nested?.trackCount).toBe(4);
  });

  it('returns null for payloads without project fields and never fabricates', () => {
    expect(parseProjectInfo(item({}))).toBeNull();
    expect(parseProjectInfo({ summary_json: 'not json' })).toBeNull();
    const p = parseProjectInfo(item({ name: 'Only Name' }));
    expect(p?.name).toBe('Only Name');
    expect(p?.bpm).toBeUndefined();
    expect(p?.note).toBeUndefined();
  });
});

describe('parseNoteField', () => {
  it('reads note/notes from a view row, absent stays absent', () => {
    expect(parseNoteField(item({ track_id: 't1', note: 'verse vox' }))).toBe('verse vox');
    expect(parseNoteField(item({ track_id: 't1' }))).toBeUndefined();
    expect(parseNoteField(item({ notes: 'alt take' }))).toBe('alt take');
  });
});

describe('note drafts store', () => {
  it('keeps drafts keyed by project/track, persists through kv', () => {
    const kv = memoryStore();
    const s = createNoteDraftsStore(kv);
    const pk = projectNoteKey('p1');
    const tk = trackNoteKey('p1', 't1');
    s.getState().actions.setDraft(pk, 'project note draft');
    s.getState().actions.setDraft(tk, 'track note draft');
    expect(s.getState().drafts[pk].text).toBe('project note draft');
    expect(s.getState().drafts[tk].key).toBe(tk);
    // rehydrate
    const s2 = createNoteDraftsStore(kv);
    expect(Object.keys(s2.getState().drafts)).toHaveLength(2);
    s2.getState().actions.clearDraft(pk);
    expect(s2.getState().drafts[pk]).toBeUndefined();
    expect(s2.getState().drafts[tk]).toBeTruthy();
  });

  it('tolerates corrupt persisted payloads', () => {
    expect(draftsFromJson(null)).toEqual({});
    expect(draftsFromJson('not json')).toEqual({});
    expect(draftsFromJson('{"drafts":[{"key":"k"}]}')).toEqual({});
  });
});
