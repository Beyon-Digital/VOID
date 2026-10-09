import { describe, expect, it } from 'vitest';
import { isRecoverable, parsePluginSlotItem, parsePluginSlots } from './pluginList';
import type { ReadItem } from 'void-client';

const item = (json: string, ref = ''): ReadItem => ({
  object_id: ref,
  summary_json: json,
});

describe('parsePluginSlotItem', () => {
  it('parses an ACTIVE slot', () => {
    const p = parsePluginSlotItem(item('{"instanceId":"i-1","name":"EQ","status":"ACTIVE"}'));
    expect(p).toEqual({ instanceId: 'i-1', name: 'EQ', status: 'ACTIVE', reason: undefined });
  });

  it('parses a MISSING slot with reason', () => {
    const p = parsePluginSlotItem(
      item('{"instanceId":"i-2","status":"MISSING","reason":"binary not found"}'),
    );
    expect(p?.status).toBe('MISSING');
    expect(p?.reason).toBe('binary not found');
    expect(isRecoverable(p!)).toBe(true);
  });

  it('treats BYPASSED as recoverable', () => {
    const p = parsePluginSlotItem(item('{"instanceId":"i-3","status":"BYPASSED"}'));
    expect(isRecoverable(p!)).toBe(true);
  });

  it('rejects malformed rows', () => {
    expect(parsePluginSlotItem(item('not json'))).toBeNull();
    expect(parsePluginSlotItem(item('{"status":"MISSING"}'))).toBeNull();
    expect(parsePluginSlotItem(item('{"instanceId":"i"}'))).toBeNull();
    expect(parsePluginSlotItem(item('42'))).toBeNull();
  });

  it('parsePluginSlots drops malformed rows and keeps order', () => {
    const slots = parsePluginSlots([
      item('{"instanceId":"a","status":"ACTIVE"}'),
      item('bad'),
      item('{"instanceId":"b","status":"MISSING","reason":"x"}'),
    ]);
    expect(slots.map((s) => s.instanceId)).toEqual(['a', 'b']);
  });
});
