// Producer op builders (W21) — the DTO payloads the coordinator will
// consume. PROTOCOL GAP (recorded in docs/producer/NEEDS.md): wire has
// no accompaniment/mastering op members yet — these builders emit the
// honest intended shapes so the request is modelled today and lands
// byte-identical when the ops exist. Everything is validated first.

import { i64str, u64str } from 'void-client';
import { validateSpec, type AccompanimentSpec } from './types';

export interface RequestAccompanimentOp {
  op: 'request_accompaniment';
  spec: AccompanimentSpec;
}

export function requestAccompanimentOp(
  spec: AccompanimentSpec,
): RequestAccompanimentOp {
  const err = validateSpec(spec);
  if (err) throw new Error(`invalid accompaniment spec: ${err}`);
  return {
    op: 'request_accompaniment',
    spec: {
      ...spec,
      seed: u64str(spec.seed),
      outputStartTicks: i64str(spec.outputStartTicks),
      outputLengthTicks: i64str(spec.outputLengthTicks),
    },
  };
}

export interface AcceptMasteringOp {
  op: 'accept_mastering';
  proposalId: string;
}

export function acceptMasteringOp(proposalId: string): AcceptMasteringOp {
  if (!/^mprop_[\da-f]{32}$/.test(proposalId) && !proposalId) {
    throw new Error('bad proposal id');
  }
  if (!proposalId) throw new Error('empty proposal id');
  return { op: 'accept_mastering', proposalId };
}

export interface RejectMasteringOp {
  op: 'reject_mastering';
  proposalId: string;
}

export function rejectMasteringOp(proposalId: string): RejectMasteringOp {
  if (!proposalId) throw new Error('empty proposal id');
  return { op: 'reject_mastering', proposalId };
}

/** A/B audition is a transport-level request: play A (source) or B
 * (ops rendered) level-matched. Spec, not audio — the render is a
 * needs-doc gap. */
export interface AuditionOp {
  op: 'mastering_audition';
  proposalId: string;
  side: 'a' | 'b';
}

export function auditionOp(proposalId: string, side: 'a' | 'b'): AuditionOp {
  if (!proposalId) throw new Error('empty proposal id');
  return { op: 'mastering_audition', proposalId, side };
}
