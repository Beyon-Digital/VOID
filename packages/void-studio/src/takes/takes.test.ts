// Takes/comp module tests (W17 studio lane; feeds T66).

import { describe, expect, it } from 'vitest';
import {
  addTake,
  buildComp,
  CompError,
  compToOps,
  cycleTakeAt,
  loopTakes,
  makeBuffer,
  makeFolder,
  planPlacement,
  pushChunk,
  disableBuffer,
  enableBuffer,
  recoverToTake,
  repeatNotes,
  seamFadePlan,
  segmentAssetOffset,
  stepInputOps,
  swipeComp,
  takeAt,
  validateComp,
  type TakeFolder,
} from './index';
import type { ClipView } from '../timeline/geometry';

const BAR = '3840000'; // 4 quarters at 960000 ticks

function audioFolder(): TakeFolder {
  return loopTakes({
    folderId: 'fld-1',
    trackId: 'trk-1',
    region: { startTicks: '0', lengthTicks: BAR },
    passes: [
      { takeId: 'take-a', assetId: 'asset-a', sourceHash: 'h1' },
      { takeId: 'take-b', assetId: 'asset-b', sourceHash: 'h2' },
      { takeId: 'take-c', assetId: 'asset-c', sourceHash: 'h3', complete: false },
    ],
  });
}

function mintSeq() {
  let n = 0;
  return () => `m${++n}`;
}

describe('loop takes (REC-04)', () => {
  it('one take per pass; interrupted pass is a kept incomplete take', () => {
    const f = audioFolder();
    expect(f.takes.map((t) => t.takeId)).toEqual(['take-a', 'take-b', 'take-c']);
    expect(f.takes[2].complete).toBe(false);
    // A failed pass never overwrites earlier sources.
    expect(new Set(f.takes.map((t) => t.assetId)).size).toBe(3);
  });

  it('rejects duplicate take ids and duplicate lanes', () => {
    expect(() =>
      makeFolder('f', 't', [
        {
          takeId: 'x',
          folderId: 'f',
          trackId: 't',
          kind: 'AUDIO',
          laneIndex: 0,
          assetId: 'a',
          regionStartTicks: '0',
          regionLengthTicks: BAR,
          offsetTicks: '0',
          complete: true,
        },
        {
          takeId: 'x',
          folderId: 'f',
          trackId: 't',
          kind: 'AUDIO',
          laneIndex: 1,
          assetId: 'b',
          regionStartTicks: '0',
          regionLengthTicks: BAR,
          offsetTicks: '0',
          complete: true,
        },
      ]),
    ).toThrow(/duplicate/);
  });

  it('takeAt resolves the topmost lane under a position', () => {
    const f = audioFolder();
    expect(takeAt(f, '100')!.takeId).toBe('take-c');
    expect(takeAt(f, BAR)).toBeNull(); // half-open region end
  });

  it('addTake never mutates the source folder', () => {
    const f = audioFolder();
    const before = f.takes.length;
    addTake(f, {
      takeId: 'take-d',
      kind: 'AUDIO',
      assetId: 'asset-d',
      regionStartTicks: '0',
      regionLengthTicks: BAR,
      offsetTicks: '0',
      complete: true,
    });
    expect(f.takes.length).toBe(before);
  });
});

describe('comping (REC-05)', () => {
  it('swipeComp builds a contiguous cover with per-segment takes', () => {
    const f = audioFolder();
    const spec = swipeComp({
      compId: 'comp-1',
      folder: f,
      region: { startTicks: '0', lengthTicks: BAR },
      baseTakeId: 'take-a',
      swipes: [
        {
          segmentId: 'seg-mid',
          takeId: 'take-b',
          startTicks: '960000',
          lengthTicks: '960000',
        },
      ],
      mint: mintSeq(),
    });
    expect(spec.segments).toHaveLength(3);
    expect(spec.segments.map((s) => s.takeId)).toEqual([
      'take-a',
      'take-b',
      'take-a',
    ]);
    expect(validateComp(spec, f)).toEqual([]);
  });

  it('compToOps emits remove+insert under one transaction, offsets exact', () => {
    const f = audioFolder();
    const spec = swipeComp({
      compId: 'comp-1',
      folder: f,
      region: { startTicks: '0', lengthTicks: BAR },
      baseTakeId: 'take-a',
      swipes: [
        {
          segmentId: 'seg-mid',
          takeId: 'take-b',
          startTicks: '960000',
          lengthTicks: '960000',
        },
      ],
      mint: mintSeq(),
    });
    const existing: ClipView[] = [
      {
        clipId: 'clip-old',
        trackId: 'trk-1',
        kind: 'AUDIO',
        assetId: 'asset-a',
        startTicks: '0',
        lengthTicks: BAR,
        offsetTicks: '0',
      },
    ];
    const plan = compToOps(spec, f, { clips: existing }, mintSeq());
    expect(plan.removedClipIds).toEqual(['clip-old']);
    expect(plan.insertedClipIds).toHaveLength(3);
    const inserts = plan.ops.filter((o) => 'InsertAudioClipOp' in o);
    expect(inserts).toHaveLength(3);
    const mid = inserts[1] as {
      InsertAudioClipOp: { asset_id: string; offset_ticks: string; start_ticks: string };
    };
    expect(mid.InsertAudioClipOp.asset_id).toBe('asset-b');
    expect(mid.InsertAudioClipOp.offset_ticks).toBe('960000');
    expect(mid.InsertAudioClipOp.start_ticks).toBe('960000');
    // Source takes unchanged: folder still lists all three.
    expect(f.takes.map((t) => t.assetId)).toEqual([
      'asset-a',
      'asset-b',
      'asset-c',
    ]);
  });

  it('seamFades are bounded by the shorter adjacent segment', () => {
    const f = audioFolder();
    const spec = swipeComp({
      compId: 'c',
      folder: f,
      region: { startTicks: '0', lengthTicks: BAR },
      baseTakeId: 'take-a',
      swipes: [
        {
          segmentId: 's',
          takeId: 'take-b',
          startTicks: '100000',
          lengthTicks: '40000', // tiny segment: half = 20000
        },
      ],
      mint: mintSeq(),
    });
    const fades = seamFadePlan(spec, { maxTicks: '240000' });
    expect(fades).toHaveLength(2);
    // Second seam: fade-in of the segment after the tiny one is bounded
    // by 20000 (half the tiny segment), not the 240000 max.
    expect(fades[1].fade.lengthTicks).toBe('20000');
    expect(fades[0].fade.lengthTicks).toBe('20000');
  });

  it('cycleTakeAt switches alternatives under the cursor', () => {
    const f = audioFolder();
    const spec = buildComp({
      compId: 'c',
      trackId: 'trk-1',
      region: { startTicks: '0', lengthTicks: BAR },
      picks: [{ takeId: 'take-a', startTicks: '0', segmentId: 's0' }],
      regionEndTicks: BAR,
      folder: f,
    });
    const next = cycleTakeAt(spec, f, '10', 1)!;
    expect(next.segments[0].takeId).toBe('take-b');
    // Wrap around past the last lane.
    const third = cycleTakeAt(next, f, '10', 1)!;
    expect(third.segments[0].takeId).toBe('take-c');
    const wrap = cycleTakeAt(third, f, '10', 1)!;
    expect(wrap.segments[0].takeId).toBe('take-a');
  });

  it('rejects a comp that does not cover the region', () => {
    const f = audioFolder();
    const spec: import('./types').CompSpec = {
      compId: 'bad',
      trackId: 'trk-1',
      regionStartTicks: '0',
      regionLengthTicks: BAR,
      segments: [
        {
          segmentId: 's',
          takeId: 'take-a',
          startTicks: '0',
          lengthTicks: '960000',
        },
      ],
    };
    expect(validateComp(spec, f).length).toBeGreaterThan(0);
  });

  it('rejects a segment outside its take coverage', () => {
    const f = makeFolder('f', 't', [
      {
        takeId: 'tk',
        folderId: 'f',
        trackId: 't',
        kind: 'AUDIO',
        laneIndex: 0,
        assetId: 'a',
        regionStartTicks: '0',
        regionLengthTicks: '960000', // covers only the first quarter
        offsetTicks: '0',
        complete: true,
      },
    ]);
    expect(() =>
      buildComp({
        compId: 'c',
        trackId: 't',
        region: { startTicks: '0', lengthTicks: BAR },
        picks: [{ takeId: 'tk', startTicks: '0', segmentId: 's' }],
        regionEndTicks: BAR,
        folder: f,
      }),
    ).toThrow(CompError);
  });
});

describe('recording modes + step input (REC-03)', () => {
  const existing: ClipView[] = [
    {
      clipId: 'c1',
      trackId: 't',
      kind: 'AUDIO',
      assetId: 'a1',
      startTicks: '0',
      lengthTicks: '480000',
      offsetTicks: '0',
    },
  ];

  it('replace removes intersecting clips only', () => {
    const p = planPlacement(
      'replace',
      { startTicks: '0', lengthTicks: '240000' },
      existing,
    );
    expect(p.removeClipIds).toEqual(['c1']);
    expect(p.createsLane).toBe(false);
  });

  it('overdub/loop keep existing clips and add a lane', () => {
    for (const mode of ['overdub', 'loop'] as const) {
      const p = planPlacement(
        mode,
        { startTicks: '0', lengthTicks: '240000' },
        existing,
      );
      expect(p.removeClipIds).toEqual([]);
      expect(p.createsLane).toBe(true);
    }
  });

  it('stepInputOps emits InsertNoteOps under one transaction id', () => {
    const { ops, transactionId, noteIds } = stepInputOps(
      'clip-1',
      [
        { pitch: 60, velocity: 100, startTicks: '0', lengthTicks: '240000' },
        { pitch: 64, velocity: 90, startTicks: '240000', lengthTicks: '240000' },
      ],
      mintSeq(),
    );
    expect(ops).toHaveLength(2);
    expect(noteIds).toHaveLength(2);
    expect(transactionId).toBeTruthy();
    const op = ops[0] as {
      InsertNoteOp: { pitch: number; start_ticks: string };
    };
    expect(op.InsertNoteOp.pitch).toBe(60);
  });

  it('repeatNotes fires on grid boundaries and clamps note length', () => {
    const notes = repeatNotes({
      note: {
        pitch: 36,
        velocity: 110,
        startTicks: '0',
        lengthTicks: '300000',
      },
      gridTicks: '240000',
      count: 4,
    });
    expect(notes.map((n) => n.startTicks)).toEqual([
      '0',
      '240000',
      '480000',
      '720000',
    ]);
    // Length clamped to the grid so repeats never overlap.
    expect(notes[0].lengthTicks).toBe('240000');
  });
});

describe('flashback capture (REC-06)', () => {
  it('disabled buffers retain nothing; disabling purges', () => {
    let b = makeBuffer({
      bufferId: 'buf',
      trackId: 't',
      kind: 'MIDI',
      capacityTicks: '1000',
      consentGranted: true,
    });
    b = pushChunk(b, { chunkId: 'c1', startTicks: '0', lengthTicks: '400' });
    expect(b.chunks).toHaveLength(0); // not enabled
    b = enableBuffer(b);
    b = pushChunk(b, { chunkId: 'c1', startTicks: '0', lengthTicks: '400' });
    expect(b.chunks).toHaveLength(1);
    b = disableBuffer(b);
    expect(b.chunks).toHaveLength(0);
  });

  it('ring evicts the oldest chunk past capacity', () => {
    let b = enableBuffer(
      makeBuffer({
        bufferId: 'buf',
        trackId: 't',
        kind: 'MIDI',
        capacityTicks: '1000',
        consentGranted: true,
      }),
    );
    b = pushChunk(b, { chunkId: 'c1', startTicks: '0', lengthTicks: '600' });
    b = pushChunk(b, { chunkId: 'c2', startTicks: '600', lengthTicks: '600' });
    expect(b.chunks.map((c) => c.chunkId)).toEqual(['c2']);
  });

  it('refuses to enable without consent', () => {
    const b = makeBuffer({
      bufferId: 'buf',
      trackId: 't',
      kind: 'MIDI',
      capacityTicks: '1000',
      consentGranted: false,
    });
    expect(() => enableBuffer(b)).toThrow(/consent/);
  });

  it('recoverToTake maps the retained window into a take record', () => {
    let b = enableBuffer(
      makeBuffer({
        bufferId: 'buf',
        trackId: 't',
        kind: 'MIDI',
        capacityTicks: '100000',
        consentGranted: true,
      }),
    );
    b = pushChunk(b, { chunkId: 'c1', startTicks: '0', lengthTicks: '500' });
    b = pushChunk(b, { chunkId: 'c2', startTicks: '500', lengthTicks: '500' });
    const rec = recoverToTake({
      buffer: b,
      takeId: 'tk',
      folderId: 'f',
      laneIndex: 0,
      spanTicks: '900',
    })!;
    expect(rec.regionStartTicks).toBe('0');
    expect(rec.regionLengthTicks).toBe('1000');
    expect(rec.kind).toBe('MIDI');
  });
});
