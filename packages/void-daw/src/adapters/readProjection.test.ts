import { describe, expect, it } from 'vitest';
import { clipMatrixFromReadItems } from './readProjection';

describe('clipMatrixFromReadItems', () => {
  it('builds scenes and clips from wire-shaped read_view items', () => {
    const items = [
      { object_id: 'scene:s1', summary_json: JSON.stringify({ id: 's1', name: 'Intro', tempo: 120 }) },
      { object_id: 'scene:s2', summary_json: JSON.stringify({ id: 's2', name: 'Verse' }) },
      {
        object_id: 'clip:c1',
        summary_json: JSON.stringify({
          clip_id: 'c1',
          name: 'Drums',
          scene_id: 's1',
          track_id: 't1',
          start_beat: 0,
          length_beats: 8,
          loop: true,
          color: '#f00',
          clip_type: 'audio',
        }),
      },
      { object_id: 'clip:c2', summary_json: JSON.stringify({ clip_id: 'c2', scene_id: 's2', track_id: 't2', clip_type: 'midi' }) },
    ];
    const { matrix, dropped, truncated } = clipMatrixFromReadItems(items);
    expect(dropped).toBe(0);
    expect(truncated).toBe(false);
    expect(matrix.scenes.map((s) => s.id)).toEqual(['s1', 's2']);
    expect(matrix.clips).toHaveLength(2);
    expect(matrix.clips[0]).toMatchObject({ id: 'c1', sceneId: 's1', trackId: 't1', loop: true, type: 'audio' });
    expect(matrix.clips[1]).toMatchObject({ id: 'c2', type: 'midi', name: 'c2' });
  });

  it('skips malformed and unknown items instead of guessing', () => {
    const { matrix, dropped } = clipMatrixFromReadItems([
      { object_id: 'weird', summary_json: 'not json' },
      { object_id: 'clip:broken', summary_json: JSON.stringify({ clip_id: 'broken' }) }, // no scene/track
      { object_id: 'note:n1', summary_json: JSON.stringify({ kind: 'note', pitch: 60 }) },
      { object_id: 'scene:ok', summary_json: JSON.stringify({ id: 'ok' }) },
    ]);
    expect(matrix.scenes.map((s) => s.id)).toEqual(['ok']);
    expect(matrix.clips).toHaveLength(0);
    expect(dropped).toBe(3);
  });

  it('never infers scene membership from id prefixes', () => {
    const { matrix } = clipMatrixFromReadItems([
      { object_id: 'scene:s1', summary_json: JSON.stringify({ id: 's1' }) },
      { object_id: 'clip:s1-x', summary_json: JSON.stringify({ clip_id: 's1-x', scene_id: 's9', track_id: 't' }) },
    ]);
    expect(matrix.clips[0].sceneId).toBe('s9');
    expect(matrix.scenes.map((s) => s.id)).toEqual(['s1']);
  });

  it('bounds the projection and reports truncation', () => {
    const items = Array.from({ length: 10 }, (_, i) => ({
      object_id: `clip:c${i}`,
      summary_json: JSON.stringify({ clip_id: `c${i}`, scene_id: 's', track_id: 't' }),
    }));
    const { matrix, truncated } = clipMatrixFromReadItems(items, { maxItems: 4 });
    expect(truncated).toBe(true);
    expect(matrix.clips.length).toBeLessThanOrEqual(4);
  });
});
