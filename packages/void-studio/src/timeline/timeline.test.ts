// T36 — arrangement gestures: clip op payload shapes (through the real
// client onto FakeTransport), hit-testing/geometry math, the
// stale-revision retry path, and the no-song-clone invariant.

import { describe, expect, it } from 'vitest';
import { FakeTransport, VoidClient } from 'void-client';
import type { PersistentCommandDto } from 'void-client';
import { makeViewport } from '../viewport';
import { TICKS_PER_BAR_44, TICKS_PER_QUARTER } from '../viewport';
import { createStudioStore, makeViewKey } from '../store';
import {
  clipEndTicks,
  clipRect,
  hitTestClip,
  hitTestClips,
  layoutClips,
  parseClipItem,
  type ClipView,
} from './geometry';
import { gridStepTicks, snapTicks } from './snap';
import {
  duplicateClipOp,
  moveClipOp,
  removeClipOp,
  splitClipOp,
  trimClipOp,
} from './ops';
import { beginClipDrag, commitClipDrag, previewClipDrag } from './drag';
import { ClipEditor } from './clipEditor';
import type { SnapSettings } from '../workspaces/editorStore';

const SNAP: SnapSettings = {
  enabled: true,
  division: 'beat',
  beatsPerBar: 4,
  tempoMapRevision: '7',
};
const BAR = TICKS_PER_BAR_44.toString();
const Q = TICKS_PER_QUARTER.toString();

const clip = (over: Partial<ClipView> = {}): ClipView => ({
  clipId: 'clip-1',
  trackId: 'trk-1',
  kind: 'MIDI',
  name: 'riff',
  startTicks: BAR,
  lengthTicks: Q,
  offsetTicks: '0',
  ...over,
});

// -- geometry + hit-testing --------------------------------------------------

describe('timeline geometry + hit-testing', () => {
  const v = makeViewport('0', (4n * TICKS_PER_BAR_44).toString());
  const z = { ticksPerPixel: 24000 }; // 4px per quarter note

  it('projects clip rect from ticks', () => {
    const c = clip();
    const r = clipRect(c, v, z);
    expect(r.x).toBeCloseTo(Number(TICKS_PER_BAR_44) / 24000, 3); // 160px
    expect(r.w).toBeCloseTo(960000 / 24000, 3); // 40px
  });

  it('hit-tests body and both trim edges', () => {
    const c = clip();
    const x0 = 160; // start px
    const x1 = 200; // end px
    expect(hitTestClip(c, x0 + 2, v, z)).toBe('trim-start');
    expect(hitTestClip(c, x1 - 2, v, z)).toBe('trim-end');
    expect(hitTestClip(c, (x0 + x1) / 2, v, z)).toBe('body');
    expect(hitTestClip(c, x0 - 40, v, z)).toBe(null);
    expect(hitTestClip(c, x1 + 40, v, z)).toBe(null);
  });

  it('topmost (last painted) clip wins the hit', () => {
    const a = clip({ clipId: 'a' });
    const b = clip({ clipId: 'b' });
    const hit = hitTestClips([a, b], 180, v, z);
    expect(hit?.clip.clipId).toBe('b');
  });

  it('clips outside the viewport do not lay out', () => {
    const off = clip({ startTicks: (10n * TICKS_PER_BAR_44).toString() });
    expect(layoutClips([off], v, z)).toHaveLength(0);
    expect(layoutClips([clip()], v, z)).toHaveLength(1);
  });

  it('parseClipItem accepts the wire shape and drops junk', () => {
    const good = parseClipItem({
      object_id: 'clip:clip-9',
      summary_json: JSON.stringify({
        clip_id: 'clip-9',
        track_id: 'trk-2',
        start_ticks: '3840000',
        length_ticks: '960000',
        kind: 'MIDI',
        name: 'verse',
      }),
    });
    expect(good).toMatchObject({
      clipId: 'clip-9',
      trackId: 'trk-2',
      startTicks: '3840000',
      lengthTicks: '960000',
    });
    expect(
      parseClipItem({ object_id: 'x', summary_json: '{not json' }),
    ).toBeNull();
    expect(
      parseClipItem({
        object_id: 'x',
        summary_json: JSON.stringify({ track_id: 't', start_ticks: '0' }),
      }),
    ).toBeNull(); // missing clip id + length
    expect(
      parseClipItem({
        object_id: 'x',
        summary_json: JSON.stringify({
          clip_id: 'c',
          track_id: 't',
          start_ticks: '0',
          length_ticks: '0',
        }),
      }),
    ).toBeNull(); // zero-length clip is malformed
  });

  it('clipEndTicks stays bigint-exact past 2^53-ish inputs', () => {
    const c = clip({
      startTicks: '9007199254740992000',
      lengthTicks: '960000',
    });
    expect(clipEndTicks(c)).toBe('9007199254741952000');
  });
});

// -- snap --------------------------------------------------------------------

describe('snap grid', () => {
  it('quantizes to the division step, ties away from zero', () => {
    const beat: SnapSettings = { ...SNAP, division: 'beat' };
    // half a beat = 480000 → ties round away from zero (up).
    expect(snapTicks('480000', beat)).toBe('960000');
    expect(snapTicks('-480000', beat)).toBe('-960000');
    expect(snapTicks('479999', beat)).toBe('0');
    expect(snapTicks('960001', beat)).toBe('960000');
  });

  it('bar division uses beatsPerBar; disabled snap passes through', () => {
    const bar: SnapSettings = { ...SNAP, division: 'bar', beatsPerBar: 4 };
    expect(gridStepTicks(bar)).toBe(TICKS_PER_BAR_44);
    expect(snapTicks('4000000', bar)).toBe('3840000');
    expect(snapTicks('4000000', { ...bar, enabled: false })).toBe('4000000');
  });
});

// -- op payloads -------------------------------------------------------------

describe('clip op payload shapes', () => {
  it('MoveClipOp carries clip_id, track_id, start_ticks (decimal string)', () => {
    expect(moveClipOp('clip-1', 'trk-1', '3840000')).toEqual({
      MoveClipOp: {
        clip_id: 'clip-1',
        track_id: 'trk-1',
        start_ticks: '3840000',
      },
    });
  });

  it('TrimClipOp carries start/length/offset', () => {
    expect(trimClipOp('clip-1', '3840000', '480000', '96000')).toEqual({
      TrimClipOp: {
        clip_id: 'clip-1',
        start_ticks: '3840000',
        length_ticks: '480000',
        offset_ticks: '96000',
      },
    });
  });

  it('SplitClipOp carries at_ticks + new_clip_id; RemoveClipOp carries clip_id', () => {
    expect(splitClipOp('clip-1', '3840000', 'clip-2')).toEqual({
      SplitClipOp: { clip_id: 'clip-1', at_ticks: '3840000', new_clip_id: 'clip-2' },
    });
    expect(removeClipOp('clip-1')).toEqual({ RemoveClipOp: { clip_id: 'clip-1' } });
  });

  it('duplicate of a MIDI clip maps to InsertMidiClipOp at the new range', () => {
    expect(duplicateClipOp(clip(), 'clip-2')).toEqual({
      InsertMidiClipOp: {
        clip_id: 'clip-2',
        track_id: 'trk-1',
        start_ticks: BAR,
        length_ticks: Q,
      },
    });
  });

  it('duplicate of an audio clip needs asset_id and keeps offset', () => {
    const audio = clip({ kind: 'AUDIO', assetId: 'asset-1', offsetTicks: '120' });
    expect(duplicateClipOp(audio, 'clip-2')).toEqual({
      InsertAudioClipOp: {
        clip_id: 'clip-2',
        track_id: 'trk-1',
        asset_id: 'asset-1',
        start_ticks: BAR,
        length_ticks: Q,
        offset_ticks: '120',
      },
    });
    expect(() =>
      duplicateClipOp(clip({ kind: 'AUDIO' }), 'clip-2'),
    ).toThrow(/asset_id/);
  });
});

// -- drag state machine --------------------------------------------------------

describe('clip drag gestures', () => {
  it('move: preview follows snapped pointer, commit issues MoveClipOp', () => {
    const d = beginClipDrag(clip(), 'move', '4032000', 'tx-1');
    const preview = previewClipDrag(d, '4032000', SNAP);
    // grabbed at 3840000+192000; pointer back at same → stays at BAR
    expect(preview.startTicks).toBe(BAR);
    const moved = previewClipDrag(d, '4992000', SNAP); // +1 quarter
    expect(moved.startTicks).toBe((3840000n + 960000n).toString());
    expect(commitClipDrag(d, moved)).toEqual({
      MoveClipOp: { clip_id: 'clip-1', track_id: 'trk-1', start_ticks: '4800000' },
    });
  });

  it('trim-start adjusts start+length+offset consistently', () => {
    const d = beginClipDrag(clip(), 'trim-start', '3840000', 'tx-1');
    // Quarter-note snap so 4320000 (18 * 240000) is exactly on-grid —
    // a half-beat would snap to 4800000 under beat snapping.
    const quarterSnap: SnapSettings = { ...SNAP, division: '1/4' };
    const p = previewClipDrag(d, '4320000', quarterSnap); // +480000
    expect(p.startTicks).toBe('4320000');
    expect(p.lengthTicks).toBe((960000n - 480000n).toString());
    expect(p.offsetTicks).toBe('480000');
    expect(commitClipDrag(d, p)).toEqual({
      TrimClipOp: {
        clip_id: 'clip-1',
        start_ticks: '4320000',
        length_ticks: '480000',
        offset_ticks: '480000',
      },
    });
  });

  it('trim-end clamps to a positive length', () => {
    const d = beginClipDrag(clip(), 'trim-end', '3840000', 'tx-1');
    const p = previewClipDrag(d, '1000', SNAP); // drag past the start
    expect(parseInt(p.lengthTicks, 10)).toBeGreaterThan(0);
  });

  it('split clamps inside the clip and commits SplitClipOp', () => {
    const d = beginClipDrag(clip(), 'split', '3840000', 'tx-1');
    const p = previewClipDrag(d, '4100000', SNAP);
    const at = BigInt(p.atTicks!);
    expect(at).toBeGreaterThan(3840000n);
    expect(at).toBeLessThan(3840000n + 960000n);
    expect(commitClipDrag(d, p, 'clip-new')).toEqual({
      SplitClipOp: {
        clip_id: 'clip-1',
        at_ticks: p.atTicks,
        new_clip_id: 'clip-new',
      },
    });
  });
});

// -- stale revision retry -----------------------------------------------------

function receiptOf(dto: PersistentCommandDto, status: string, revision: string, error = 'NONE') {
  return {
    kind: 'CommandReceipt',
    command_id: dto.command_id,
    transaction_id: dto.transaction_id,
    status,
    error,
    revision,
    engine_epoch: dto.engine_epoch,
  };
}

describe('edit session — STALE_REVISION retry', () => {
  it('re-reads the view once and retries against the receipt revision', async () => {
    const transport = new FakeTransport();
    let calls = 0;
    transport.respond('send_command', (args) => {
      const dto = args!.dto as PersistentCommandDto;
      calls++;
      // First send expected rev 0 → stale; the retry must carry rev 5.
      return receiptOf(
        dto,
        calls === 1 ? 'REJECTED' : 'APPLIED',
        calls === 1 ? '5' : '6',
        calls === 1 ? 'STALE_REVISION' : 'NONE',
      );
    });
    const client = new VoidClient({ transport, ids: () => 'id-1' });
    const editor = new ClipEditor(client, () => 'new-id');
    const store = createStudioStore();
    let refreshed = 0;

    const res = await editor.moveClip('clip-1', 'trk-1', '3840000', 'tx-1', () => {
      refreshed++;
      return Promise.resolve(store);
    });

    expect(calls).toBe(2);
    expect(refreshed).toBe(1);
    expect(res.retried).toBe(true);
    expect(res.receipt.status).toBe('APPLIED');
    const second = transport.calls[1].args!.dto as PersistentCommandDto;
    expect(second.expected_revision).toBe('5');
    // one gesture → one transaction id across both sends
    expect(second.transaction_id).toBe('tx-1');
    expect(second.op).toEqual({
      MoveClipOp: { clip_id: 'clip-1', track_id: 'trk-1', start_ticks: '3840000' },
    });
  });

  it('does not retry a plain REJECTED and surfaces the error text', async () => {
    const transport = new FakeTransport();
    transport.respond('send_command', (args) =>
      receiptOf(args!.dto as PersistentCommandDto, 'REJECTED', '3', 'NOT_FOUND'),
    );
    const client = new VoidClient({ transport, ids: () => 'id-2' });
    const editor = new ClipEditor(client, () => 'new-id');
    const res = await editor.removeClip('clip-9', 'tx-2');
    expect(res.receipt.status).toBe('REJECTED');
    expect(res.retried).toBe(false);
    expect(res.errorText).toContain('NOT_FOUND');
    expect(transport.calls).toHaveLength(1);
  });
});

// -- clip paging + store invariant ---------------------------------------------

describe('clip paging', () => {
  it('pages CLIP_LIST with cursors into the bounded store cache', async () => {
    const transport = new FakeTransport();
    const pages = [
      {
        kind: 'ReadResponse',
        request_id: 'p1',
        revision: '2',
        items: [
          {
            object_id: 'clip:a',
            summary_json: JSON.stringify({
              clip_id: 'a',
              track_id: 'trk-1',
              start_ticks: '0',
              length_ticks: '960000',
            }),
          },
        ],
        next_cursor: 'cur-1',
        done: false,
        error: 'NONE',
      },
      {
        kind: 'ReadResponse',
        request_id: 'p2',
        revision: '2',
        items: [
          {
            object_id: 'clip:b',
            summary_json: JSON.stringify({
              clip_id: 'b',
              track_id: 'trk-1',
              start_ticks: '960000',
              length_ticks: '960000',
            }),
          },
          { object_id: 'junk', summary_json: '{bad' },
        ],
        next_cursor: '',
        done: true,
        error: 'NONE',
      },
    ];
    let i = 0;
    transport.respond('read_view', () => pages[Math.min(i++, pages.length - 1)]);

    const client = new VoidClient({ transport, ids: () => 'id-3' });
    const editor = new ClipEditor(client, () => 'new-id');
    const store = createStudioStore();
    const { clips, dropped } = await editor.loadTrackClips(store, 'trk-1');

    expect(clips.map((c) => c.clipId)).toEqual(['a', 'b']);
    expect(dropped).toBe(1);
    const entry = store.getState().views[makeViewKey('CLIP_LIST', 'trk-1', undefined, undefined)];
    expect(entry.items).toHaveLength(3); // raw items incl. the dropped one
    expect(entry.done).toBe(true);
    // second page requested with the cursor from the first
    expect(transport.calls[1].args!.dto).toMatchObject({ cursor: 'cur-1' });
  });
});
