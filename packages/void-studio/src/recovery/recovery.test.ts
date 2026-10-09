import { describe, expect, it } from 'vitest';
import { createRecoveryStore } from './index';
import type { SaveResultEvent } from 'void-client';

const durable = (over: Partial<SaveResultEvent> = {}): SaveResultEvent => ({
  kind: 'SaveResultEvent',
  project_id: 'p-1',
  command_id: 'c-1',
  status: 'SAVE_DURABLE',
  revision: '7',
  checkpoint_id: 'ck-7',
  manifest_sha256: 'ab'.repeat(32),
  error: 'NONE',
  message: '',
  ...over,
});

const failed = (over: Partial<SaveResultEvent> = {}): SaveResultEvent => ({
  kind: 'SaveResultEvent',
  project_id: 'p-1',
  command_id: 'c-2',
  status: 'SAVE_FAILED',
  revision: '9',
  checkpoint_id: '',
  manifest_sha256: '',
  error: 'DISK_FULL',
  message: 'target write failed',
  ...over,
});

describe('recovery store', () => {
  it('keeps the last durable save even after a later failure', () => {
    const s = createRecoveryStore();
    s.getState().actions.noteSaveResult(durable());
    s.getState().actions.noteSaveResult(failed());
    expect(s.getState().lastDurableSave?.checkpoint_id).toBe('ck-7');
    expect(s.getState().lastFailedSave?.error).toBe('DISK_FULL');
  });

  it('a subsequent durable save replaces the last-good record', () => {
    const s = createRecoveryStore();
    s.getState().actions.noteSaveResult(durable());
    s.getState().actions.noteSaveResult(failed());
    s.getState().actions.noteSaveResult(durable({ revision: '11', checkpoint_id: 'ck-11' }));
    expect(s.getState().lastDurableSave?.checkpoint_id).toBe('ck-11');
    expect(s.getState().lastFailedSave).not.toBeNull();
  });

  it('records engine-lost and clears it on re-attach', () => {
    const s = createRecoveryStore();
    s.getState().actions.noteEngineLost('w-1');
    expect(s.getState().lastEngineLost?.workerId).toBe('w-1');
    s.getState().actions.noteEngineAttached();
    expect(s.getState().lastEngineLost).toBeNull();
  });

  it('ignores blank spawn paths and keeps the last good one', () => {
    const s = createRecoveryStore();
    s.getState().actions.noteSpawnExecutable('./engine');
    s.getState().actions.noteSpawnExecutable('   ');
    expect(s.getState().lastSpawnExecutable).toBe('./engine');
  });
});
