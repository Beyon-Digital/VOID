import { describe, expect, it } from 'vitest';
import {
  createRecordingViewStore,
  parseRecordingSummary,
  recordingFromSummaryItems,
  recordingUiState,
} from './recording';

describe('parseRecordingSummary', () => {
  it('parses the engine payload verbatim', () => {
    const s = parseRecordingSummary({
      phase: 'recording',
      isRecording: true,
      armedTracks: ['trk-1', 'trk-2'],
      takeId: 'take-9',
      lastError: 'disk full',
    });
    expect(s).toEqual({
      phase: 'recording',
      isRecording: true,
      armedTracks: ['trk-1', 'trk-2'],
      takeId: 'take-9',
      lastError: 'disk full',
    });
  });

  it('returns undefined for absent or malformed payloads', () => {
    expect(parseRecordingSummary(undefined)).toBeUndefined();
    expect(parseRecordingSummary(null)).toBeUndefined();
    expect(parseRecordingSummary('recording')).toBeUndefined();
    expect(parseRecordingSummary(42)).toBeUndefined();
  });

  it('defaults unknown phase to idle and drops non-string armed tracks', () => {
    const s = parseRecordingSummary({ phase: 'bogus', armedTracks: ['a', 7, null] });
    expect(s?.phase).toBe('idle');
    expect(s?.armedTracks).toEqual(['a']);
  });
});

describe('recordingFromSummaryItems', () => {
  it('reads item[0].recording', () => {
    const s = recordingFromSummaryItems([{ name: 'p', recording: { phase: 'armed', armedTracks: ['t'] } }]);
    expect(s?.phase).toBe('armed');
    expect(s?.armedTracks).toEqual(['t']);
  });

  it('returns undefined when the summary carries no recording block', () => {
    expect(recordingFromSummaryItems([{ name: 'p' }])).toBeUndefined();
    expect(recordingFromSummaryItems([])).toBeUndefined();
    expect(recordingFromSummaryItems('nope')).toBeUndefined();
  });
});

describe('recordingUiState', () => {
  it('is unavailable without an attached engine', () => {
    expect(recordingUiState({ engineAttached: false })).toBe('unavailable');
    expect(
      recordingUiState({ engineAttached: false, summary: { phase: 'recording', isRecording: true, armedTracks: [] } }),
    ).toBe('unavailable');
  });

  it('maps engine phases through', () => {
    const base = { engineAttached: true };
    expect(recordingUiState({ ...base, summary: { phase: 'recording', isRecording: true, armedTracks: [] } })).toBe(
      'recording',
    );
    expect(recordingUiState({ ...base, summary: { phase: 'stopping', isRecording: false, armedTracks: [] } })).toBe(
      'stopping',
    );
    expect(recordingUiState({ ...base, summary: { phase: 'failed', isRecording: false, armedTracks: [] } })).toBe(
      'failed',
    );
    expect(recordingUiState({ ...base, summary: { phase: 'armed', isRecording: false, armedTracks: ['t'] } })).toBe(
      'armed',
    );
    expect(recordingUiState({ ...base, summary: { phase: 'idle', isRecording: false, armedTracks: [] } })).toBe(
      'ready',
    );
  });

  it('enters review only when idle with an open review (UI-T10)', () => {
    expect(
      recordingUiState({
        engineAttached: true,
        summary: { phase: 'idle', isRecording: false, armedTracks: [] },
        reviewing: true,
      }),
    ).toBe('review');
    expect(
      recordingUiState({
        engineAttached: true,
        summary: { phase: 'recording', isRecording: true, armedTracks: [] },
        reviewing: true,
      }),
    ).toBe('recording');
  });
});

describe('createRecordingViewStore', () => {
  it('records intent without touching engine state', () => {
    const s = createRecordingViewStore();
    s.getState().actions.setMonitorMode('on');
    s.getState().actions.setCountInBars(2);
    s.getState().actions.setMetronome(true);
    s.getState().actions.openReview('take-1');
    expect(s.getState().monitorMode).toBe('on');
    expect(s.getState().countInBars).toBe(2);
    expect(s.getState().metronome).toBe(true);
    expect(s.getState().reviewTakeId).toBe('take-1');
    s.getState().actions.reset();
    expect(s.getState().reviewTakeId).toBeNull();
    expect(s.getState().countInBars).toBe(1);
  });

  it('clamps negative count-in to zero', () => {
    const s = createRecordingViewStore();
    s.getState().actions.setCountInBars(-3);
    expect(s.getState().countInBars).toBe(0);
  });
});
