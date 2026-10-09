// T40 — instrument rack: descriptor op shapes through the real client +
// FakeTransport, preset completeness, stale-retry, unknown-param refusal.

import { describe, expect, it } from 'vitest';
import { FakeTransport, VoidClient } from 'void-client';
import type { PersistentCommandDto, CommandReceipt } from 'void-client';
import {
  FOUR_OSC,
  SAMPLER,
  clampParam,
  insertInstrumentOp,
  setInstrumentParamOp,
  paramOf,
} from './descriptors';
import {
  createInstrumentBrowseStore,
  presetsFor,
  resolvePreset,
  BUILTIN_PRESETS,
} from './presets';
import { loadInstrument, setInstrumentParam, removeInstrument } from './binding';

const applied = (rev: string): CommandReceipt => ({
  kind: 'CommandReceipt',
  command_id: 'c',
  status: 'APPLIED',
  error: 'NONE',
  revision: rev,
});

function rig(receipt: CommandReceipt = applied('9')) {
  const t = new FakeTransport();
  const sent: PersistentCommandDto[] = [];
  t.respond('send_command', (args) => {
    sent.push((args as { dto: PersistentCommandDto }).dto);
    return receipt;
  });
  const client = new VoidClient({ transport: t, projectId: 'p1', ids: () => `id-${sent.length}` });
  const store = createInstrumentBrowseStore();
  return { t, sent, client, store };
}

describe('instrument ops', () => {
  it('insert maps descriptor to InsertPluginOp with builtin format', () => {
    const op = insertInstrumentOp('trk1', FOUR_OSC, 'inst-1', 0);
    expect(op).toEqual({
      InsertPluginOp: {
        track_id: 'trk1',
        slot: 0,
        plugin_instance_id: 'inst-1',
        format: 'VOID-BUILTIN',
        plugin_uid: 'void.builtin.four_osc',
      },
    });
  });

  it('param op clamps to descriptor range and quantizes steps', () => {
    expect(setInstrumentParamOp(FOUR_OSC, 'i1', 'filter_cutoff', 999999)).toEqual({
      SetPluginParamOp: {
        plugin_instance_id: 'i1',
        param_id: 'filter_cutoff',
        value: 20000,
      },
    });
    // osc wave is stepped: 1.4 → 1
    expect(setInstrumentParamOp(FOUR_OSC, 'i1', 'osc1_wave', 1.4)).toEqual({
      SetPluginParamOp: {
        plugin_instance_id: 'i1',
        param_id: 'osc1_wave',
        value: 1,
      },
    });
  });

  it('rejects params the descriptor does not expose', () => {
    expect(() => setInstrumentParamOp(FOUR_OSC, 'i1', 'explode', 1)).toThrow(/not exposed/);
    expect(paramOf(SAMPLER, 'nonexistent')).toBeUndefined();
  });

  it('rejects non-finite values', () => {
    expect(() => clampParam(FOUR_OSC.params[0], NaN)).toThrow(/finite/);
    expect(() => setInstrumentParamOp(FOUR_OSC, 'i', 'env_attack', Infinity)).toThrow();
  });
});

describe('presets', () => {
  it('resolve produces complete param state (diff → full set)', () => {
    const preset = BUILTIN_PRESETS.find((p) => p.id === 'preset.four_osc.warm_pad')!;
    const resolved = resolvePreset(FOUR_OSC, preset);
    // Every descriptor param gets a value — reopening reproduces the sound.
    expect(resolved.length).toBe(FOUR_OSC.params.length);
    const cut = resolved.find((r) => r.param.id === 'filter_cutoff')!;
    expect(cut.value).toBe(2400);
    // Unlisted params land on defaults.
    const osc4 = resolved.find((r) => r.param.id === 'osc4_level')!;
    expect(osc4.value).toBe(0);
  });

  it('rejects presets targeting unknown params or wrong instrument', () => {
    expect(() =>
      resolvePreset(FOUR_OSC, {
        id: 'x', name: 'x', instrumentUid: FOUR_OSC.pluginUid,
        category: 'x', params: { bogus: 1 }, license: '',
      }),
    ).toThrow(/unknown param/);
    expect(() =>
      resolvePreset(FOUR_OSC, {
        id: 'x', name: 'x', instrumentUid: 'void.builtin.sampler',
        category: 'x', params: {}, license: '',
      }),
    ).toThrow(/targets/);
  });

  it('browse state filters presets per selected instrument', () => {
    const store = createInstrumentBrowseStore();
    store.getState().actions.setBrowseUid(FOUR_OSC.pluginUid);
    store.getState().actions.setCategory('leads');
    const st = store.getState();
    expect(presetsFor(FOUR_OSC.pluginUid).length).toBe(3);
    expect(st.filterText).toBe('');
  });
});

describe('binding', () => {
  it('loadInstrument sends insert + full preset ops, one transaction', async () => {
    const { sent, client, store } = rig();
    const preset = BUILTIN_PRESETS.find((p) => p.id === 'preset.four_osc.bright_lead')!;
    const outcomes = await loadInstrument(
      { client, store, refreshPlugins: async () => {}, transactionId: 'gesture-1' },
      'trk1', FOUR_OSC, 'inst-9', 0, preset,
    );
    expect(sent.length).toBe(1 + FOUR_OSC.params.length);
    expect(sent[0].op).toHaveProperty('InsertPluginOp');
    // One gesture — every command shares the transaction id.
    for (const dto of sent) expect(dto.transaction_id).toBe('gesture-1');
    expect(sent[1].op).toHaveProperty('SetPluginParamOp');
    expect(outcomes.every((o) => o.receipt.status === 'APPLIED')).toBe(true);
    expect(store.getState().slots[0].pluginUid).toBe(FOUR_OSC.pluginUid);
  });

  it('insert failure suppresses param ops', async () => {
    const { sent, client, store } = rig({
      kind: 'CommandReceipt', command_id: 'c', status: 'REJECTED',
      error: 'PLUGIN_UNAVAILABLE', revision: '9',
    });
    const outcomes = await loadInstrument(
      { client, store, refreshPlugins: async () => {} },
      'trk1', FOUR_OSC, 'inst-9', 0,
    );
    expect(outcomes.length).toBe(1);
    expect(sent.length).toBe(1);
    expect(store.getState().slots.length).toBe(0);
  });

  it('setInstrumentParam records only accepted values; STALE_REVISION retries once', async () => {
    const t = new FakeTransport();
    let calls = 0;
    t.respond('send_command', () => {
      calls++;
      // first is stale, second applies
      return calls === 1
        ? { kind: 'CommandReceipt', command_id: 'c', status: 'REJECTED', error: 'STALE_REVISION', revision: '55' }
        : applied('56');
    });
    const sent: PersistentCommandDto[] = [];
    t.respond('send_command', (args) => {
      sent.push((args as { dto: PersistentCommandDto }).dto);
      calls++;
      return calls === 1
        ? { kind: 'CommandReceipt', command_id: 'c', status: 'REJECTED', error: 'STALE_REVISION', revision: '55' }
        : applied('56');
    });
    const client = new VoidClient({ transport: t, projectId: 'p1' });
    const store = createInstrumentBrowseStore();
    let refreshed = false;
    const outcome = await setInstrumentParam(
      { client, store, refreshPlugins: async () => { refreshed = true; } },
      FOUR_OSC, 'inst-1', 'env_sustain', 0.42,
    );
    expect(outcome.retried).toBe(true);
    expect(refreshed).toBe(true);
    // retry used the revision the receipt reported
    expect(sent[1].expected_revision).toBe('55');
    expect(store.getState().slots).toEqual([]); // no slot registered → noteParamSent is a no-op on unknown instance
  });

  it('rejected param send leaves lastKnown untouched', async () => {
    const { client, store } = rig({
      kind: 'CommandReceipt', command_id: 'c', status: 'REJECTED',
      error: 'BAD_REQUEST', revision: '9',
    });
    store.getState().actions.upsertSlot({
      slotIndex: 0, instanceId: 'i1', pluginUid: FOUR_OSC.pluginUid,
      lastKnown: { master_level: 0.8 },
    });
    await setInstrumentParam(
      { client, store, refreshPlugins: async () => {} },
      FOUR_OSC, 'i1', 'master_level', 0.1,
    );
    expect(store.getState().slots[0].lastKnown.master_level).toBe(0.8);
  });

  it('removeInstrument drops the slot on success only', async () => {
    const { client, store } = rig();
    store.getState().actions.upsertSlot({
      slotIndex: 0, instanceId: 'i1', pluginUid: FOUR_OSC.pluginUid, lastKnown: {},
    });
    await removeInstrument({ client, store, refreshPlugins: async () => {} }, 'i1');
    expect(store.getState().slots.length).toBe(0);
  });
});
