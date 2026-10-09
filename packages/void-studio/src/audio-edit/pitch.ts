// Note-level audio pitch model (W19 / EDIT-02, FX-06; T74 model half).
//
// Pitch DETECTION and the correction/shift renderers are engine-resident
// DSP (docs/content-rights/NEEDS.md). This module is the editable model:
// per-note detected pitch (immutable original), correction and shift
// values, and the reversibility invariant — every edit keeps the
// detected source of truth so the model can always return to source
// exactly (T74 "nondestructive reversibility").
//
// Values are integer cents (exact wire data, no float drift).

/** One detected audio note (analysis product). */
export interface DetectedNote {
  /** Stable note id (caller-minted). */
  id: string;
  /** Clip-local start/length in ticks. */
  startTicks: string;
  lengthTicks: string;
  /** Detected centre pitch in cents (MIDI note * 100). IMMUTABLE. */
  detectedCents: number;
  /** Detection confidence 0..1 — low-confidence notes stay editable. */
  confidence?: number;
}

/** An editable pitch row: detected original + layered edits. */
export interface PitchNote {
  id: string;
  startTicks: string;
  lengthTicks: string;
  /** Source detection — never mutated after creation. */
  readonly detectedCents: number;
  /** In-place correction toward the detected pitch's target (cents). */
  correctionCents: number;
  /** Shift applied on top (cents) — transposition. */
  shiftCents: number;
  /** Formant shift in cents (FX-06 vocal transformation). */
  formantCents: number;
  confidence?: number;
}

/** assetSha -> per-clip edit document. */
export interface PitchEdit {
  /** SHA of the source asset the notes were detected on. */
  assetSha256: string;
  clipId: string;
  notes: PitchNote[];
}

/** Effective sounding pitch: detected + correction + shift. */
export function effectiveCents(n: PitchNote): number {
  return n.detectedCents + n.correctionCents + n.shiftCents;
}

/** Build the edit model from an analysis product. */
export function pitchEditFromDetection(
  clipId: string,
  assetSha256: string,
  detected: DetectedNote[],
): PitchEdit {
  return {
    assetSha256,
    clipId,
    notes: detected.map((d) => ({
      id: d.id,
      startTicks: d.startTicks,
      lengthTicks: d.lengthTicks,
      detectedCents: d.detectedCents,
      correctionCents: 0,
      shiftCents: 0,
      formantCents: 0,
      confidence: d.confidence,
    })),
  };
}

function withNote(edit: PitchEdit, noteId: string, f: (n: PitchNote) => PitchNote): PitchEdit {
  const idx = edit.notes.findIndex((n) => n.id === noteId);
  if (idx < 0) throw new Error(`note ${noteId} not in pitch edit`);
  const notes = [...edit.notes];
  const next = f(notes[idx]);
  if (next.detectedCents !== notes[idx].detectedCents) {
    // The invariant is structural, but belt-and-braces: a mutation that
    // tried to touch the original is refused outright.
    throw new Error('detectedCents is immutable — correction/shift only');
  }
  notes[idx] = next;
  return { ...edit, notes };
}

/** Set the in-place correction for one note (cents). */
export function correctNote(edit: PitchEdit, noteId: string, cents: number): PitchEdit {
  if (!Number.isFinite(cents)) throw new Error('correction must be finite cents');
  return withNote(edit, noteId, (n) => ({ ...n, correctionCents: Math.trunc(cents) }));
}

/** Set the transposition shift for one note (cents). */
export function shiftNote(edit: PitchEdit, noteId: string, cents: number): PitchEdit {
  if (!Number.isFinite(cents)) throw new Error('shift must be finite cents');
  return withNote(edit, noteId, (n) => ({ ...n, shiftCents: Math.trunc(cents) }));
}

/** Set the formant shift for one note (cents). */
export function setFormant(edit: PitchEdit, noteId: string, cents: number): PitchEdit {
  if (!Number.isFinite(cents)) throw new Error('formant must be finite cents');
  return withNote(edit, noteId, (n) => ({ ...n, formantCents: Math.trunc(cents) }));
}

/**
 * Snap one note to a target pitch (e.g. scale member): sets correction =
 * target - detected. Caller computes the musical target — this stays a
 * pure value model.
 */
export function correctTo(edit: PitchEdit, noteId: string, targetCents: number): PitchEdit {
  const note = edit.notes.find((n) => n.id === noteId);
  if (!note) throw new Error(`note ${noteId} not in pitch edit`);
  return correctNote(edit, noteId, targetCents - note.detectedCents);
}

/**
 * Revert one note to its detected source — correction and shift to zero.
 * `detectedCents` never moved, so this is an exact reversal, not a guess.
 */
export function resetNote(edit: PitchEdit, noteId: string): PitchEdit {
  return withNote(edit, noteId, (n) => ({
    ...n,
    correctionCents: 0,
    shiftCents: 0,
    formantCents: 0,
  }));
}

/** Revert every note — the whole region returns to source. */
export function resetAll(edit: PitchEdit): PitchEdit {
  return {
    ...edit,
    notes: edit.notes.map((n) => ({
      ...n,
      correctionCents: 0,
      shiftCents: 0,
      formantCents: 0,
    })),
  };
}

/** True when the edit currently sounds identical to the detection. */
export function isUnedited(edit: PitchEdit): boolean {
  return edit.notes.every(
    (n) => n.correctionCents === 0 && n.shiftCents === 0 && n.formantCents === 0,
  );
}

/**
 * The serializable op payload for the engine when the wire surface
 * exists — every value the renderer needs, with detected kept for
 * provenance (mirrors the fades/notes NEEDS pattern).
 */
export interface PitchEditSpec {
  clipId: string;
  assetSha256: string;
  notes: Array<{
    id: string;
    startTicks: string;
    lengthTicks: string;
    detectedCents: number;
    correctionCents: number;
    shiftCents: number;
    formantCents: number;
  }>;
}

export function pitchEditSpec(edit: PitchEdit): PitchEditSpec {
  return {
    clipId: edit.clipId,
    assetSha256: edit.assetSha256,
    notes: edit.notes.map((n) => ({
      id: n.id,
      startTicks: n.startTicks,
      lengthTicks: n.lengthTicks,
      detectedCents: n.detectedCents,
      correctionCents: n.correctionCents,
      shiftCents: n.shiftCents,
      formantCents: n.formantCents,
    })),
  };
}
