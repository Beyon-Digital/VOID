// T38 + T39 — workspace invariants (store purity, no song clones, bounded
// caches, view-state-only discipline) and accessibility/state-error
// surface (workspace defs carry ARIA labels + shortcuts; transport bar
// state honestly reports '—' when detached; loop range rejects malformed
// input before it hits the wire).

import { describe, expect, it } from 'vitest';
import { FakeTransport, VoidClient } from 'void-client';
import type { ClockSnapshot, PersistentCommandDto } from 'void-client';
import {
  createStudioStore,
  assertViewStateOnly,
  VIEW_PAGE_ITEM_MAX,
  type ReadViewEntry,
} from '../store';
import {
  WORKSPACES,
  WORKSPACE_ORDER,
  isWorkspaceId,
  workspaceForShortcut,
} from './workspaces';
import {
  createEditorStore,
  assertEditorViewState,
  initialEditorState,
} from './editorStore';
import { transportBarState, setLoopRange, setLiveCycle } from './transportBar';

const clock = (over: Partial<ClockSnapshot> = {}): ClockSnapshot => ({
  kind: 'ClockSnapshot',
  project_id: 'p',
  engine_epoch: '1',
  timeline_sample: '0',
  device_sample_counter: '0',
  sample_rate: 48000,
  transport: 'STOPPED',
  loop_start_ticks: '0',
  loop_end_ticks: '0',
  tempo_map_revision: '0',
  sequence: '0',
  host_clock_ns: '0',
  ...over,
});

// -- workspace defs ---------------------------------------------------------------

describe('workspace defs (accessibility surface)', () => {
  it('exposes three labelled workspaces in order with digit shortcuts', () => {
    expect(WORKSPACE_ORDER).toEqual(['compose', 'arrange', 'mix']);
    for (const id of WORKSPACE_ORDER) {
      const w = WORKSPACES[id];
      expect(w.ariaLabel.length).toBeGreaterThan(10);
      expect(w.shortcut).toMatch(/^[1-3]$/);
      expect(['piano-roll', 'timeline', 'mixer']).toContain(w.primaryRegion);
    }
    expect(isWorkspaceId('arrange')).toBe(true);
    expect(isWorkspaceId('nope')).toBe(false);
    expect(workspaceForShortcut('2')).toBe('arrange');
    expect(workspaceForShortcut('9')).toBeNull();
  });
});

// -- transport bar state ------------------------------------------------------------

describe('transportBarState', () => {
  it('reports — (no fake state) when there is no clock or the engine is detached', () => {
    expect(transportBarState(undefined, false).transport).toBe('—');
    expect(transportBarState(undefined, false).timelineSample).toBeNull();
    expect(transportBarState(clock(), false).transport).toBe('—');
  });

  it('derives the cycle indicator from ClockSnapshot loop bounds', () => {
    const st = transportBarState(
      clock({ transport: 'PLAYING', loop_start_ticks: '0', loop_end_ticks: '3840000' }),
      true,
    );
    expect(st.transport).toBe('PLAYING');
    expect(st.cycle).toEqual({
      active: true,
      startTicks: '0',
      endTicks: '3840000',
    });
    expect(transportBarState(clock(), true).cycle.active).toBe(false);
  });
});

// -- loop range --------------------------------------------------------------------

describe('setLoopRange', () => {
  it('rejects a malformed enabled range client-side (nothing hits the wire)', async () => {
    const transport = new FakeTransport();
    const client = new VoidClient({ transport, ids: () => 'id' });
    await expect(
      setLoopRange(client, { startTicks: '3840000', endTicks: '0', enabled: true }),
    ).rejects.toThrow(/end > start/);
    expect(transport.calls).toHaveLength(0);
  });

  it('issues SetLoopRangeOp with decimal-string ticks', async () => {
    const transport = new FakeTransport();
    transport.respond('send_command', (args) => ({
      kind: 'CommandReceipt',
      command_id: (args!.dto as PersistentCommandDto).command_id,
      status: 'APPLIED',
      error: 'NONE',
      revision: '9',
    }));
    const client = new VoidClient({ transport, ids: () => 'id' });
    const out = await setLoopRange(
      client,
      { startTicks: '0', endTicks: '7680000', enabled: true },
      { transactionId: 'tx-loop' },
    );
    expect(out.receipt.status).toBe('APPLIED');
    const dto = transport.calls[0].args!.dto as PersistentCommandDto;
    expect(dto.op).toEqual({
      SetLoopRangeOp: { start_ticks: '0', end_ticks: '7680000', enabled: true },
    });
    expect(dto.transaction_id).toBe('tx-loop');
  });

  it('setLiveCycle routes through send_transport SET_CYCLE', async () => {
    const transport = new FakeTransport();
    transport.respond('send_transport', (args) => ({
      kind: 'TransportAck',
      request_id: (args!.dto as { request_id: string }).request_id,
      ok: true,
      error: 'NONE',
    }));
    const client = new VoidClient({ transport, ids: () => 'id' });
    await setLiveCycle(client, '0', '960000');
    const dto = transport.calls[0].args!.dto as {
      op: string;
      cycle_start_ticks: string;
      cycle_end_ticks: string;
    };
    expect(dto.op).toBe('SET_CYCLE');
    expect(dto.cycle_start_ticks).toBe('0');
    expect(dto.cycle_end_ticks).toBe('960000');
    await expect(setLiveCycle(client, '960000', '0')).rejects.toThrow();
  });
});

// -- editor store invariants ---------------------------------------------------------

describe('editor store invariants', () => {
  it('passes the view-state-only scan on initial + mutated state', () => {
    const store = createEditorStore();
    assertEditorViewState(store.getState());
    store.getState().actions.selectClips('trk-1', ['c1', 'c2', 'c1']);
    store.getState().actions.setDrag({
      mode: 'move',
      clipId: 'c1',
      trackId: 'trk-1',
      transactionId: 'tx',
      startTicks: '0',
      lengthTicks: '960000',
    });
    assertEditorViewState(store.getState());
    expect(store.getState().clipSelection.clipIds).toEqual(['c1', 'c2']);
  });

  it('throws when a forbidden key sneaks in (PCM / song doc / undo stack)', () => {
    const bad = { ...initialEditorState(), songDoc: { clips: [] } };
    expect(() => assertEditorViewState(bad as never)).toThrow(/non-view key/);
    const pcm = { ...initialEditorState() } as Record<string, unknown>;
    (pcm as { drag?: unknown }).drag = {
      mode: 'move',
      clipId: 'c',
      trackId: 't',
      transactionId: 'tx',
      startTicks: '0',
      lengthTicks: '1',
      waveformData: new Float32Array(4), // forbidden field name + PCM buffer
    };
    expect(() => assertEditorViewState(pcm as never)).toThrow(/forbidden field name|binary/);
  });

  it('toggle selects without duplicating and clears independently', () => {
    const store = createEditorStore();
    const a = store.getState().actions;
    a.toggleClip('trk-1', 'c1');
    a.toggleClip('trk-1', 'c2');
    expect(store.getState().clipSelection.clipIds).toEqual(['c1', 'c2']);
    a.toggleClip('trk-1', 'c1');
    expect(store.getState().clipSelection.clipIds).toEqual(['c2']);
    // switching tracks resets the clip id set
    a.toggleClip('trk-2', 'c9');
    expect(store.getState().clipSelection).toEqual({
      trackId: 'trk-2',
      clipIds: ['c9'],
    });
    a.selectNotes('clip-1', ['n1']);
    a.clearClipSelection();
    expect(store.getState().noteSelection.noteIds).toEqual(['n1']);
    expect(store.getState().clipSelection.clipIds).toEqual([]);
  });
});

// -- bounded view cache (T38 memory discipline) ---------------------------------------

describe('view cache bounds', () => {
  it('caps a view at VIEW_PAGE_ITEM_MAX items and marks it truncated', () => {
    const store = createStudioStore();
    const page = (n: number) => ({
      kind: 'ReadResponse' as const,
      request_id: 'r',
      revision: '1',
      items: Array.from({ length: n }, (_, i) => ({
        object_id: `o${i}`,
        summary_json: '{}',
      })),
      next_cursor: 'c',
      done: false,
      error: 'NONE' as const,
    });
    store.getState().actions.mergeReadPage('CLIP_LIST||0|', page(VIEW_PAGE_ITEM_MAX - 1));
    store.getState().actions.mergeReadPage('CLIP_LIST||0|', page(10));
    const entry = store.getState().views['CLIP_LIST||0|'] as ReadViewEntry;
    expect(entry.items.length).toBe(VIEW_PAGE_ITEM_MAX);
    expect(entry.truncated).toBe(true);
    expect(entry.received).toBe(VIEW_PAGE_ITEM_MAX + 9);
    assertViewStateOnly(store.getState());
  });

  it('base store stays clean under assertViewStateOnly through edits', () => {
    const store = createStudioStore();
    store.getState().actions.selectTrack('t1');
    store.getState().actions.setViewport('0', '3840000');
    store.getState().actions.applyTelemetry(clock({ transport: 'PLAYING' }));
    assertViewStateOnly(store.getState());
    const poisoned = {
      ...store.getState(),
      views: {
        k: {
          items: [],
          revision: '0',
          nextCursor: '',
          done: true,
          truncated: false,
          received: 0,
          audioBuffer: new Float32Array(8),
        },
      },
    };
    expect(() => assertViewStateOnly(poisoned as never)).toThrow(/forbidden field name/);
  });
});
