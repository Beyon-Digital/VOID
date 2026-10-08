// Mixer controller — user gestures → wire ops with revision tracking
// (W10, T41; same editSession semantics as instruments).
//
// Optimistic policy mirrors the studio convention: the strip store is
// marked pending on send and updated only on APPLIED/DUPLICATE — a
// rejected receipt leaves the strip at its last accepted value.

import type { VoidClient } from 'void-client';
import {
  sendWithStaleRetry,
  type EditOutcome,
} from '../workspaces/editSession';
import { setTrackGainOp, setTrackPanOp, setTrackMuteOp, setTrackSoloOp, dbToGain } from './ops';
import type { MixerViewStore } from './model';

export interface MixerBinding {
  client: VoidClient;
  store: MixerViewStore;
  /** Re-read TRACK_LIST before a stale retry. */
  refreshTracks: () => Promise<unknown> | unknown;
  transactionId?: string;
}

function settled(outcome: EditOutcome): boolean {
  return outcome.receipt.status === 'APPLIED' || outcome.receipt.status === 'DUPLICATE';
}

/** Linear-gain set. `sent` is what goes on the wire. */
export async function setGain(b: MixerBinding, trackId: string, gainLinear: number) {
  const op = setTrackGainOp(trackId, gainLinear); // validates first
  b.store.getState().actions.noteSent(trackId, 'gain');
  const outcome = await sendWithStaleRetry(b.client, op, {
    transactionId: b.transactionId,
    refresh: b.refreshTracks,
  });
  if (settled(outcome)) {
    b.store.getState().actions.noteAccepted(trackId, { gainLinear });
  } else {
    b.store.getState().actions.noteRejected(trackId);
  }
  return outcome;
}

/** Same, given dB. */
export async function setGainDb(b: MixerBinding, trackId: string, db: number) {
  return setGain(b, trackId, dbToGain(db));
}

export async function setPan(b: MixerBinding, trackId: string, pan: number) {
  const op = setTrackPanOp(trackId, pan);
  b.store.getState().actions.noteSent(trackId, 'pan');
  const outcome = await sendWithStaleRetry(b.client, op, {
    transactionId: b.transactionId,
    refresh: b.refreshTracks,
  });
  if (settled(outcome)) {
    b.store.getState().actions.noteAccepted(trackId, { pan });
  } else {
    b.store.getState().actions.noteRejected(trackId);
  }
  return outcome;
}

export async function setMuted(b: MixerBinding, trackId: string, muted: boolean) {
  b.store.getState().actions.noteSent(trackId, 'mute');
  const outcome = await sendWithStaleRetry(b.client, setTrackMuteOp(trackId, muted), {
    transactionId: b.transactionId,
    refresh: b.refreshTracks,
  });
  if (settled(outcome)) {
    b.store.getState().actions.noteAccepted(trackId, { muted });
  } else {
    b.store.getState().actions.noteRejected(trackId);
  }
  return outcome;
}

/**
 * Solo is NOT inverted-mute logic and carries no "only one solo" client
 * rule — the engine decides exclusive-solo semantics; the UI forwards
 * the user's toggle verbatim.
 */
export async function setSoloed(b: MixerBinding, trackId: string, soloed: boolean) {
  b.store.getState().actions.noteSent(trackId, 'solo');
  const outcome = await sendWithStaleRetry(b.client, setTrackSoloOp(trackId, soloed), {
    transactionId: b.transactionId,
    refresh: b.refreshTracks,
  });
  if (settled(outcome)) {
    b.store.getState().actions.noteAccepted(trackId, { soloed });
  } else {
    b.store.getState().actions.noteRejected(trackId);
  }
  return outcome;
}
