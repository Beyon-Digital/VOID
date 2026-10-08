import { describe, expect, it } from 'vitest';
import { FakeTransport, VoidClient } from 'void-client';
import type { ReadResponse } from 'void-client';
import {
  assertViewStateOnly,
  createStudioStore,
  makeViewKey,
  VIEW_PAGE_ITEM_MAX,
} from './store';
import { bindStudioClient, loadViewPage } from './bind';

function readPage(
  items: { object_id: string; summary_json: string }[],
  over: Partial<Omit<ReadResponse, 'items'>> = {},
) {
  return {
    kind: 'ReadResponse' as const,
    request_id: 'r',
    revision: '5',
    items,
    next_cursor: '',
    done: true,
    error: 'NONE' as const,
    ...over,
  };
}
function mk(s: string) {
  return { object_id: s, summary_json: '{}' };
}

describe('studio store — view state only', () => {
  it('initial state passes the no-song-clone invariant', () => {
    const s = createStudioStore();
    expect(() => assertViewStateOnly(s.getState())).not.toThrow();
  });

  it('invariant rejects a planted song clone (PCM buffer + extra key)', () => {
    const s = createStudioStore();
    const poisoned = {
      ...s.getState(),
      song: { tracks: [] },
      pcm: new Float32Array(8),
    };
    expect(() => assertViewStateOnly(poisoned)).toThrow();
    const poisoned2 = { ...s.getState(), undoStack: [{}] };
    expect(() => assertViewStateOnly(poisoned2)).toThrow();
  });

  it('view cache is bounded: never exceeds VIEW_PAGE_ITEM_MAX', () => {
    const s = createStudioStore();
    const key = makeViewKey('TRACK_LIST');
    // merge a first page at the cap…
    s.getState().actions.mergeReadPage(
      key,
      readPage(Array.from({ length: VIEW_PAGE_ITEM_MAX }, (_, i) => mk(`t${i}`)), {
        done: false,
        next_cursor: 'c',
      }),
    );
    // …and a second page — it must be truncated, not accumulated.
    s.getState().actions.mergeReadPage(key, readPage([mk('extra')]));
    const entry = s.getState().views[key];
    expect(entry.items.length).toBe(VIEW_PAGE_ITEM_MAX);
    expect(entry.truncated).toBe(true);
    expect(entry.received).toBe(VIEW_PAGE_ITEM_MAX + 1);
    expect(() => assertViewStateOnly(s.getState())).not.toThrow();
  });

  it('selection cascades: track clears clip/note, clip keeps track', () => {
    const s = createStudioStore();
    s.getState().actions.selectTrack('trk-1');
    expect(s.getState().selection).toEqual({
      trackId: 'trk-1',
      clipId: null,
      noteId: null,
    });
    s.getState().actions.selectClip('trk-1', 'clip-9');
    expect(s.getState().selection).toEqual({
      trackId: 'trk-1',
      clipId: 'clip-9',
      noteId: null,
    });
    s.getState().actions.selectNote('trk-1', 'clip-9', 'n-3');
    expect(s.getState().selection.noteId).toBe('n-3');
    s.getState().actions.selectTrack('trk-2');
    expect(s.getState().selection).toEqual({
      trackId: 'trk-2',
      clipId: null,
      noteId: null,
    });
  });

  it('project change clears all projections and revision', () => {
    const s = createStudioStore();
    const key = makeViewKey('TRACK_LIST');
    s.getState().actions.mergeReadPage(key, readPage([mk('a')]));
    s.getState().actions.setProject('proj-1', '3');
    expect(s.getState().revision).toBe('3');
    s.getState().actions.mergeReadPage(key, readPage([mk('b')]));
    s.getState().actions.setProject('proj-2');
    expect(s.getState().views).toEqual({});
    expect(s.getState().revision).toBe('0');
  });

  it('epoch change clears projections; same-epoch update keeps them', () => {
    const s = createStudioStore();
    s.getState().actions.setEngine({ attached: true, workerId: 'w', epoch: '7' });
    const key = makeViewKey('TRACK_LIST');
    s.getState().actions.mergeReadPage(key, readPage([mk('a')]));
    s.getState().actions.setEngine({ attached: true, workerId: 'w', epoch: '7' });
    expect(Object.keys(s.getState().views)).toHaveLength(1);
    s.getState().actions.setEngine({ attached: true, workerId: 'w', epoch: '8' });
    expect(s.getState().views).toEqual({});
  });

  it('telemetry stores latest clock + per-track meters (replaced, not appended)', () => {
    const s = createStudioStore();
    const a = s.getState().actions;
    a.applyTelemetry({
      kind: 'ClockSnapshot',
      project_id: 'p',
      engine_epoch: '1',
      timeline_sample: '10',
      device_sample_counter: '20',
      sample_rate: 48000,
      transport: 'PLAYING',
      loop_start_ticks: '0',
      loop_end_ticks: '0',
      tempo_map_revision: '1',
      sequence: '1',
      host_clock_ns: '1',
    });
    a.applyTelemetry({
      kind: 'MeterFrame',
      project_id: 'p',
      engine_epoch: '1',
      track_id: 'trk-1',
      peak_l: 0.5,
      peak_r: 0.4,
      rms_l: 0.2,
      rms_r: 0.18,
      clipped: false,
      sequence: '1',
    });
    a.applyTelemetry({
      kind: 'MeterFrame',
      project_id: 'p',
      engine_epoch: '1',
      track_id: 'trk-1',
      peak_l: 0.9,
      peak_r: 0.8,
      rms_l: 0.3,
      rms_r: 0.28,
      clipped: true,
      sequence: '2',
    });
    const t = s.getState().telemetry;
    expect(t.clock?.timeline_sample).toBe('10');
    expect(Object.keys(t.meters)).toEqual(['trk-1']);
    expect(t.meters['trk-1'].peak_l).toBe(0.9);
    expect(t.meters['trk-1'].clipped).toBe(true);
  });
});

describe('client binding', () => {
  it('receipts update revision; engine-lost detaches and clears projections', async () => {
    const t = new FakeTransport();
    const client = new VoidClient({ transport: t, ids: (() => 'id-1') });
    const s = createStudioStore();
    const unbind = bindStudioClient(s, client);
    await client.start();

    t.emit('void://control', {
      kind: 'CommandReceipt',
      command_id: 'c',
      status: 'APPLIED',
      error: 'NONE',
      revision: '42',
      engine_epoch: '1',
    });
    expect(s.getState().revision).toBe('42');

    s.getState().actions.mergeReadPage('TRACK_LIST|', readPage([mk('x')]));
    t.emit('void://engine-lost', { worker_id: 'w-1' });
    expect(s.getState().engine.attached).toBe(false);
    expect(s.getState().views).toEqual({});
    unbind();
  });

  it('undo goes through send_command as UndoOp — no local undo stack', async () => {
    const t = new FakeTransport();
    const client = new VoidClient({ transport: t, ids: (() => 'id-1') });
    t.respond('send_command', () => ({
      kind: 'CommandReceipt',
      command_id: 'id-1',
      status: 'APPLIED',
      error: 'NONE',
      revision: '2',
    }));
    const r = await client.undo();
    expect(r.status).toBe('APPLIED');
    expect(
      (t.calls[0].args!.dto as { op: Record<string, unknown> }).op,
    ).toEqual({ UndoOp: { transaction_id: '' } });
    const s = createStudioStore();
    expect('undoStack' in s.getState()).toBe(false);
    expect(() => assertViewStateOnly(s.getState())).not.toThrow();
  });

  it('loadViewPage stores a bounded page under its view key', async () => {
    const t = new FakeTransport();
    const client = new VoidClient({
      transport: t,
      ids: (() => 'id-1'),
      projectId: 'p-1',
    });
    t.respond('read_view', () => readPage([mk('trk-1'), mk('trk-2')]));
    const s = createStudioStore();
    await loadViewPage(s, client, 'TRACK_LIST');
    const key = makeViewKey('TRACK_LIST');
    const entry = s.getState().views[key];
    expect(entry.items.map((i) => i.object_id)).toEqual(['trk-1', 'trk-2']);
    expect(entry.done).toBe(true);
    expect(() => assertViewStateOnly(s.getState())).not.toThrow();
  });
});
