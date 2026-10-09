// Proposal wire types — mirrors crates/void-proposals serde exactly
// (camelCase, tick fields as decimal strings). Records are authoritative
// data from the coordinator; everything here parses defensively.

export type ProposalStatus =
  | 'pending'
  | 'ready'
  | 'stale'
  | 'accepted'
  | 'rejected'
  | 'failed';

export type StaleCause =
  | 'context_changed'
  | 'target_gone'
  | 'session_ended'
  | 'superseded'
  | 'cancelled';

export interface ProposedNote {
  pitch: number;
  velocity: number;
  onsetTicks: string;
  lengthTicks: string;
}

export interface ProposalCandidate {
  rank: number;
  /** Model's own sample likelihood — not a confidence claim. */
  score: number;
  rationale: string;
  notes: ProposedNote[];
}

export interface ProposalProvenance {
  jobId: string;
  generatorId: string;
  generatorVersion: string;
  modelId: string;
  runtimeId: string;
  runtimeSha256: string;
  /** Deterministic seed — same context + seed ⇒ same proposals. */
  seed: string;
  documentSha256: string;
  analysis: unknown;
  documentAsset: string;
}

export interface ProposalRecord {
  proposalId: string;
  projectId: string;
  sourceRevision: string;
  contextSha256: string;
  status: ProposalStatus;
  staleCause?: StaleCause;
  /** Scoped context echoed from the record (audit + ghost lock paint). */
  context?: {
    clipId?: string;
    lockedRanges?: { startTicks: string; lengthTicks: string }[];
    [k: string]: unknown;
  };
  candidates: ProposalCandidate[];
  provenance?: ProposalProvenance;
  accepted?: {
    transactionId: string;
    candidateRank: number;
    noteIds: string[];
    droppedIndices: number[];
    acceptedAt: string;
  };
  supersedes?: string;
  error?: string;
  createdAt: string;
  updatedAt: string;
}

const STATUS: ProposalStatus[] = [
  'pending',
  'ready',
  'stale',
  'accepted',
  'rejected',
  'failed',
];

function note(v: unknown): ProposedNote | null {
  const n = v as Partial<ProposedNote>;
  if (
    typeof n?.pitch !== 'number' ||
    typeof n?.velocity !== 'number' ||
    typeof n?.onsetTicks !== 'string' ||
    typeof n?.lengthTicks !== 'string'
  )
    return null;
  return {
    pitch: n.pitch,
    velocity: n.velocity,
    onsetTicks: n.onsetTicks,
    lengthTicks: n.lengthTicks,
  };
}

export function parseCandidate(v: unknown): ProposalCandidate | null {
  const c = v as Partial<ProposalCandidate>;
  if (typeof c?.rank !== 'number' || typeof c?.score !== 'number') return null;
  if (!Array.isArray(c.notes)) return null;
  const notes = c.notes.map(note);
  if (notes.some((n) => n === null)) return null;
  return {
    rank: c.rank,
    score: c.score,
    rationale: typeof c.rationale === 'string' ? c.rationale : '',
    notes: notes as ProposedNote[],
  };
}

export function parseProposalRecord(v: unknown): ProposalRecord | null {
  const r = v as Partial<ProposalRecord>;
  if (
    typeof r?.proposalId !== 'string' ||
    typeof r?.projectId !== 'string' ||
    !STATUS.includes(r.status as ProposalStatus)
  )
    return null;
  const candidates = (r.candidates ?? [])
    .map(parseCandidate)
    .filter((c): c is ProposalCandidate => c !== null);
  return {
    proposalId: r.proposalId,
    projectId: r.projectId,
    sourceRevision: typeof r.sourceRevision === 'string' ? r.sourceRevision : '0',
    contextSha256: typeof r.contextSha256 === 'string' ? r.contextSha256 : '',
    status: r.status as ProposalStatus,
    staleCause: r.staleCause,
    context: r.context as ProposalRecord['context'],
    candidates,
    provenance: r.provenance,
    accepted: r.accepted,
    supersedes: r.supersedes,
    error: r.error,
    createdAt: typeof r.createdAt === 'string' ? r.createdAt : '',
    updatedAt: typeof r.updatedAt === 'string' ? r.updatedAt : '',
  };
}
