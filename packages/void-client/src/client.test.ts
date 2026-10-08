import { describe, expect, it, vi } from 'vitest';
import { VoidClient } from './client';
import { FakeTransport } from './transport';
import {
  CommandReceipt,
  PersistentCommandDto,
  ReadRequestDto,
  ReadResponse,
} from './dto';
import { i64str, parseI64, u64str } from './i64';

function makeClient(transport: FakeTransport, ids?: () => string) {
  let n = 0;
  const gen =
    ids ??
    (() => `00000000-0000-4000-8000-${String(++n).padStart(12, '0')}`);
  return new VoidClient({ transport, ids: gen, projectId: 'proj-1' });
}

function appliedReceipt(over: Partial<CommandReceipt> = {}): CommandReceipt {
  return {
    kind: 'CommandReceipt',
    command_id: 'cmd-1',
    transaction_id: 'tx-1',
    status: 'APPLIED',
    error: 'NONE',
    revision: '7',
    engine_epoch: '3',
    message: 'applied by mock worker',
    payload_hash: 'deadbeef',
    ...over,
  };
}

describe('VoidClient.sendCommand DTO shape', () => {
  it('sends codec-shaped command: tagged op + string int64 fields', async () => {
    const t = new FakeTransport();
    const client = makeClient(t);
    t.respond('send_command', () => appliedReceipt());

    await client.sendCommand(
      { AddTrackOp: { track_id: 'trk-1', kind: 'MIDI', name: 'Keys' } },
      { commandId: 'cmd-42', transactionId: 'tx-42', engineEpoch: '3', expectedRevision: '6' },
    );

    expect(t.calls).toHaveLength(1);
    expect(t.calls[0].cmd).toBe('send_command');
    const dto = t.calls[0].args!.dto as PersistentCommandDto;
    expect(dto.command_id).toBe('cmd-42');
    expect(dto.transaction_id).toBe('tx-42');
    expect(dto.project_id).toBe('proj-1');
    // int64 fields serialize as decimal STRINGS, not numbers.
    expect(dto.engine_epoch).toBe('3');
    expect(typeof dto.engine_epoch).toBe('string');
    expect(dto.expected_revision).toBe('6');
    expect(typeof dto.expected_revision).toBe('string');
    // tagged union op: exactly one key
    expect(Object.keys(dto.op)).toEqual(['AddTrackOp']);
    expect(dto.op).toEqual({
      AddTrackOp: { track_id: 'trk-1', kind: 'MIDI', name: 'Keys' },
    });
  });

  it('keeps int64 values > 2^53 exact via bigint/string', async () => {
    const t = new FakeTransport();
    const client = makeClient(t);
    t.respond('send_command', () => appliedReceipt());

    const big = 9007199254740993n; // > MAX_SAFE_INTEGER
    await client.sendCommand(
      {
        InsertMidiClipOp: {
          clip_id: 'c1',
          track_id: 'trk-1',
          start_ticks: i64str(big),
          length_ticks: '18446744073709550',
        },
      },
      { expectedRevision: '18446744073709551615' },
    );

    const dto = t.calls[0].args!.dto as PersistentCommandDto;
    const op = (dto.op as { InsertMidiClipOp: { start_ticks: string; length_ticks: string } })
      .InsertMidiClipOp;
    expect(op.start_ticks).toBe('9007199254740993');
    expect(parseI64(op.start_ticks)).toBe(big);
    expect(op.length_ticks).toBe('18446744073709550');
    expect(dto.expected_revision).toBe('18446744073709551615');
  });

  it('rejects unsafe JS numbers for int64 fields', () => {
    expect(() => i64str(2 ** 53 + 1)).toThrow(/safe integer/);
    expect(() => i64str(1.5)).toThrow(/integer/);
    expect(() => i64str(NaN)).toThrow();
    expect(() => i64str('12a')).toThrow(/invalid i64/);
    expect(() => u64str('-5')).toThrow(/u64/);
    expect(i64str(-960000n)).toBe('-960000'); // pickup positions may be negative
  });

  it('mirrors container_dir at top level for lifecycle ops', async () => {
    const t = new FakeTransport();
    const client = makeClient(t);
    t.respond('send_command', () => appliedReceipt());

    await client.createProject({
      projectId: 'p-9',
      name: 'Demo',
      containerDir: '/tmp/demo.void',
    });
    const dto = t.calls[0].args!.dto as PersistentCommandDto;
    expect(dto.project_id).toBe('p-9');
    expect(dto.container_dir).toBe('/tmp/demo.void');
    expect(dto.op).toEqual({
      CreateProjectOp: expect.objectContaining({
        name: 'Demo',
        container_dir: '/tmp/demo.void',
      }),
    });
  });
});

describe('receipt handling', () => {
  it('surfaces DUPLICATE as a receipt, not an error', async () => {
    const t = new FakeTransport();
    const client = makeClient(t);
    t.respond('send_command', () =>
      appliedReceipt({ status: 'DUPLICATE', command_id: 'cmd-9' }),
    );
    const r = await client.sendCommand(
      { AddTrackOp: { track_id: 'trk', kind: 'AUDIO' } },
      { commandId: 'cmd-9' },
    );
    expect(r.status).toBe('DUPLICATE');
    expect(r.error).toBe('NONE');
  });

  it('dedups concurrent identical command_id into one invoke', async () => {
    const t = new FakeTransport();
    const client = makeClient(t);
    let resolveReceipt: (r: CommandReceipt) => void = () => {};
    t.respond(
      'send_command',
      () =>
        new Promise((res) => {
          resolveReceipt = res as (r: CommandReceipt) => void;
        }),
    );
    const p1 = client.sendCommand(
      { AddTrackOp: { track_id: 'a', kind: 'AUDIO' } },
      { commandId: 'dup-1' },
    );
    const p2 = client.sendCommand(
      { AddTrackOp: { track_id: 'a', kind: 'AUDIO' } },
      { commandId: 'dup-1' },
    );
    expect(t.calls).toHaveLength(1); // second call attached to the first
    resolveReceipt(appliedReceipt({ command_id: 'dup-1' }));
    const [r1, r2] = await Promise.all([p1, p2]);
    expect(r1).toBe(r2);
    // after resolution, resend is allowed again
    t.respond('send_command', () =>
      appliedReceipt({ status: 'DUPLICATE', command_id: 'dup-1' }),
    );
    const r3 = await client.sendCommand(
      { AddTrackOp: { track_id: 'a', kind: 'AUDIO' } },
      { commandId: 'dup-1' },
    );
    expect(r3.status).toBe('DUPLICATE');
    expect(t.calls).toHaveLength(2);
  });

  it('tracks revision from APPLIED receipts for expected_revision', async () => {
    const t = new FakeTransport();
    const client = makeClient(t);
    t.respond('send_command', () => appliedReceipt({ revision: '11' }));
    await client.sendCommand({ SaveProjectOp: {} });
    expect(client.projectRevision).toBe('11');
    t.respond('send_command', () => appliedReceipt({ revision: '12' }));
    await client.sendCommand({ SaveProjectOp: {} });
    const dto = t.calls[1].args!.dto as PersistentCommandDto;
    expect(dto.expected_revision).toBe('11'); // previous ack became expected
    expect(client.projectRevision).toBe('12');
  });

  it('adopts current revision from a STALE_REVISION rejection', async () => {
    const t = new FakeTransport();
    const client = makeClient(t);
    t.respond('send_command', () =>
      appliedReceipt({ status: 'REJECTED', error: 'STALE_REVISION', revision: '99' }),
    );
    const r = await client.sendCommand({ SaveProjectOp: {} });
    expect(r.status).toBe('REJECTED');
    expect(r.error).toBe('STALE_REVISION');
    expect(client.projectRevision).toBe('99');
  });
});

describe('transport requests', () => {
  it('sends transport DTO with op name + string ticks', async () => {
    const t = new FakeTransport();
    const client = makeClient(t);
    t.respond('send_transport', () => ({
      kind: 'TransportAck',
      request_id: 'r-1',
      ok: true,
      error: 'NONE',
    }));
    const ack = await client.seek(3840000n); // 1 bar @4/4, 960k ticks/quarter
    expect(ack).toMatchObject({ kind: 'TransportAck', ok: true });
    const dto = t.calls[0].args!.dto as {
      op: string;
      position_ticks: string;
      engine_epoch: string;
    };
    expect(dto.op).toBe('SEEK');
    expect(dto.position_ticks).toBe('3840000');
    expect(typeof dto.engine_epoch).toBe('string');
  });

  it('panic resolves with the send-ack shape', async () => {
    const t = new FakeTransport();
    const client = makeClient(t);
    t.respond('send_transport', () => ({ accepted: true }));
    const ack = await client.panic();
    expect(ack).toEqual({ accepted: true });
  });
});

describe('read_view', () => {
  it('sends read DTO and decodes a page', async () => {
    const t = new FakeTransport();
    const client = makeClient(t);
    t.respond('read_view', () => ({
      kind: 'ReadResponse',
      request_id: 'x',
      revision: '4',
      items: [{ object_id: 'trk-1', summary_json: '{"name":"Keys"}' }],
      next_cursor: '',
      done: true,
      error: 'NONE',
    }));
    const res = await client.readView({ view: 'TRACK_LIST' });
    expect(res.kind).toBe('ReadResponse');
    expect(res.items).toHaveLength(1);
    expect(res.items[0].object_id).toBe('trk-1');
    expect(res.done).toBe(true);
    const dto = t.calls[0].args!.dto as ReadRequestDto;
    expect(dto.view).toBe('TRACK_LIST');
    expect(dto.start_ticks).toBe('-1'); // unbounded window default
  });

  it('readViewPages follows next_cursor until done and stays bounded', async () => {
    const t = new FakeTransport();
    const client = makeClient(t);
    let page = 0;
    t.respond('read_view', (args) => {
      const c = (args!.dto as { cursor: string }).cursor;
      page += 1;
      const done = page === 3;
      return {
        kind: 'ReadResponse',
        request_id: 'x',
        revision: '4',
        items: [{ object_id: `o${c || '0'}`, summary_json: '{}' }],
        next_cursor: done ? '' : `c${page}`,
        done,
        error: 'NONE',
      } satisfies ReadResponse;
    });
    const pages: ReadResponse[] = [];
    for await (const p of client.readViewPages({ view: 'CLIP_LIST' })) {
      pages.push(p);
    }
    expect(pages).toHaveLength(3);
    expect(pages[2].done).toBe(true);
    expect(t.calls.map((c) => (c.args!.dto as { cursor: string }).cursor)).toEqual([
      '',
      'c1',
      'c2',
    ]);
  });

  it('readViewPages aborts past maxPages instead of looping forever', async () => {
    const t = new FakeTransport();
    const client = makeClient(t);
    t.respond('read_view', () => ({
      kind: 'ReadResponse',
      request_id: 'x',
      revision: '0',
      items: [],
      next_cursor: 'next',
      done: false,
      error: 'NONE',
    }));
    const iter = client.readViewPages({ view: 'TRACK_LIST', maxPages: 2 });
    await expect(async () => {
      for await (const _p of iter) void _p;
    }).rejects.toThrow(/maxPages/);
  });
});

describe('events', () => {
  it('dispatches telemetry and control events after start()', async () => {
    const t = new FakeTransport();
    const client = makeClient(t);
    const onT = vi.fn();
    const onC = vi.fn();
    client.onTelemetry(onT);
    client.onControl(onC);
    await client.start();

    t.emit('void://telemetry', {
      kind: 'ClockSnapshot',
      project_id: 'proj-1',
      engine_epoch: '3',
      timeline_sample: '48000',
      device_sample_counter: '96000',
      sample_rate: 48000,
      transport: 'PLAYING',
      loop_start_ticks: '0',
      loop_end_ticks: '0',
      tempo_map_revision: '1',
      sequence: '7',
      host_clock_ns: '12345',
    });
    expect(onT).toHaveBeenCalledWith(
      expect.objectContaining({ kind: 'ClockSnapshot', transport: 'PLAYING' }),
    );

    t.emit('void://control', appliedReceipt({ command_id: 'z' }));
    expect(onC).toHaveBeenCalledWith(
      expect.objectContaining({ kind: 'CommandReceipt', command_id: 'z' }),
    );
    // control-bus receipts also reconcile revision
    expect(client.projectRevision).toBe('7');
  });

  it('engine-lost marks detached and notifies listeners', async () => {
    const t = new FakeTransport();
    const client = makeClient(t);
    const onL = vi.fn();
    client.onEngineLost(onL);
    await client.start();
    t.respond('spawn_engine', () => ({
      attached: true,
      worker_id: 'w-1',
      engine_epoch: '5',
    }));
    await client.spawnEngine('void-mock-worker');
    expect(client.isAttached).toBe(true);
    expect(client.engineEpoch).toBe('5');

    t.emit('void://engine-lost', { worker_id: 'w-1' });
    expect(onL).toHaveBeenCalledWith({ worker_id: 'w-1' });
    expect(client.isAttached).toBe(false);
  });

  it('stop() unsubscribes all channels', async () => {
    const t = new FakeTransport();
    const client = makeClient(t);
    const onT = vi.fn();
    client.onTelemetry(onT);
    await client.start();
    await client.dispose();
    t.emit('void://telemetry', { kind: 'TransportAck' });
    expect(onT).not.toHaveBeenCalled();
  });
});

describe('undo/redo', () => {
  it('issues UndoOp/RedoOp as persistent commands — no client-side undo', async () => {
    const t = new FakeTransport();
    const client = makeClient(t);
    t.respond('send_command', () => appliedReceipt());
    await client.undo();
    const dto = t.calls[0].args!.dto as PersistentCommandDto;
    expect(dto.op).toEqual({ UndoOp: { transaction_id: '' } });
    await client.redo('tx-9');
    const dto2 = t.calls[1].args!.dto as PersistentCommandDto;
    expect(dto2.op).toEqual({ RedoOp: { transaction_id: 'tx-9' } });
  });
});
