// Project/track note DTO shapes (W11 item 2, DOC-05).
//
// PROTOCOL GAP: protocol major.1 has no SetProjectNoteOp/SetTrackNoteOp
// in the PersistentOp union and no note field in the ReadItem summaries
// the schema pins. Recorded in docs/engine/NEEDS.md §rev2 — until the ops
// land, these builders document the *intended* payload (same pattern as
// jobs/job.ts for SubmitJobOp) and are never sent through send_command:
// the major.1 codec would reject them as unknown op variants.
//
// `NOTE_OPS_AVAILABLE` is the single honest feature flag the UI gates on.

export const NOTE_OPS_AVAILABLE = false;

/** Intended shape of the project-note persistent op (proposed, rev2). */
export interface SetProjectNotePayload {
  text: string;
}
/** Intended shape of the track-note persistent op (proposed, rev2). */
export interface SetTrackNotePayload {
  track_id: string;
  text: string;
}

/** The proposed tagged payload — NOT a `PersistentOp` on major.1. */
export function proposedSetProjectNoteOp(text: string): { SetProjectNoteOp: SetProjectNotePayload } {
  return { SetProjectNoteOp: { text } };
}

export function proposedSetTrackNoteOp(
  trackId: string,
  text: string,
): { SetTrackNoteOp: SetTrackNotePayload } {
  return { SetTrackNoteOp: { track_id: trackId, text } };
}
