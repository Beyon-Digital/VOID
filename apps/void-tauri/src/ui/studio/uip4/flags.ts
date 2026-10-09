// Honest capability surface for the proposal / generation / jobs screens.
//
// These constants name wire gaps documented in docs/engine/NEEDS.md —
// they are NOT invented flags. Every gated affordance renders its
// reason so the screen is never silently dead.

/** Coordinator ops/view the proposal lifecycle needs but protocol
 * major.1 does not carry (NEEDS.md §12–14). */
export const PROPOSAL_OPS = {
  /** RequestProposalOp — ask the engine for a continuation. */
  request: false,
  /** ResolveProposalOp — server-side accept/reject bookkeeping. */
  resolve: false,
  /** PreviewLayerOp — native audition of ghost notes. */
  audition: false,
  /** ProposalStaleEvent — push staleness; we detect locally instead. */
  staleEvent: false,
} as const;

/** Job ops + views absent from protocol major.1 (NEEDS.md §9–11). */
export const JOB_OPS = {
  /** SubmitJob persistent op. */
  submit: false,
  /** CancelJob persistent op. */
  cancel: false,
  /** JOB_LIST / MODEL_LIST read views. */
  listViews: false,
  /** Model install/download op. */
  modelInstall: false,
  /** Pause optional jobs op. */
  pause: false,
} as const;

export const REASONS = {
  proposalRequest:
    'Requesting a suggestion needs the coordinator proposal ops (docs/engine/NEEDS.md §12–14) — not in this build.',
  proposalResolve:
    'Server-side resolve op absent (NEEDS.md §14) — the preview was dropped locally only.',
  audition:
    'Audio audition needs the engine preview layer (NEEDS.md §12) — not in this build. Ghost notes are shown visually only.',
  staleEvent:
    'No ProposalStaleEvent on the wire (NEEDS.md §14) — staleness is detected locally from revision + context.',
  jobSubmit:
    'Submitting a generation job needs the coordinator SubmitJob op (NEEDS.md §9) — not in this build.',
  jobCancel:
    'Cancelling a job needs the coordinator CancelJob op (NEEDS.md §9) — not in this build.',
  jobPause:
    'Pausing optional jobs needs a coordinator op that is not in this build.',
  modelInstall:
    'Model installs need a coordinator op (NEEDS.md §9–11) — nothing downloads automatically.',
  modelList:
    'No MODEL_LIST read view (NEEDS.md §10) — this list only shows entries the coordinator pushes.',
  assetAudition:
    'Candidate playback needs a preview layer (NEEDS.md §12) — assets cannot be streamed to the UI yet.',
} as const;
