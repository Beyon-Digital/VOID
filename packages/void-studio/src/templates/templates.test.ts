import { describe, expect, it } from 'vitest';
import { FakeTransport, VoidClient } from 'void-client';
import type { CommandReceipt, PersistentCommandDto } from 'void-client';
import { createTranslator, en } from '../i18n';
import { applyProjectTemplate } from './apply';
import { BUILTIN_TEMPLATES, TEMPLATE_EMPTY, TEMPLATE_LIVE_BAND, templateById } from './templates';

const t = createTranslator(en);

function appliedReceipt(dto: PersistentCommandDto, revision: number): CommandReceipt {
  return {
    kind: 'CommandReceipt',
    command_id: dto.command_id,
    status: 'APPLIED',
    error: 'NONE',
    revision: String(revision),
    engine_epoch: '1',
  };
}

function clientWithApplySeq(): { client: VoidClient; transport: FakeTransport; ops: string[] } {
  const transport = new FakeTransport();
  const ops: string[] = [];
  let rev = 0;
  transport.respond('send_command', (args) => {
    const dto = args!.dto as PersistentCommandDto;
    const name = Object.keys(dto.op)[0];
    ops.push(name);
    rev += 1;
    return appliedReceipt(dto, rev);
  });
  return { client: new VoidClient({ transport }), transport, ops };
}

describe('builtin templates', () => {
  it('registry is deterministic and self-consistent', () => {
    expect(BUILTIN_TEMPLATES.map((t) => t.id)).toEqual([
      'builtin.empty',
      'builtin.vocal_drums',
      'builtin.live_band',
    ]);
    for (const tpl of BUILTIN_TEMPLATES) {
      expect(templateById(tpl.id)).toBe(tpl);
      expect(tpl.sampleRate).toBeGreaterThan(0);
      expect(tpl.initialBpm).toBeGreaterThan(0);
      // every name/description/track key must resolve in the shipped catalog
      expect(t(tpl.nameKey)).not.toContain('⟦');
      if (tpl.descriptionKey) expect(t(tpl.descriptionKey)).not.toContain('⟦');
      for (const spec of tpl.tracks) {
        expect(['AUDIO', 'MIDI', 'INSTRUMENT', 'BUS']).toContain(spec.kind);
        expect(en[spec.nameKey]).toBeTruthy();
        if (spec.instrumentUid) expect(spec.instrumentUid.startsWith('void.builtin.')).toBe(true);
      }
    }
    expect(templateById('nope')).toBeUndefined();
  });
});

describe('applyProjectTemplate', () => {
  it('empty template issues exactly CreateProjectOp', async () => {
    const { client, ops } = clientWithApplySeq();
    let i = 0;
    const res = await applyProjectTemplate(
      client,
      TEMPLATE_EMPTY,
      { projectId: 'p1', name: 'song', containerDir: '/tmp/song.void' },
      { t, ids: () => `id-${++i}` },
    );
    expect(res.ok).toBe(true);
    expect(ops).toEqual(['CreateProjectOp']);
    expect(res.steps[0].receipt.revision).toBe('1');
    expect(client.currentProjectId).toBe('p1');
  });

  it('live band issues create + tracks + instrument plugins in order, one transaction', async () => {
    const { client, ops } = clientWithApplySeq();
    let i = 0;
    const res = await applyProjectTemplate(
      client,
      TEMPLATE_LIVE_BAND,
      { projectId: 'p1', name: 'gig', containerDir: '/tmp/gig.void' },
      { t, ids: () => `id-${++i}`, transactionId: 'tx-1' },
    );
    expect(res.ok).toBe(true);
    // 1 create + 6 AddTrack + 2 InsertPlugin (drums sampler, keys four_osc)
    expect(ops).toEqual([
      'CreateProjectOp',
      'AddTrackOp',
      'AddTrackOp',
      'AddTrackOp',
      'AddTrackOp',
      'InsertPluginOp',
      'AddTrackOp',
      'InsertPluginOp',
      'AddTrackOp',
    ]);
    expect(res.steps.length).toBe(9);
    // instrument ops reference their parent track + builtin format
    const inserts = res.steps.filter((s) => s.opName === 'InsertPluginOp');
    expect(inserts).toHaveLength(2);
    for (const s of inserts) {
      const op = (s.receipt as { kind: string }).kind;
      expect(op).toBe('CommandReceipt');
    }
  });

  it('stops at the first rejection and reports it honestly', async () => {
    const transport = new FakeTransport();
    const ops: string[] = [];
    let rev = 0;
    transport.respond('send_command', (args) => {
      const dto = args!.dto as PersistentCommandDto;
      const name = Object.keys(dto.op)[0];
      ops.push(name);
      rev += 1;
      if (name === 'InsertPluginOp') {
        return {
          kind: 'CommandReceipt',
          command_id: dto.command_id,
          status: 'REJECTED',
          error: 'PLUGIN_UNAVAILABLE',
          revision: String(rev - 1),
          engine_epoch: '1',
          message: 'sampler unavailable',
        } as CommandReceipt;
      }
      return appliedReceipt(dto, rev);
    });
    const client = new VoidClient({ transport });
    let i = 0;
    const res = await applyProjectTemplate(
      client,
      TEMPLATE_LIVE_BAND,
      { projectId: 'p1', name: 'gig', containerDir: '/tmp/gig.void' },
      { t, ids: () => `id-${++i}` },
    );
    expect(res.ok).toBe(false);
    expect(res.failedStep?.opName).toBe('InsertPluginOp');
    expect(res.failedStep?.receipt.error).toBe('PLUGIN_UNAVAILABLE');
    // nothing after the failed step was sent (create + 4 tracks + the
    // rejected insert = 6 ops)
    expect(ops[ops.length - 1]).toBe('InsertPluginOp');
    expect(ops.length).toBe(6);
  });

  it('uses the localized track names from the translator', async () => {
    const transport = new FakeTransport();
    const names: string[] = [];
    transport.respond('send_command', (args) => {
      const dto = args!.dto as PersistentCommandDto;
      const op = dto.op as { AddTrackOp?: { name?: string } };
      if (op.AddTrackOp?.name) names.push(op.AddTrackOp.name);
      return appliedReceipt(dto, names.length);
    });
    const client = new VoidClient({ transport });
    await applyProjectTemplate(
      client,
      TEMPLATE_LIVE_BAND,
      { projectId: 'p1', name: 'gig', containerDir: '/tmp/gig.void' },
      { t, ids: (() => { let i = 0; return () => `id-${++i}`; })() },
    );
    expect(names).toEqual(['Vocals', 'Guitar', 'Bass', 'Drums', 'Keys', 'Mix Bus']);
  });
});
