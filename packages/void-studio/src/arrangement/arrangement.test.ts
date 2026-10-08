import { describe, expect, it } from 'vitest';
import {
  aliasPropagateOps,
  applyProjectAlternativeOps,
  applyTrackAlternativeOps,
  assertUnprotected,
  captureTrackAlternative,
  checkSections,
  copySectionOps,
  createArrangementStore,
  crossfadeForSeam,
  detectSilence,
  freezeApplyOps,
  freezeRenderRegion,
  groupEditOps,
  insertMeterOp,
  insertTempoOp,
  keptRegions,
  loopClipOps,
  makeStreamCache,
  moveSectionOps,
  orderTracksWithFolders,
  ProtectedError,
  requestWindow,
  sectionBars,
  sendPlan,
  setFade,
  stripSilenceOps,
  ticksToBarBeat,
  tileResident,
  tileWindows,
  type AlternativeClipSpec,
  type EditGroup,
  type FadeMap,
  type Section,
  type TrackAlternative,
  visibleTracks,
  type LoudnessTile,
} from './index';
import type { ClipView } from '../timeline/geometry';
import { FakeTransport } from 'void-client';
import { VoidClient } from 'void-client';

const BAR = '3840000';

let seq = 0;
const mint = () => `m${++seq}`;

function audioClip(partial: Partial<ClipView> & { clipId: string }): ClipView {
  return {
    trackId: 'trk1',
    kind: 'AUDIO',
    assetId: 'asset-1',
    startTicks: '0',
    lengthTicks: BAR,
    offsetTicks: '0',
    ...partial,
  } as ClipView;
}

describe('region loops + aliases (ARR-03)', () => {
  it('loopClipOps inserts repeats back-to-back under one transaction', () => {
    const plan = loopClipOps(audioClip({ clipId: 'c1' }), 2, mint);
    expect(plan.ops).toHaveLength(2);
    for (const [i, op] of plan.ops.entries()) {
      const ins = (op as { InsertAudioClipOp: { start_ticks: string; track_id: string } })
        .InsertAudioClipOp;
      expect(ins.start_ticks).toBe((BigInt(BAR) * BigInt(i + 1)).toString());
      expect(ins.track_id).toBe('trk1');
    }
    expect(plan.insertedClipIds).toHaveLength(2);
    expect(new Set(plan.ops.map((_, i) => plan.insertedClipIds[i]))).not.toContain('c1');
  });

  it('aliasPropagateOps fans a trim out to every member; !propagate is null', () => {
    const g = { groupId: 'g', memberClipIds: ['a', 'b', 'c'], propagate: true };
    const edit = { TrimClipOp: { clip_id: 'a', start_ticks: '0', length_ticks: BAR } };
    const ops = aliasPropagateOps(g, edit)!;
    expect(ops.map((o) => (o as { TrimClipOp: { clip_id: string } }).TrimClipOp.clip_id)).toEqual(
      ['a', 'b', 'c'],
    );
    expect(aliasPropagateOps({ ...g, propagate: false }, edit)).toBeNull();
    expect(aliasPropagateOps(g, { RemoveClipOp: { clip_id: 'a' } })).toBeNull();
  });

  it('orderTracksWithFolders groups members under the folder header; collapse hides them', () => {
    const folders = [
      { folderId: 'f1', name: 'drums', memberTrackIds: ['t2', 't3'], collapsed: false, kind: 'folder' as const },
    ];
    const open = orderTracksWithFolders(['t1', 't2', 't3', 't4'], folders);
    expect(open).toEqual([
      { trackId: 't1' },
      { folderId: 'f1' },
      { trackId: 't2' },
      { trackId: 't3' },
      { trackId: 't4' },
    ]);
    const closed = orderTracksWithFolders(['t1', 't2', 't3', 't4'], [
      { ...folders[0]!, collapsed: true },
    ]);
    expect(closed).toEqual([{ trackId: 't1' }, { folderId: 'f1' }, { trackId: 't4' }]);
  });
});

describe('fades + strip silence (ARR-05)', () => {
  it('setFade stores specs per clip edge and bounds length to the clip', () => {
    const clip = audioClip({ clipId: 'c1' });
    let map: FadeMap = {};
    map = setFade(map, clip, 'in', { shape: 'linear', lengthTicks: '1000' });
    expect(map['c1']?.fadeIn?.lengthTicks).toBe('1000');
    expect(() =>
      setFade(map, clip, 'out', { shape: 'linear', lengthTicks: '99999999' }),
    ).toThrow();
  });

  it('crossfadeForSeam returns bounded equal fades for overlapping clips', () => {
    const left = audioClip({ clipId: 'l', startTicks: '0', lengthTicks: BAR });
    const right = audioClip({ clipId: 'r', startTicks: '3600000', lengthTicks: BAR });
    const { leftFade, rightFade } = crossfadeForSeam(left, right, { lengthTicks: '120000' });
    expect(leftFade?.lengthTicks).toBe('120000');
    expect(rightFade?.shape).toBe('equal-power');
    // no overlap → no fades
    const far = audioClip({ clipId: 'r2', startTicks: BAR, lengthTicks: BAR });
    expect(crossfadeForSeam(left, far).leftFade).toBeNull();
  });

  it('detectSilence merges under-threshold runs at min duration', () => {
    const tiles: LoudnessTile[] = [
      { startTicks: '0', lengthTicks: '100', peakDb: -60 },
      { startTicks: '100', lengthTicks: '100', peakDb: -70 },
      { startTicks: '200', lengthTicks: '100', peakDb: -20 },
      { startTicks: '300', lengthTicks: '100', peakDb: -80 },
    ];
    const silent = detectSilence(tiles, { thresholdDb: -50, minSilenceTicks: '150' });
    expect(silent).toEqual([{ startTicks: '0', lengthTicks: '200' }]);
  });

  it('stripSilenceOps removes the source and inserts kept regions with asset offsets', () => {
    const clip = audioClip({
      clipId: 'c1',
      startTicks: '1000',
      lengthTicks: '400',
      offsetTicks: '500',
    });
    const tiles: LoudnessTile[] = [
      { startTicks: '1000', lengthTicks: '100', peakDb: -10 },
      { startTicks: '1100', lengthTicks: '200', peakDb: -90 },
      { startTicks: '1300', lengthTicks: '100', peakDb: -15 },
    ];
    const plan = stripSilenceOps(clip, tiles, {
      thresholdDb: -50,
      minSilenceTicks: '100',
      padTicks: '0',
    }, mint);
    expect(plan.removedClipIds).toEqual(['c1']);
    const removes = plan.ops.filter((o) => 'RemoveClipOp' in o);
    const inserts = plan.ops.filter((o) => 'InsertAudioClipOp' in o);
    expect(removes).toHaveLength(1);
    expect(inserts).toHaveLength(2);
    const kept = keptRegions(clip, tiles, {
      thresholdDb: -50, minSilenceTicks: '100', padTicks: '0',
    });
    expect(kept).toEqual([
      { startTicks: '1000', lengthTicks: '100' },
      { startTicks: '1300', lengthTicks: '100' },
    ]);
    const second = (inserts[1] as { InsertAudioClipOp: { offset_ticks: string } }).InsertAudioClipOp;
    expect(second.offset_ticks).toBe('800'); // 500 + (1300-1000)
  });
});

describe('sections + markers (TIME-04)', () => {
  const sections: Section[] = [
    { sectionId: 's1', name: 'verse', startTicks: '0', lengthTicks: BAR },
    { sectionId: 's2', name: 'chorus', startTicks: BAR, lengthTicks: BAR },
  ];

  it('checkSections flags overlaps and malformed sections', () => {
    expect(checkSections(sections)).toEqual([]);
    expect(
      checkSections([
        { sectionId: 'a', name: 'a', startTicks: '0', lengthTicks: '10' },
        { sectionId: 'b', name: 'b', startTicks: '5', lengthTicks: '10' },
      ]),
    ).toHaveLength(1);
  });

  it('moveSectionOps moves inside clips, splits boundary clips, shifts markers', () => {
    const clips = [
      audioClip({ clipId: 'inside', startTicks: '0', lengthTicks: '1000' }),
      audioClip({ clipId: 'crossing', startTicks: '3700000', lengthTicks: '400000' }), // crosses BAR
      audioClip({ clipId: 'outside', startTicks: '9000000', lengthTicks: '1000' }),
    ];
    const markers = [{ markerId: 'mk1', name: 'm', ticks: '1000', kind: 'marker' as const }];
    const plan = moveSectionOps(
      sections[0]!,
      clips,
      markers,
      '8000000',
      { clips: true, markers: true, automation: true, chords: true },
      mint,
    );
    const moves = plan.ops.filter((o) => 'MoveClipOp' in o);
    const splits = plan.ops.filter((o) => 'SplitClipOp' in o);
    // inside clip moves by +8000000; crossing clip splits at BAR then
    // its inside piece moves.
    expect(moves).toHaveLength(2);
    expect(splits).toHaveLength(1);
    expect(
      (splits[0] as { SplitClipOp: { at_ticks: string } }).SplitClipOp.at_ticks,
    ).toBe(BAR);
    const insideMove = (moves[0] as { MoveClipOp: { clip_id: string; start_ticks: string } })
      .MoveClipOp;
    expect(insideMove).toEqual({ clip_id: 'inside', track_id: 'trk1', start_ticks: '8000000' });
    expect(plan.markerMoves).toEqual([{ markerId: 'mk1', ticks: '8001000' }]);
    expect(plan.carriedSpec.automation).toBe(true);
  });

  it('copySectionOps duplicates inside clips at the destination offset', () => {
    const clips = [
      audioClip({ clipId: 'a', startTicks: '100', lengthTicks: '1000' }),
      audioClip({ clipId: 'b', startTicks: '9000000', lengthTicks: '10' }),
    ];
    const plan = copySectionOps(sections[0]!, clips, BAR, {
      clips: true, markers: false, automation: false, chords: false,
    }, mint);
    const inserts = plan.ops.filter((o) => 'InsertAudioClipOp' in o);
    expect(inserts).toHaveLength(1);
    expect(
      (inserts[0] as { InsertAudioClipOp: { start_ticks: string } }).InsertAudioClipOp.start_ticks,
    ).toBe((BigInt(BAR) + 100n).toString());
    expect(plan.ops.some((o) => 'RemoveClipOp' in o)).toBe(false);
  });

  it('sectionBars maps ranges to bar positions under 4/4', () => {
    const b = sectionBars(sections[1]!);
    expect(b).toEqual({ startBar: 1, endBar: 2 });
  });
});

describe('tempo + meter (TIME-03)', () => {
  it('insertTempoOp/insertMeterOp emit one op under one transaction, validate input', () => {
    const t = insertTempoOp(BAR, 140, mint);
    expect(t.ops).toEqual([
      { SetTempoOp: { at_ticks: BAR, bpm: 140 } },
    ]);
    const m = insertMeterOp('0', 3, 4, mint);
    expect(m.ops).toEqual([
      { SetTimeSignatureOp: { at_ticks: '0', numerator: 3, denominator: 4 } },
    ]);
    expect(() => insertTempoOp('0', 0, mint)).toThrow();
  });

  it('ticksToBarBeat is exact at meter-change boundaries', () => {
    const meters = [
      { atTicks: '0', numerator: 4, denominator: 4 },
      { atTicks: BAR, numerator: 3, denominator: 4 },
    ];
    // tick at BAR = bar 1 beat 0 of the new 3/4 meter
    const at = ticksToBarBeat(BAR, meters);
    expect(at.bar).toBe(1);
    expect(at.beatInBar).toBe(0);
    // two quarters into the 3/4 bar
    const later = ticksToBarBeat((BigInt(BAR) + 1920000n).toString(), meters);
    expect(later.bar).toBe(1);
    expect(later.beatInBar).toBe(2);
  });
});

describe('groups + protected edits (ARR-06/07)', () => {
  const group: EditGroup = { groupId: 'g1', name: 'rhythm', trackIds: ['trk1', 'trk2'] };
  const memberClips = (t: string) =>
    t === 'trk1' ? [audioClip({ clipId: 'c1' })] : [audioClip({ clipId: 'c2', trackId: 'trk2' })];

  it('groupEditOps fans a member edit to every member in one batch', () => {
    const ops = groupEditOps(
      group,
      'c1',
      'trk1',
      { MoveClipOp: { clip_id: 'c1', track_id: 'trk1', start_ticks: '500' } },
      memberClips,
    )!;
    expect(ops).toEqual([
      { MoveClipOp: { clip_id: 'c1', track_id: 'trk1', start_ticks: '500' } },
      { MoveClipOp: { clip_id: 'c2', track_id: 'trk2', start_ticks: '500' } },
    ]);
  });

  it('assertUnprotected blocks edits to protected clips, tracks and ranges', () => {
    const base = {
      protectedClipIds: new Set(['c1']),
      protectedTrackIds: new Set<string>(),
      clipAt: () => audioClip({ clipId: 'c1' }),
    };
    expect(() =>
      assertUnprotected({ MoveClipOp: { clip_id: 'c1', track_id: 't', start_ticks: '0' } }, base),
    ).toThrow(ProtectedError);
    expect(() =>
      assertUnprotected(
        { MoveClipOp: { clip_id: 'c1', track_id: 't', start_ticks: '0' } },
        { protectedClipIds: new Set(), protectedTrackIds: new Set(), clipAt: base.clipAt },
      ),
    ).not.toThrow();
    expect(() =>
      assertUnprotected(
        { RemoveTrackOp: { track_id: 't9' } },
        { protectedClipIds: new Set(), protectedTrackIds: new Set(['t9']) },
      ),
    ).toThrow(ProtectedError);
    // insert into a protected range
    expect(() =>
      assertUnprotected(
        {
          InsertAudioClipOp: {
            clip_id: 'n1', track_id: 't1', asset_id: 'a',
            start_ticks: '50', length_ticks: '50',
          },
        },
        {
          protectedClipIds: new Set(),
          protectedTrackIds: new Set(),
          protectedRegions: [{ trackId: 't1', region: { startTicks: '0', lengthTicks: '100' } }],
          clipAt: () => undefined,
        },
      ),
    ).toThrow(ProtectedError);
  });

  it('visibleTracks filters the hidden set', () => {
    expect(visibleTracks(['t1', 't2', 't3'], new Set(['t2']))).toEqual(['t1', 't3']);
  });
});

describe('alternatives (DOC-02)', () => {
  const altClips: AlternativeClipSpec[] = [
    { kind: 'AUDIO', assetId: 'asset-9', startTicks: '0', lengthTicks: BAR, offsetTicks: '0' },
    { kind: 'MIDI', startTicks: BAR, lengthTicks: BAR, offsetTicks: '0' },
  ];

  it('captureTrackAlternative snapshots the live clip list', () => {
    const a = captureTrackAlternative('a1', 'trk1', 'v2', [
      audioClip({ clipId: 'x', startTicks: '5', offsetTicks: '7' }),
    ]);
    expect(a.clips[0]).toMatchObject({ startTicks: '5', offsetTicks: '7', assetId: 'asset-1' });
  });

  it('applyTrackAlternativeOps removes live clips then inserts spec in one tx', () => {
    const alt: TrackAlternative = {
      alternativeId: 'a1', trackId: 'trk1', name: 'take B', clips: altClips,
    };
    const live = [audioClip({ clipId: 'old1' }), audioClip({ clipId: 'old2' })];
    const plan = applyTrackAlternativeOps(alt, live, mint);
    const removes = plan.ops.filter((o) => 'RemoveClipOp' in o);
    const inserts = plan.ops.filter((o) => 'InsertAudioClipOp' in o || 'InsertMidiClipOp' in o);
    expect(removes).toHaveLength(2);
    expect(inserts).toHaveLength(2);
    expect(plan.removedClipIds).toEqual(['old1', 'old2']);
    expect(plan.insertedClipIds).toHaveLength(2);
    expect(plan.transactionId).toBeTruthy();
  });

  it('applyProjectAlternativeOps spans tracks in one transaction', () => {
    const alt = {
      alternativeId: 'p1',
      name: 'arrangement B',
      tracks: {
        trk1: [altClips[0]!],
        trk2: [{ kind: 'MIDI', startTicks: '0', lengthTicks: '100', offsetTicks: '0' } as AlternativeClipSpec],
      },
    };
    const live = [
      audioClip({ clipId: 'a', trackId: 'trk1' }),
      audioClip({ clipId: 'b', trackId: 'trk2' }),
      audioClip({ clipId: 'c', trackId: 'trk3' }),
    ];
    const plan = applyProjectAlternativeOps(alt, live, mint);
    // trk3 untouched
    expect(plan.removedClipIds.sort()).toEqual(['a', 'b']);
    expect(plan.insertedClipIds).toHaveLength(2);
  });

  it('rejects an audio spec without an asset id', () => {
    const bad: TrackAlternative = {
      alternativeId: 'bad', trackId: 't', name: 'x',
      clips: [{ kind: 'AUDIO', startTicks: '0', lengthTicks: '1', offsetTicks: '0' }],
    };
    expect(() => applyTrackAlternativeOps(bad, [], mint)).toThrow();
  });
});

describe('freeze + bounded streaming (ENG-05, TIME-06; T68)', () => {
  it('freezeRenderRegion covers clip span + fixed tail; empty track → null', () => {
    const clips = [
      audioClip({ clipId: 'a', startTicks: '1000', lengthTicks: '2000' }),
      audioClip({ clipId: 'b', startTicks: '5000', lengthTicks: '1000' }),
    ];
    expect(
      freezeRenderRegion(clips, { kind: 'fixed', ticks: '500' }),
    ).toEqual({ startTicks: '1000', lengthTicks: '5500' });
    expect(freezeRenderRegion([], { kind: 'fixed', ticks: '500' })).toBeNull();
  });

  it('freezeApplyOps removes sources and inserts the bounce clip in one tx', () => {
    const spec = {
      freezeId: 'f1', trackId: 'trk1',
      tail: { kind: 'fixed' as const, ticks: '100' },
      preserveSources: false,
    };
    const r = freezeApplyOps(
      spec,
      [audioClip({ clipId: 's1' }), audioClip({ clipId: 's2' })],
      'bounce-asset',
      { startTicks: '0', lengthTicks: '9000' },
      mint,
    );
    expect(r.removedClipIds).toEqual(['s1', 's2']);
    const ins = r.ops.find((o) => 'InsertAudioClipOp' in o) as {
      InsertAudioClipOp: { asset_id: string; clip_id: string };
    };
    expect(ins.InsertAudioClipOp.asset_id).toBe('bounce-asset');
    expect(r.bounceClipId).toBe(ins.InsertAudioClipOp.clip_id);
  });

  it('stream cache hits inside a resident tile, misses outside, evicts LRU past capacity', () => {
    let cache = makeStreamCache('asset-1', 2);
    const w = { startTicks: '0', lengthTicks: '100' };
    let r = requestWindow(cache, w, 1);
    expect(r.resident).toBe(false);
    cache = tileResident(r.cache, w, 1);
    r = requestWindow(cache, w, 2);
    expect(r.resident).toBe(true);
    // two more misses fill capacity and evict the oldest resident
    r = requestWindow(r.cache, { startTicks: '200', lengthTicks: '100' }, 3);
    cache = tileResident(r.cache, { startTicks: '200', lengthTicks: '100' }, 3);
    r = requestWindow(cache, { startTicks: '400', lengthTicks: '100' }, 4);
    expect(r.evictedTile?.region.startTicks).toBe('0');
    expect(r.cache.evictions).toBe(1);
  });

  it('tileWindows covers the asset with bounded fixed windows', () => {
    const wins = tileWindows('a', '250', '100');
    expect(wins).toEqual([
      { startTicks: '0', lengthTicks: '100' },
      { startTicks: '100', lengthTicks: '100' },
      { startTicks: '200', lengthTicks: '50' },
    ]);
  });
});

describe('sendPlan commit path', () => {
  it('sends ops under one transactionId and stops at a failed receipt', async () => {
    const t = new FakeTransport();
    t.respond('send_command', (args) => {
      const dto = (args as { dto: { transaction_id: string; op: Record<string, unknown> } }).dto;
      const failed = 'RemoveClipOp' in dto.op;
      return {
        kind: 'CommandReceipt', command_id: 'c', transaction_id: dto.transaction_id,
        status: failed ? 'REJECTED' : 'APPLIED',
        error: failed ? 'NOT_FOUND' : 'NONE',
        revision: '1',
      };
    });
    const client = new VoidClient({ transport: t, ids: () => `cmd-${++seq}` });
    const plan = {
      transactionId: 'tx-1',
      ops: [
        { MoveClipOp: { clip_id: 'a', track_id: 't', start_ticks: '1' } },
        { RemoveClipOp: { clip_id: 'b' } },
        { MoveClipOp: { clip_id: 'c', track_id: 't', start_ticks: '1' } },
      ],
    };
    await expect(sendPlan(client, plan)).rejects.toThrow(/REJECTED/);
    // third op never sent
    expect(t.calls.filter((c) => c.cmd === 'send_command')).toHaveLength(2);
  });
});
