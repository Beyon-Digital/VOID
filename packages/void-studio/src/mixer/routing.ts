// Bus / AUX routing model (W10, T41).
//
// What the schema actually provides (protocol/void_control.fbs,
// verified): AddTrackOp can create a track of kind 'BUS' — that is the
// full routing vocabulary. There is NO SetSendOp / route op variant, so
// an AUX send cannot be expressed on the wire yet. This module models
// the honest subset — creating/addressing buses — and reports the send
// gap as a typed capability error instead of pretending sends work.

import type { PersistentOp } from 'void-client';

/** Thrown when the UI asks for routing the protocol cannot express. */
export class UnsupportedCapability extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'UnsupportedCapability';
  }
}

/** AddTrackOp creating a BUS track (the only routing op that exists). */
export function addBusTrackOp(trackId: string, name: string, index?: number): PersistentOp {
  return {
    AddTrackOp: { track_id: trackId, kind: 'BUS', name, index },
  };
}

export interface RouteModel {
  /** Track ids of BUS tracks (mix destinations). */
  buses: string[];
  /**
   * Sends are not expressible: every track feeds the master bus
   * implicitly until a send/route op lands in the protocol.
   */
  sendsExpressible: false;
}

/**
 * Derive the routing view from TRACK_LIST read items. Track summaries
 * are engine-defined JSON; we read `kind` defensively and surface only
 * what exists.
 */
export function routeModelFromTrackItems(items: { summary_json: string }[]): RouteModel {
  const buses: string[] = [];
  for (const it of items) {
    try {
      const s = JSON.parse(it.summary_json) as { kind?: string; track_id?: string; id?: string };
      if (s.kind === 'BUS') buses.push(s.track_id ?? s.id ?? '');
    } catch {
      // malformed summary — skip; never invent routing state
    }
  }
  return { buses: buses.filter(Boolean), sendsExpressible: false };
}

/**
 * Would-be send builder — kept as an explicit, typed refusal so the
 * moment a send op lands in the schema this call site is the only place
 * that changes. Passing `amount` is validated anyway so call sites get
 * their argument checking for free.
 */
export function routeSendOp(
  _fromTrackId: string,
  _toBusId: string,
  amount = 1,
): PersistentOp {
  if (!Number.isFinite(amount) || amount < 0) {
    throw new Error(`send amount must be finite and >= 0, got ${amount}`);
  }
  throw new UnsupportedCapability(
    'no send/route op in void_control.fbs — bus tracks exist but AUX sends are not expressible on the wire yet',
  );
}
