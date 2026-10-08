// Ghost-note preview — view-state-only data derived from a candidate.
// Ghost notes are NEVER applied to the engine and never cross into the
// persistent document: they exist so the piano roll can paint the
// audition. `ghost` stays true until the accept path mints real note
// ids — the marker is honest about transience (invariant: the store
// holds only display derivations, no authoritative data).

import type { ProposalCandidate, ProposedNote } from './types';

export interface GhostNote {
  /** 1-based index inside the candidate — the partial-accept key. */
  index: number;
  pitch: number;
  velocity: number;
  onsetTicks: string;
  lengthTicks: string;
  /** True when the note overlaps a locked range → painted muted,
   *  excluded from partial accept unless the caller drops them. */
  locked: boolean;
  /** Always true — marks the note as transient, never persistent. */
  ghost: true;
}

export interface TickRangeLike {
  startTicks: string;
  lengthTicks: string;
}

function overlaps(
  r: TickRangeLike,
  start: bigint,
  end: bigint,
): boolean {
  const rs = BigInt(r.startTicks);
  const re = rs + BigInt(r.lengthTicks);
  return rs < end && start < re;
}

/** Candidate → ghost notes for the roll. Pure derivation. */
export function ghostNotesOf(
  candidate: ProposalCandidate | undefined,
  lockedRanges: TickRangeLike[] = [],
): GhostNote[] {
  if (!candidate) return [];
  return candidate.notes.map((n: ProposedNote, i: number): GhostNote => {
    const start = BigInt(n.onsetTicks);
    const end = start + BigInt(n.lengthTicks);
    return {
      index: i,
      pitch: n.pitch,
      velocity: n.velocity,
      onsetTicks: n.onsetTicks,
      lengthTicks: n.lengthTicks,
      locked: lockedRanges.some((r) => overlaps(r, start, end)),
      ghost: true,
    };
  });
}
