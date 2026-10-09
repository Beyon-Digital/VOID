import { describe, expect, it } from 'vitest';
import { ClipMatrix, SessionClip, groupClipsByScene } from './SessionView';

function clip(id: string, sceneId: string, trackId: string): SessionClip {
  return {
    id,
    name: id,
    sceneId,
    trackId,
    startBeat: 0,
    length: 4,
    loop: true,
    type: 'midi',
  };
}

describe('groupClipsByScene', () => {
  it('places clips by explicit sceneId only, never by ID-prefix', () => {
    // T02 collision regression: scenes "a" and "ab"; clip IDs deliberately
    // share prefixes with scene IDs they do not belong to.
    const matrix: ClipMatrix = {
      scenes: [
        { id: 'a', name: 'A' },
        { id: 'ab', name: 'AB' },
      ],
      clips: [
        clip('ab-lead', 'a', 't1'), // ID starts with "ab" but belongs to scene "a"
        clip('a-pad', 'ab', 't1'), // ID starts with "a" but belongs to scene "ab"
        clip('zzz', 'ab', 't2'),
      ],
    };

    const grouped = groupClipsByScene(matrix);

    expect(grouped.get('a')!.map((c) => c.id)).toEqual(['ab-lead']);
    expect(grouped.get('ab')!.map((c) => c.id)).toEqual(['a-pad', 'zzz']);
  });

  it('drops clips whose sceneId matches no present scene', () => {
    const matrix: ClipMatrix = {
      scenes: [{ id: 'a', name: 'A' }],
      clips: [clip('a-1', 'a', 't1'), clip('ghost', 'missing-scene', 't1')],
    };

    const grouped = groupClipsByScene(matrix);

    expect(grouped.get('a')!.map((c) => c.id)).toEqual(['a-1']);
    expect([...grouped.values()].flat().map((c) => c.id)).not.toContain('ghost');
  });

  it('keeps distinct clips distinct even when IDs collide across scenes', () => {
    const matrix: ClipMatrix = {
      scenes: [
        { id: 's1', name: 'S1' },
        { id: 's2', name: 'S2' },
      ],
      clips: [clip('clip-x', 's1', 't1'), clip('clip-x', 's2', 't2')],
    };

    const grouped = groupClipsByScene(matrix);

    expect(grouped.get('s1')!.map((c) => c.trackId)).toEqual(['t1']);
    expect(grouped.get('s2')!.map((c) => c.trackId)).toEqual(['t2']);
  });
});
