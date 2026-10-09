import { describe, expect, it } from 'vitest';
import { deviceChainFromItems, deviceFromSummary, devicesFromItems } from './devices';

const item = (id: string, json: string) => ({ object_id: id, summary_json: json });

describe('deviceFromSummary', () => {
  it('parses a healthy engine item', () => {
    const d = deviceFromSummary(
      'obj-1',
      JSON.stringify({ plugin_instance_id: 'pi-1', track_id: 't1', name: 'EQ8', format: 'vst3', slot: 0 }),
    );
    expect(d).toMatchObject({ instanceId: 'pi-1', trackId: 't1', name: 'EQ8', format: 'vst3', slot: 0 });
  });

  it('falls back to object_id when no instance id is reported', () => {
    const d = deviceFromSummary('obj-9', JSON.stringify({ name: 'Comp' }));
    expect(d?.instanceId).toBe('obj-9');
  });

  it('surfaces failed state verbatim (missing/quarantined/error)', () => {
    expect(
      deviceFromSummary('o', JSON.stringify({ name: 'X', quarantined: true }))?.failed,
    ).toBe('quarantined');
    expect(
      deviceFromSummary('o', JSON.stringify({ name: 'Y', error: 'dsp crash' }))?.failed,
    ).toBe('dsp crash');
  });

  it('never assumes powered state the engine did not report', () => {
    const d = deviceFromSummary('o', JSON.stringify({ name: 'Z' }));
    expect(d?.powered).toBeUndefined();
  });

  it('drops malformed items instead of fabricating a device', () => {
    expect(deviceFromSummary('o', '{not json')).toBeNull();
  });
});

describe('deviceChainFromItems', () => {
  const items = [
    item('i1', JSON.stringify({ plugin_instance_id: 'p1', track_id: 't1', name: 'B', slot: 1 })),
    item('i2', JSON.stringify({ plugin_instance_id: 'p2', track_id: 't1', name: 'A', slot: 0 })),
    item('i3', JSON.stringify({ plugin_instance_id: 'p3', track_id: 't2', name: 'other' })),
  ];

  it('returns the target track chain ordered by slot', () => {
    const chain = deviceChainFromItems(items, 't1');
    expect(chain.map((d) => d.name)).toEqual(['A', 'B']);
  });

  it('excludes devices bound to another track', () => {
    expect(deviceChainFromItems(items, 't1').find((d) => d.name === 'other')).toBeUndefined();
  });

  it('devicesFromItems returns everything parseable', () => {
    expect(devicesFromItems([...items, item('bad', 'nope')])).toHaveLength(3);
  });
});
