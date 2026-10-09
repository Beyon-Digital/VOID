// T41 — mixer ops on the wire, honest meters, routing model limits.

import { describe, expect, it } from 'vitest';
import { FakeTransport, VoidClient } from 'void-client';
import type { PersistentCommandDto, CommandReceipt, MeterFrame } from 'void-client';
import {
  setTrackGainOp, setTrackPanOp, setTrackMuteOp, setTrackSoloOp,
  gainToDb, dbToGain, MIN_DB,
} from './ops';
import { addBusTrackOp, routeSendOp, routeModelFromTrackItems, UnsupportedCapability } from './routing';
import { meterStrip, levelToDb, MeterClock, METER_FRESH_MS } from './meters';
import { createMixerViewStore, stripsFromTrackItems } from './model';
import { setGainDb, setMuted, setPan, setSoloed } from './controller';

const applied = (rev: string): CommandReceipt => ({
  kind: 'CommandReceipt', command_id: 'c', status: 'APPLIED', error: 'NONE', revision: rev,
});

function rig(receipt: CommandReceipt = applied('3')) {
  const t = new FakeTransport();
  const sent: PersistentCommandDto[] = [];
  t.respond('send_command', (args) => {
    sent.push((args as { dto: PersistentCommandDto }).dto);
    return receipt;
  });
  const client = new VoidClient({ transport: t, projectId: 'p1' });
  const store = createMixerViewStore();
  return { sent, client, store };
}

const frame = (over: Partial<MeterFrame> = {}): MeterFrame => ({
  kind: 'MeterFrame', project_id: 'p1', engine_epoch: '1', track_id: 't1',
  peak_l: 0.5, peak_r: 0.25, rms_l: 0.3, rms_r: 0.2, clipped: false,
  sequence: '9', ...over,
});

describe('mixer ops', () => {
  it('gain/pan/mute/solo produce exact wire shapes', () => {
    expect(setTrackGainOp('t1', 0.75)).toEqual({ SetTrackGainOp: { track_id: 't1', gain_linear: 0.75 } });
    expect(setTrackPanOp('t1', -0.5)).toEqual({ SetTrackPanOp: { track_id: 't1', pan: -0.5 } });
    expect(setTrackMuteOp('t1', true)).toEqual({ SetTrackMuteOp: { track_id: 't1', muted: true } });
    expect(setTrackSoloOp('t1', false)).toEqual({ SetTrackSoloOp: { track_id: 't1', soloed: false } });
  });

  it('rejects out-of-range/NaN before the wire', () => {
    expect(() => setTrackGainOp('t', -0.1)).toThrow();
    expect(() => setTrackGainOp('t', NaN)).toThrow(/finite/);
    expect(() => setTrackPanOp('t', 1.5)).toThrow();
    expect(() => setTrackPanOp('t', Infinity)).toThrow();
  });

  it('dB helpers round-trip and floor at MIN_DB', () => {
    expect(dbToGain(0)).toBeCloseTo(1);
    expect(gainToDb(1)).toBeCloseTo(0);
    expect(dbToGain(-6)).toBeCloseTo(0.501, 2);
    expect(dbToGain(-120)).toBe(0);
    expect(gainToDb(0)).toBe(MIN_DB);
  });
});

describe('meter honesty (T41)', () => {
  it('idle when no frame has ever arrived — not a fake zero-envelope', () => {
    const s = meterStrip(undefined, 0);
    expect(s.status).toBe('idle');
    expect(s.peakDbL).toBe(MIN_DB);
    expect(s.clipped).toBe(false);
  });

  it('live within the freshness window, stale after', () => {
    const now = 10_000;
    const live = meterStrip(frame(), now, now - 50);
    expect(live.status).toBe('live');
    expect(live.peakDbL).toBeCloseTo(levelToDb(0.5), 5);
    const stale = meterStrip(frame(), now, now - METER_FRESH_MS - 1);
    expect(stale.status).toBe('idle');
    expect(stale.sequence).toBe('9'); // last frame id still visible
  });

  it('clipped flag and true levels pass through verbatim', () => {
    const s = meterStrip(frame({ clipped: true, peak_l: 1.0, peak_r: 0.0 }), 100, 90);
    expect(s.status).toBe('live');
    expect(s.clipped).toBe(true);
    expect(s.peakDbL).toBeCloseTo(0);
    expect(s.peakDbR).toBe(MIN_DB); // genuine silence reads floor, not fake noise
  });

  it('MeterClock tracks per-track arrival times', () => {
    let t = 0;
    const clock = new MeterClock(() => t);
    clock.note(frame());
    expect(clock.strip(frame()).status).toBe('live');
    t += METER_FRESH_MS + 1;
    expect(clock.strip(frame()).status).toBe('idle');
  });
});

describe('routing model', () => {
  it('creates BUS tracks through AddTrackOp', () => {
    expect(addBusTrackOp('bus1', 'Drums Bus')).toEqual({
      AddTrackOp: { track_id: 'bus1', kind: 'BUS', name: 'Drums Bus', index: undefined },
    });
  });

  it('sends are a typed refusal — no fake routing op on the wire', () => {
    expect(() => routeSendOp('t1', 'bus1')).toThrow(UnsupportedCapability);
    expect(() => routeSendOp('t1', 'bus1', -2)).toThrow(/finite|>= 0/);
    const model = routeModelFromTrackItems([
      { summary_json: JSON.stringify({ kind: 'BUS', track_id: 'b1' }) },
      { summary_json: JSON.stringify({ kind: 'AUDIO', track_id: 'a1' }) },
      { summary_json: 'not json' },
    ]);
    expect(model.buses).toEqual(['b1']);
    expect(model.sendsExpressible).toBe(false);
  });
});

describe('mixer view + controller', () => {
  it('strips project only well-formed summaries', () => {
    const strips = stripsFromTrackItems([
      { object_id: 'o1', summary_json: JSON.stringify({ kind: 'AUDIO', name: 'Vox', gain_linear: 0.9, muted: false }) },
      { object_id: 'o2', summary_json: '{bad' },
      { object_id: 'o3', summary_json: JSON.stringify({ kind: 'ALIEN' }) },
      { object_id: 'o4', summary_json: JSON.stringify({ kind: 'BUS', name: 'Bus' }) },
    ]);
    expect(strips.map((s) => s.trackId)).toEqual(['o1', 'o4']);
    expect(strips[0].name).toBe('Vox');
    expect(strips[0].gainLinear).toBe(0.9);
  });

  it('controller marks pending then accepted; wire shows the op', async () => {
    const { sent, client, store } = rig();
    store.getState().actions.setStrips([{ trackId: 't1', kind: 'AUDIO', name: 'T' }]);
    const outcome = await setGainDb({ client, store, refreshTracks: async () => {} }, 't1', -6);
    expect(outcome.receipt.status).toBe('APPLIED');
    expect(sent[0].op).toHaveProperty('SetTrackGainOp');
    const gain = (sent[0].op as { SetTrackGainOp: { gain_linear: number } }).SetTrackGainOp.gain_linear;
    expect(gain).toBeCloseTo(0.501, 2);
    expect(store.getState().strips[0].gainLinear).toBeCloseTo(0.501, 2);
    expect(store.getState().strips[0].pending).toBeUndefined();
  });

  it('rejection clears pending without touching last values', async () => {
    const { client, store } = rig({
      kind: 'CommandReceipt', command_id: 'c', status: 'REJECTED', error: 'BAD_REQUEST', revision: '4',
    });
    store.getState().actions.setStrips([{ trackId: 't1', kind: 'AUDIO', name: 'T', muted: false }]);
    await setMuted({ client, store, refreshTracks: async () => {} }, 't1', true);
    expect(store.getState().strips[0].muted).toBe(false);
    expect(store.getState().strips[0].pending).toBeUndefined();
  });

  it('pan and solo round-trip through the controller', async () => {
    const { sent, client, store } = rig();
    store.getState().actions.setStrips([{ trackId: 't1', kind: 'AUDIO', name: 'T' }]);
    await setPan({ client, store, refreshTracks: async () => {} }, 't1', 0.25);
    await setSoloed({ client, store, refreshTracks: async () => {} }, 't1', true);
    expect(sent[0].op).toEqual({ SetTrackPanOp: { track_id: 't1', pan: 0.25 } });
    expect(sent[1].op).toEqual({ SetTrackSoloOp: { track_id: 't1', soloed: true } });
    expect(store.getState().strips[0].pan).toBe(0.25);
    expect(store.getState().strips[0].soloed).toBe(true);
  });
});
