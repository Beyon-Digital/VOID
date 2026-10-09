// Honest capability flags for the notation surface (W25).
//
// A flag is only ever `true` when the implementation behind it is REAL
// on this lane. UI code must gate on these constants — never on its own
// guesses about platform support.

/**
 * Engraving / rendered score view. The lane decision is Verovio
 * (WORK_PACKAGES VIS-04), which needs the native engine + GUI surface —
 * this Linux lane implements the model + interchange only.
 * Recorded in docs/notation/NEEDS.md.
 */
export const ENGRAVING_AVAILABLE = false;

/**
 * Whether a protocol op carries score ops to the coordinator today.
 * The op-plan layer is real (`crates/void-notation`), but protocol
 * major.1 has no score op variant — the plans the store stages are
 * honest wire DTOs for the binding the integrator adds.
 * Recorded in docs/notation/NEEDS.md.
 */
export const SCORE_OPS_WIRE_AVAILABLE = false;

/**
 * MusicXML 4 import/export — real in `crates/void-notation`
 * (round-trip tested); the file-io entry point is the coordinator's.
 */
export const MUSICXML_INTERCHANGE_AVAILABLE = true;

/**
 * AAF / Final Cut Pro XML interchange for scoring. Proprietary-side
 * conversion stays blocked on this lane — no converter exists and no
 * supported flag is claimed.
 */
export const AAF_INTERCHANGE_AVAILABLE = false;
export const FINALCUT_INTERCHANGE_AVAILABLE = false;

/** Apple .logicx project interchange — proprietary, stays blocked. */
export const LOGICX_INTERCHANGE_AVAILABLE = false;

/** Movie-scoring anchors + declared timecode modes (real in the crate;
 *  the film/smtp-sync surface is coordinator-side). */
export const SCORE_ANCHORS_AVAILABLE = true;
