// Recording view-model (lane UIP1; S05 record + S15 audio/MIDI setup).
//
// View-state only: parses the `recording` summary the engine emits inside
// PROJECT_SUMMARY (EngineSession::recordingSummaryJson) into the UI state
// machine from the record screen's interaction contract
// (unavailable / ready / armed / countIn / recording / stopping / review /
// failed) and holds the user's not-yet-applied control intent (monitor mode,
// count-in, metronome, punch). Native wire ops for arm/start/stop/monitor/
// count-in/punch do not exist yet — docs/engine/NEEDS.md §7 — so the store
// records intent only; screens must render controls as unavailable rather
// than simulate transport. STOP is the one real op today (sendTransport).

import { createStore, StoreApi } from 'zustand/vanilla';

/** Engine phases per native/void-engine/src/recording/RecordingManager.h. */
export type RecordingPhase = 'idle' | 'armed' | 'recording' | 'stopping' | 'failed';

/** Shape emitted as `recording` inside PROJECT_SUMMARY item[0]. */
export interface RecordingSummary {
  phase: RecordingPhase;
  isRecording: boolean;
  /** Track voidIds with recordEnabled set (RecordingManager::armedTrackItemIds). */
  armedTracks: string[];
  takeId?: string;
  lastError?: string;
}

const PHASES: ReadonlySet<string> = new Set(['idle', 'armed', 'recording', 'stopping', 'failed']);

/** Defensive parse of the `recording` field from a PROJECT_SUMMARY item.
 *  Returns undefined when the payload is absent/malformed — callers treat
 *  that as "engine did not report", distinct from a reported idle state. */
export function parseRecordingSummary(raw: unknown): RecordingSummary | undefined {
  if (typeof raw !== 'object' || raw === null) return undefined;
  const r = raw as Record<string, unknown>;
  const phase = typeof r.phase === 'string' && PHASES.has(r.phase) ? (r.phase as RecordingPhase) : 'idle';
  const armedTracks = Array.isArray(r.armedTracks)
    ? r.armedTracks.filter((t): t is string => typeof t === 'string')
    : [];
  const out: RecordingSummary = {
    phase,
    isRecording: r.isRecording === true,
    armedTracks,
  };
  if (typeof r.takeId === 'string' && r.takeId.length > 0) out.takeId = r.takeId;
  if (typeof r.lastError === 'string' && r.lastError.length > 0) out.lastError = r.lastError;
  return out;
}

/** Extract + parse the recording block from raw PROJECT_SUMMARY items. */
export function recordingFromSummaryItems(items: unknown): RecordingSummary | undefined {
  if (!Array.isArray(items) || items.length === 0) return undefined;
  const first = items[0];
  if (typeof first !== 'object' || first === null) return undefined;
  return parseRecordingSummary((first as Record<string, unknown>).recording);
}

/** UI state machine from docs/ui-handoff/VOID_Design_System_and_Screen_Map.json
 *  interactionContracts.recording. `review` is UI-side: entered when a
 *  stopped take has retained output the user can inspect (UI-T10). */
export type RecordingUiState =
  | 'unavailable'
  | 'ready'
  | 'armed'
  | 'countIn'
  | 'recording'
  | 'stopping'
  | 'review'
  | 'failed';

export function recordingUiState(args: {
  engineAttached: boolean;
  summary?: RecordingSummary;
  /** Set when the UI is showing the post-stop take review (UI-T10). */
  reviewing?: boolean;
}): RecordingUiState {
  const { engineAttached, summary, reviewing } = args;
  if (!engineAttached) return 'unavailable';
  if (summary === undefined) return 'ready';
  switch (summary.phase) {
    case 'recording':
      return 'recording';
    case 'stopping':
      return 'stopping';
    case 'failed':
      return 'failed';
    case 'armed':
      return 'armed';
    case 'idle':
      return reviewing ? 'review' : 'ready';
  }
}

/** Monitor modes per RecordingManager::Monitor. */
export type MonitorMode = 'off' | 'automatic' | 'on';

export interface RecordingViewState {
  /** Intent only — no SetMonitorOp on the wire yet (NEEDS §7). */
  monitorMode: MonitorMode;
  /** Intent only — no SetCountInOp on the wire yet (NEEDS §7). */
  countInBars: number;
  /** Intent only — no SetMetronomeOp on the wire yet (NEEDS §7). */
  metronome: boolean;
  /** Post-stop review focus (UI-T10): the takeId being reviewed, or null. */
  reviewTakeId: string | null;
}

export interface RecordingViewActions {
  setMonitorMode(mode: MonitorMode): void;
  setCountInBars(bars: number): void;
  setMetronome(on: boolean): void;
  openReview(takeId: string | null): void;
  reset(): void;
}

export type RecordingViewStore = StoreApi<RecordingViewState & { actions: RecordingViewActions }>;

export function createRecordingViewStore(): RecordingViewStore {
  return createStore<RecordingViewState & { actions: RecordingViewActions }>()((set) => ({
    monitorMode: 'automatic',
    countInBars: 1,
    metronome: false,
    reviewTakeId: null,
    actions: {
      setMonitorMode: (monitorMode) => set({ monitorMode }),
      setCountInBars: (countInBars) =>
        set({ countInBars: Number.isFinite(countInBars) && countInBars >= 0 ? Math.floor(countInBars) : 0 }),
      setMetronome: (metronome) => set({ metronome }),
      openReview: (reviewTakeId) => set({ reviewTakeId }),
      reset: () => set({ monitorMode: 'automatic', countInBars: 1, metronome: false, reviewTakeId: null }),
    },
  }));
}
