// Honest capability surface for the proposal / generation / jobs screens.
//
// rev-2 (protocol minor 1): every op + view below now exists on the wire
// (docs/engine/NEEDS.md §7–14, §17, §26, §32 — all landed by the P1 lane:
// protocol/void_control.fbs rev-2 union members, void-client SendableOp).
// The flags remain as the single flip point; when a future regression or
// an older coordinator drops the surface, set the flag back to false and
// the screens render the REASONS text again instead of dead controls.

/** Coordinator ops/view the proposal lifecycle uses (NEEDS.md §12–14). */
export const PROPOSAL_OPS = {
  /** RequestProposalOp — ask the engine for a continuation. */
  request: true,
  /** ResolveProposalOp — server-side accept/reject bookkeeping. */
  resolve: true,
  /** PreviewLayerOp — native audition of ghost notes. */
  audition: true,
  /** ProposalStaleEvent — push staleness (kept alongside local detection). */
  staleEvent: true,
} as const;

/** Job ops + views on the wire since rev-2 (NEEDS.md §9–11). */
export const JOB_OPS = {
  /** SubmitJob persistent op. */
  submit: true,
  /** CancelJob persistent op. */
  cancel: true,
  /** JOB_LIST / MODEL_LIST read views. */
  listViews: true,
  /** InstallModelOp — download/install a registry model. */
  modelInstall: true,
  /** PauseJobOp — pause optional jobs (coordinator may reject). */
  pause: true,
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
