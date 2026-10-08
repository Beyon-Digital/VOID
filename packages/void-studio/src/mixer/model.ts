// Mixer view model (W10, T41).
//
// Strips are a *projection*: they come from TRACK_LIST read-view items
// plus the telemetry meter map — never from a document clone. A strip
// carries exactly what its read item declared (kind/name) plus the last
// accepted mixer values the UI sent (or that the summary carried), and
// the honest meter state from meters.ts.

import type { MeterFrame } from 'void-client';
import { createStore, StoreApi } from 'zustand/vanilla';
import { meterStrip, MeterStripState } from './meters';

export interface MixerStripModel {
  trackId: string;
  kind: string;
  name: string;
  /** Last known mixer values (sent or read); undefined = not yet known. */
  gainLinear?: number;
  pan?: number;
  muted?: boolean;
  soloed?: boolean;
  /** Set when a send is in flight — UI may show pending state. */
  pending?: 'gain' | 'pan' | 'mute' | 'solo';
}

/** Parse one TRACK_LIST ReadItem into a strip projection (defensive). */
export function stripFromSummary(objectId: string, summaryJson: string): MixerStripModel | null {
  try {
    const s = JSON.parse(summaryJson) as {
      kind?: string;
      name?: string;
      track_id?: string;
      gain_linear?: number;
      pan?: number;
      muted?: boolean;
      soloed?: boolean;
    };
    const kind = typeof s.kind === 'string' ? s.kind : 'AUDIO';
    if (!['AUDIO', 'MIDI', 'INSTRUMENT', 'BUS'].includes(kind)) return null;
    return {
      trackId: s.track_id ?? objectId,
      kind,
      name: typeof s.name === 'string' ? s.name : '(unnamed)',
      gainLinear: typeof s.gain_linear === 'number' ? s.gain_linear : undefined,
      pan: typeof s.pan === 'number' ? s.pan : undefined,
      muted: typeof s.muted === 'boolean' ? s.muted : undefined,
      soloed: typeof s.soloed === 'boolean' ? s.soloed : undefined,
    };
  } catch {
    return null; // malformed summary is not a strip — never fabricate one
  }
}

/** Strips for a TRACK_LIST page — order preserved, buses last. */
export function stripsFromTrackItems(
  items: { object_id: string; summary_json: string }[],
): MixerStripModel[] {
  const out: MixerStripModel[] = [];
  for (const it of items) {
    const s = stripFromSummary(it.object_id, it.summary_json);
    if (s) out.push(s);
  }
  return out;
}

// ---------------------------------------------------------------------------
// Mixer panel store — view state only (sent values + pending flags)
// ---------------------------------------------------------------------------

export interface MixerViewState {
  strips: MixerStripModel[];
  /** Meter arrival timestamps per track (MeterClock feeds honesty). */
  meterAt: Record<string, number>;
}

export interface MixerViewActions {
  setStrips(strips: MixerStripModel[]): void;
  noteSent(trackId: string, field: 'gain' | 'pan' | 'mute' | 'solo'): void;
  noteAccepted(
    trackId: string,
    patch: Partial<Pick<MixerStripModel, 'gainLinear' | 'pan' | 'muted' | 'soloed'>>,
  ): void;
  noteRejected(trackId: string): void;
  noteMeter(trackId: string, atMs: number): void;
  meterFor(trackId: string, frames: Record<string, MeterFrame>, nowMs: number): MeterStripState;
  reset(): void;
}

export type MixerViewStore = StoreApi<MixerViewState & { actions: MixerViewActions }>;

export function createMixerViewStore(
  init?: Partial<MixerViewState>,
): MixerViewStore {
  const base: MixerViewState = { strips: [], meterAt: {}, ...init };
  return createStore<MixerViewState & { actions: MixerViewActions }>()((set, get) => ({
    ...base,
    actions: {
      setStrips: (strips) => set({ strips }),
      noteSent: (trackId, field) =>
        set((s) => ({
          strips: s.strips.map((x) =>
            x.trackId === trackId ? { ...x, pending: field } : x,
          ),
        })),
      noteAccepted: (trackId, patch) =>
        set((s) => ({
          strips: s.strips.map((x) =>
            x.trackId === trackId ? { ...x, ...patch, pending: undefined } : x,
          ),
        })),
      noteRejected: (trackId) =>
        set((s) => ({
          // Revert to last accepted values — drop pending, change nothing.
          strips: s.strips.map((x) =>
            x.trackId === trackId ? { ...x, pending: undefined } : x,
          ),
        })),
      noteMeter: (trackId, atMs) =>
        set((s) => ({ meterAt: { ...s.meterAt, [trackId]: atMs } })),
      meterFor: (trackId, frames, nowMs) =>
        meterStrip(frames[trackId], nowMs, get().meterAt[trackId]),
      reset: () => set({ strips: [], meterAt: {} }),
    },
  }));
}
