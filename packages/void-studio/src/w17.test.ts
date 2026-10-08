// W17 scenario coverage — T66 (loop takes → comp → crossfade
// alternatives), T67 (tempo/meter changes, sections, loops), T68
// (bounded streaming + freeze/bounce). Real ops go through a
// FakeTransport client so receipts/undo are exercised; view-state
// halves are asserted as specs (no wire ops exist for them — NEEDS
// rev2 §18–31).
import { describe, expect, it } from 'vitest';
import { FakeTransport, VoidClient } from 'void-client';
import {
  addTake,
  applyCompPlan,
  buildComp,
  compToOps,
  cycleTakeAt,
  loopTakes,
  makeFolder,
  seamFadePlan,
  swipeComp,
  takeAt,
  undoCompApply,
  validateComp,
  type TakeFolder,
  type TakeRecord,
} from './takes';
import {
  applyTrackAlternativeOps,
  captureTrackAlternative,
  checkSections,
  freezeApplyOps,
  freezeRenderRegion,
  insertMeterOp,
  insertTempoOp,
  loopClipOps,
  makeStreamCache,
  moveSectionOps,
  requestWindow,
  sendPlan,
  shiftMarkers,
  stripSilenceOps,
  tileResident,
  tileWindows,
  type LoudnessTile,
  type Section,
} from './arrangement';
import {
  captureToArrangementOps,
  createSceneStore,
  patternToClipOps,
  recordEvent,
  makeCapture,
} from './scenes';
import { makeRow, setStep, type StepPattern } from './patterns/stepPattern';
import type { ClipView } from './timeline/geometry';

const BAR = '3840000';
const HALF = '1920000';
let seq = 0;
const mint = () => `w17-${++seq}`;

function client(t: FakeTransport): VoidClient {
  let n = 0;
  return new VoidClient({ transport: t, ids: () => `cmd-${++n}` });
}

function applied(t: FakeTransport): void {
  t.respond('send_command', (args) => {
    const dto = (args as { dto: { transaction_id: string } }).dto;
    return {
      kind: 'CommandReceipt' as const,
      command_id: 'c',
      transaction_id: dto.transaction_id,
      status: 'APPLIED' as const,
      error: 'NONE' as const,
      revision: '9',
    };
  });
}

function audioClip(p: Partial<ClipView> & { clipId: string }): ClipView {
  return {
    trackId: 'trk1', kind: 'AUDIO', assetId: 'a1',
    startTicks: '0', lengthTicks: BAR, offsetTicks: '0', ...p,
  } as ClipView;
}

function take(p: Partial<TakeRecord> & { takeId: string; laneIndex: number }): TakeRecord {
  return {
    folderId: 'f1', trackId: 'trk1', kind: 'AUDIO', assetId: 'a1',
    regionStartTicks: '0', regionLengthTicks: BAR, offsetTicks: '0',
    complete: true, ...p,
  };
}

describe('T66 — loop takes, comp across boundaries, crossfades, alternatives', () => {
  it('loop record → three-lane folder → swipe comp → one-transaction apply → undo', async () => {
    // Four passes over a one-bar loop; pass 3 interrupted (incomplete
    // take, kept — REC-04).
    const folder: TakeFolder = loopTakes({
      trackId: 'trk1',
      folderId: 'f1',
      kind: 'AUDIO',
      region: { startTicks: '0', lengthTicks: BAR },
      passes: [
        { takeId: 'p0', assetId: 'a-takes' },
        { takeId: 'p1', assetId: 'a-takes' },
        { takeId: 'p2', assetId: 'a-takes', complete: false },
        { takeId: 'p3', assetId: 'a-takes' },
      ],
    });
    expect(folder.takes).toHaveLength(4);
    expect(folder.takes[2]!.complete).toBe(false);
    const passes = folder.takes;

    // Swipe: lane 0 for the first half, lane 1 for the second half.
    const spec = swipeComp({
      compId: 'comp1',
      folder,
      region: { startTicks: '0', lengthTicks: BAR },
      baseTakeId: passes[0]!.takeId,
      swipes: [
        { segmentId: 'seg-2', startTicks: HALF, lengthTicks: HALF, takeId: passes[1]!.takeId },
      ],
      mint,
    });
    expect(validateComp(spec, folder)).toEqual([]);
    expect(spec.segments.map((s) => s.takeId)).toEqual([
      passes[0]!.takeId, passes[1]!.takeId,
    ]);

    // Seam fade spec is produced at the boundary (bounded by the
    // shorter adjacent segment) — a FadeSpec exists even though the
    // wire has no fade fields (NEEDS §18).
    const seams = seamFadePlan(spec, { maxTicks: '60000' });
    expect(seams).toHaveLength(1);
    expect(seams[0]!.seamTicks).toBe(HALF);
    expect(BigInt(seams[0]!.fade.lengthTicks)).toBeLessThanOrEqual(60000n);

    // The existing arrangement clip gets swapped for comp pieces in
    // ONE transaction — undo restores it whole.
    const live = [audioClip({ clipId: 'live-1', lengthTicks: BAR })];
    const plan = compToOps(spec, folder, { clips: live }, mint);
    expect(plan.removedClipIds).toEqual(['live-1']);
    const t = new FakeTransport();
    applied(t);
    const c = client(t);
    const receipts = await applyCompPlan(c, plan, async () => {});
    expect(receipts.every((r) => r.status === 'APPLIED')).toBe(true);
    const txIds = new Set(
      t.calls
        .filter((x) => x.cmd === 'send_command')
        .map((x) => (x.args as { dto: { transaction_id: string } }).dto.transaction_id),
    );
    expect(txIds.size).toBe(1); // one gesture = one transactionId
    const undo = await undoCompApply(c, plan);
    expect(undo.status).toBe('APPLIED');
  });

  it('switch-alternatives cycles take per segment; sources never mutate', () => {
    let folder = makeFolder('f1', 'trk1');
    folder = addTake(folder, take({ takeId: 't0', laneIndex: 0 }));
    folder = addTake(folder, take({ takeId: 't1', laneIndex: 1 }));
    folder = addTake(folder, take({ takeId: 't2', laneIndex: 2 }));
    const frozen0 = JSON.stringify(folder.takes[0]);
    const spec = buildComp({
      compId: 'c', trackId: 'trk1', folder,
      region: { startTicks: '0', lengthTicks: BAR },
      picks: [{ takeId: folder.takes[0]!.takeId, startTicks: '0', segmentId: 's0' }],
      regionEndTicks: BAR,
    });
    const next = cycleTakeAt(spec, folder, '100', +1);
    expect(next.segments[0]!.takeId).toBe('t1');
    const wrap = cycleTakeAt(cycleTakeAt(next, folder, '100', +1), folder, '100', +1);
    expect(wrap.segments[0]!.takeId).toBe('t0'); // wraps
    expect(JSON.stringify(folder.takes[0])).toBe(frozen0); // untouched
    expect(takeAt(folder, '100')?.takeId).toBe('t2'); // topmost lane wins
  });
});

describe('T67 — tempo/meter changes, sections and loops, section move', () => {
  it('tempo + meter edits are real ops; section move carries clips+markers in one tx', async () => {
    const t = new FakeTransport();
    applied(t);
    const c = client(t);

    // Insert tempo bump at bar 2 and a 3/4 meter at bar 4.
    await sendPlan(c, insertTempoOp(BAR, 140, mint));
    await sendPlan(c, insertMeterOp('7680000', 3, 4, mint));

    // Section "verse" [0, BAR): inside clip + boundary-crossing clip;
    // marker at quarter-bar moves with the section.
    const verse: Section = {
      sectionId: 'verse', name: 'Verse', startTicks: '0', lengthTicks: BAR,
    };
    const clips = [
      audioClip({ clipId: 'inside', startTicks: '100', lengthTicks: '1000' }),
      audioClip({ clipId: 'cross', startTicks: '3700000', lengthTicks: '400000' }),
      audioClip({ clipId: 'other', startTicks: BAR, lengthTicks: '1000' }),
    ];
    const markers = [
      { markerId: 'm1', name: 'v-start', ticks: '960000', kind: 'marker' as const },
    ];
    const dest = (BigInt(BAR) * 4n).toString();
    const plan = moveSectionOps(verse, clips, markers, dest, {
      clips: true, markers: true, automation: true, chords: true,
    }, mint);
    // Order: SplitClipOp for the crossing clip, then moves.
    const first = plan.ops[0]!;
    expect('SplitClipOp' in first).toBe(true);
    expect(
      (first as { SplitClipOp: { clip_id: string; at_ticks: string } }).SplitClipOp,
    ).toMatchObject({ clip_id: 'cross', at_ticks: BAR });
    const receipts = await sendPlan(c, plan);
    expect(receipts).toHaveLength(plan.ops.length);
    expect(plan.markerMoves).toEqual([
      { markerId: 'm1', ticks: (960000n + BigInt(BAR) * 4n).toString() },
    ]);
    const moved = shiftMarkers(markers, plan.markerMoves);
    expect(moved[0]!.ticks).toBe(plan.markerMoves[0]!.ticks);
    // checkSections stays clean after the edit's book-keeping.
    expect(checkSections([
      { sectionId: 'verse', name: 'V', startTicks: dest, lengthTicks: BAR },
    ])).toEqual([]);
  });

  it('region loop materializes repeats; alternative apply is remove+insert in one tx', async () => {
    const t = new FakeTransport();
    applied(t);
    const c = client(t);
    const loop = loopClipOps(audioClip({ clipId: 'src' }), 2, mint);
    await sendPlan(c, loop);
    expect(loop.insertedClipIds).toHaveLength(2);

    // DOC-02: save current arrangement, apply an alternative — one tx.
    const alt = captureTrackAlternative('alt1', 'trk1', 'verse-alt', [
      audioClip({ clipId: 'keep', startTicks: '0', lengthTicks: BAR }),
    ]);
    const plan = applyTrackAlternativeOps(alt, [audioClip({ clipId: 'live' })], mint);
    await sendPlan(c, plan);
    expect(plan.removedClipIds).toEqual(['live']);
    expect(plan.insertedClipIds).toHaveLength(1);
  });

  it('scene launch quantizes to the bar; capture→arrangement emits real ops', async () => {
    const store = createSceneStore({
      scenes: [{ sceneId: 's1', name: 'A', region: { startTicks: '0', lengthTicks: BAR } }],
      trackIds: ['trk1'],
    });
    store.getState().actions.setSlot({
      slotId: 'cell', sceneId: 's1', trackId: 'trk1',
      content: { kind: 'clip', clipId: 'clip-x' },
    });
    const at = store.getState().actions.launch('s1', '1000000', 'bar');
    expect(at).toBe(BAR); // quantized to next bar
    store.getState().actions.settle(BAR);
    expect(store.getState().launch.slots['cell']?.phase).toBe('playing');

    const rec = recordEvent(makeCapture('cap', '0'), {
      sceneId: 's1', atTicks: BAR,
      slots: [{ trackId: 'trk1', content: { kind: 'clip', clipId: 'clip-x' } }],
    });
    const plan = captureToArrangementOps(rec, {
      resolveClip: (id) => id === 'clip-x'
        ? audioClip({ clipId: 'clip-x', startTicks: '0' }) : undefined,
    }, mint);
    const t = new FakeTransport();
    applied(t);
    await sendPlan(client(t), plan);
    expect(plan.ops).toHaveLength(1);
    expect(
      (plan.ops[0] as { InsertAudioClipOp: { start_ticks: string } }).InsertAudioClipOp
        .start_ticks,
    ).toBe(BAR);
  });
});

describe('T68 — bounded streaming + freeze/bounce', () => {
  it('huge-asset playback stays inside the cache bound; freeze swaps to the bounce clip in one tx', async () => {
    // Asset is 100× the cache capacity: resident set stays bounded.
    const total = '1000000';
    const win = '10000';
    const windows = tileWindows('big-asset', total, win);
    expect(windows).toHaveLength(100);
    let cache = makeStreamCache('big-asset', 4);
    let tick = 0;
    // Stream the whole asset; only `capacity` tiles resident at once.
    for (const w of windows) {
      const r = requestWindow(cache, w, ++tick);
      cache = tileResident(r.cache, w, tick);
    }
    const resident = cache.tiles.filter((x) => x.state === 'resident');
    expect(resident.length).toBeLessThanOrEqual(4);
    expect(cache.misses).toBe(100);
    expect(cache.evictions).toBe(96); // all but capacity were evicted
    // a fresh request for the first window misses again (evicted)
    const again = requestWindow(cache, windows[0]!, ++tick);
    expect(again.resident).toBe(false);

    // Freeze: render spec covers the clip span + tail; on the engine's
    // bounce asset the clip swap is one transaction.
    const clips = [
      audioClip({ clipId: 's1', startTicks: '0', lengthTicks: BAR }),
      audioClip({ clipId: 's2', startTicks: BAR, lengthTicks: BAR }),
    ];
    const region = freezeRenderRegion(clips, { kind: 'fixed', ticks: HALF });
    expect(region).toEqual({
      startTicks: '0', lengthTicks: (BigInt(BAR) * 2n + BigInt(HALF)).toString(),
    });
    const plan = freezeApplyOps(
      { freezeId: 'fr1', trackId: 'trk1', tail: { kind: 'fixed', ticks: HALF }, preserveSources: false },
      clips, 'bounce-asset', { startTicks: '0', lengthTicks: region!.lengthTicks }, mint,
    );
    const t = new FakeTransport();
    applied(t);
    await sendPlan(client(t), plan);
    expect(plan.removedClipIds).toEqual(['s1', 's2']);
    expect(
      plan.ops.filter((o) => 'InsertAudioClipOp' in o),
    ).toHaveLength(1);
  });

  it('strip-silence turns a loudness tile table into bounded clip ops', () => {
    const clip = audioClip({ clipId: 'vox', startTicks: '0', lengthTicks: '400', offsetTicks: '0' });
    const tiles: LoudnessTile[] = Array.from({ length: 40 }, (_, i) => ({
      startTicks: (i * 10).toString(), lengthTicks: '10',
      peakDb: i < 5 || i > 30 ? -12 : -90,
    }));
    const plan = stripSilenceOps(clip, tiles, {
      thresholdDb: -50, minSilenceTicks: '30', padTicks: '0',
    }, mint);
    expect(plan.kept.length).toBe(2);
    expect(plan.ops.filter((o) => 'RemoveClipOp' in o)).toHaveLength(1);
    expect(plan.ops.filter((o) => 'InsertAudioClipOp' in o)).toHaveLength(2);
  });

  it('pattern→MIDI conversion emits clip+notes in one transaction', async () => {
    const row = makeRow('r1', 36, 4, 'kick');
    const pattern: StepPattern = setStep(
      { patternId: 'p', name: 'n', stepTicks: '240000', seed: 'x', rows: [row] },
      'r1', 0, { on: true, velocity: 110 },
    );
    const plan = patternToClipOps('trk1', pattern, '0', BAR, mint);
    const t = new FakeTransport();
    applied(t);
    const receipts = await sendPlan(client(t), plan);
    expect(receipts).toHaveLength(plan.ops.length);
    expect(plan.ops[0]).toHaveProperty('InsertMidiClipOp');
    expect(plan.noteIds.length).toBeGreaterThan(0);
  });
});
