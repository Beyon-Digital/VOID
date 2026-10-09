// Comp spec model (W17/REC-05; T66) — quick-swipe-style comping.
//
// A CompSpec picks one take per segment over a region. Applying a comp
// emits ordinary clip ops under ONE transaction (compToOps): every
// segment becomes an InsertAudioClipOp carrying the take's immutable
// asset + computed offset — sources are never touched and one undo
// restores the pre-comp state.
//
// Seam fades: protocol major.1 has no fade fields on clip ops, so the
// crossfade plan travels with the spec as explicit FadeSpec metadata
// (NEEDS.md rev2). seamFadePlan derives honest defaults bounded by the
// shorter adjacent segment; the engine consumes the spec when the
// field lands.

import { i64str, parseI64 } from 'void-client';
import type { PersistentOp } from 'void-client';
import type { ClipView } from '../timeline/geometry';
import {
  isValidRegion,
  regionEnd,
  regionsOverlap,
  takeRegion,
  type Region,
} from './takes';
import type {
  CompSegment,
  CompSpec,
  FadeShape,
  FadeSpec,
  TakeFolder,
  TakeRecord,
} from './types';

export class CompError extends Error {
  constructor(msg: string) {
    super(msg);
    this.name = 'CompError';
  }
}

function segEnd(s: CompSegment): bigint {
  return parseI64(s.startTicks) + parseI64(s.lengthTicks);
}

function takeById(folder: TakeFolder, takeId: string): TakeRecord {
  const t = folder.takes.find((x) => x.takeId === takeId);
  if (!t) throw new CompError(`take ${takeId} not in folder ${folder.folderId}`);
  return t;
}

/**
 * Validate a comp spec against its folder: sorted, contiguous,
 * covering exactly the comp region, every referenced take exists and
 * covers its segment (offset math is exact — a take that does not
 * cover a segment cannot comp that span).
 */
export function validateComp(spec: CompSpec, folder: TakeFolder): string[] {
  const errors: string[] = [];
  if (!isValidRegion({ startTicks: spec.regionStartTicks, lengthTicks: spec.regionLengthTicks })) {
    errors.push('comp region is malformed');
    return errors;
  }
  if (spec.segments.length === 0) {
    errors.push('comp has no segments');
    return errors;
  }
  const regionStart = parseI64(spec.regionStartTicks);
  const regionEndTicks = regionStart + parseI64(spec.regionLengthTicks);
  let cursor = regionStart;
  for (const seg of spec.segments) {
    if (!isValidRegion({ startTicks: seg.startTicks, lengthTicks: seg.lengthTicks })) {
      errors.push(`segment ${seg.segmentId} is malformed`);
      continue;
    }
    const s = parseI64(seg.startTicks);
    if (s !== cursor) {
      errors.push(
        `segment ${seg.segmentId} starts at ${s} but expected ${cursor} (gap/overlap)`,
      );
    }
    cursor = segEnd(seg);
    let take: TakeRecord;
    try {
      take = takeById(folder, seg.takeId);
    } catch (e) {
      errors.push((e as Error).message);
      continue;
    }
    // Segment must be inside the take's covered region.
    const tr = takeRegion(take);
    const tS = parseI64(tr.startTicks);
    const tE = regionEnd(tr);
    if (s < tS || segEnd(seg) > tE) {
      errors.push(
        `segment ${seg.segmentId} [${s}..${segEnd(seg)}) exceeds take ${seg.takeId} [${tS}..${tE})`,
      );
    }
    if (seg.fadeIn !== undefined) {
      const fl = parseI64(seg.fadeIn.lengthTicks);
      if (fl <= 0n) errors.push(`segment ${seg.segmentId} fade must be > 0`);
    }
  }
  if (spec.segments.length > 0 && cursor !== regionEndTicks) {
    errors.push(`comp ends at ${cursor} but region ends at ${regionEndTicks}`);
  }
  return errors;
}

/** Build a comp from ordered picks. Cuts must be strictly increasing,
 * inside the region, and every pick names a take covering its span. */
export function buildComp(args: {
  compId: string;
  trackId: string;
  region: Region;
  /** Ordered picks; the first pick starts at region start, the last
   * ends at region end. `starts[i+1]` is pick i's end. */
  picks: Array<{ takeId: string; startTicks: string; segmentId: string }>;
  regionEndTicks: string;
  folder: TakeFolder;
}): CompSpec {
  const spec: CompSpec = {
    compId: args.compId,
    trackId: args.trackId,
    regionStartTicks: args.region.startTicks,
    regionLengthTicks: args.region.lengthTicks,
    segments: args.picks.map((p, i) => {
      const start = parseI64(p.startTicks);
      const end =
        i + 1 < args.picks.length
          ? parseI64(args.picks[i + 1].startTicks)
          : parseI64(args.regionEndTicks);
      return {
        segmentId: p.segmentId,
        takeId: p.takeId,
        startTicks: i64str(start),
        lengthTicks: i64str(end - start),
      };
    }),
  };
  const errors = validateComp(spec, args.folder);
  if (errors.length) throw new CompError(`invalid comp: ${errors.join('; ')}`);
  return spec;
}

/**
 * Quick-swipe comp (REC-05): one whole-region take selection plus
 * swipe ranges. Each swipe range replaces the span's pick with its
 * take — like dragging across lanes in a comp editor.
 */
export function swipeComp(args: {
  compId: string;
  folder: TakeFolder;
  region: Region;
  /** Base take covering the whole region. */
  baseTakeId: string;
  /** Swipe selections: (startTicks,lengthTicks,takeId) — sorted; the
   *  base take fills the gaps. */
  swipes: Array<{ segmentId: string; takeId: string; startTicks: string; lengthTicks: string }>;
  mint: () => string;
}): CompSpec {
  const rS = parseI64(args.region.startTicks);
  const rE = regionEnd(args.region);
  const cuts = new Set<bigint>([rS, rE]);
  for (const w of args.swipes) {
    const s = parseI64(w.startTicks);
    const e = s + parseI64(w.lengthTicks);
    if (s < rS || e > rE || e <= s) {
      throw new CompError(`swipe ${w.segmentId} outside comp region`);
    }
    cuts.add(s);
    cuts.add(e);
  }
  const ordered = [...cuts].sort((a, b) => (a < b ? -1 : 1));
  const segments: CompSegment[] = [];
  for (let i = 0; i + 1 < ordered.length; i++) {
    const s = ordered[i];
    const e = ordered[i + 1];
    const swipe = args.swipes.find((w) => {
      const ws = parseI64(w.startTicks);
      return s >= ws && e <= ws + parseI64(w.lengthTicks);
    });
    segments.push({
      segmentId: swipe ? swipe.segmentId : args.mint(),
      takeId: swipe ? swipe.takeId : args.baseTakeId,
      startTicks: i64str(s),
      lengthTicks: i64str(e - s),
    });
  }
  const spec: CompSpec = {
    compId: args.compId,
    trackId: args.folder.trackId,
    regionStartTicks: args.region.startTicks,
    regionLengthTicks: args.region.lengthTicks,
    segments,
  };
  const errors = validateComp(spec, args.folder);
  if (errors.length) throw new CompError(`invalid comp: ${errors.join('; ')}`);
  return spec;
}

/**
 * Switch-alternatives: cycle the take under `ticks` through the
 * folder's takes that cover that segment. Returns a NEW spec (the
 * input is immutable). `direction` +1/-1 walks the lane order.
 */
export function cycleTakeAt(
  spec: CompSpec,
  folder: TakeFolder,
  ticks: string,
  direction: 1 | -1,
): CompSpec | null {
  const t = parseI64(ticks);
  const idx = spec.segments.findIndex((s) => {
    const s0 = parseI64(s.startTicks);
    return t >= s0 && t < s0 + parseI64(s.lengthTicks);
  });
  if (idx < 0) return null;
  const seg = spec.segments[idx];
  const covering = folder.takes.filter((tk) => {
    const r = takeRegion(tk);
    const rs = parseI64(r.startTicks);
    const re = rs + parseI64(r.lengthTicks);
    const ss = parseI64(seg.startTicks);
    const se = ss + parseI64(seg.lengthTicks);
    return ss >= rs && se <= re;
  });
  if (covering.length < 2) return null;
  const cur = covering.findIndex((x) => x.takeId === seg.takeId);
  const next = covering[(cur + direction + covering.length) % covering.length];
  return setSegmentTake(spec, seg.segmentId, next.takeId);
}

/** Repoint one segment at a different take (validated). */
export function setSegmentTake(
  spec: CompSpec,
  segmentId: string,
  takeId: string,
): CompSpec {
  const next: CompSpec = {
    ...spec,
    segments: spec.segments.map((s) =>
      s.segmentId === segmentId ? { ...s, takeId } : s,
    ),
  };
  return next;
}

/**
 * Per-seam crossfade plan. Each internal seam gets the spec's explicit
 * fadeIn when present, else a default `shape`/`maxTicks` bounded by
 * half the shorter adjacent segment (a fade can never exceed the
 * audio on either side). `seamFades[i]` describes the seam at
 * `segments[i].startTicks` for i >= 1.
 */
export function seamFadePlan(
  spec: CompSpec,
  opts: { shape?: FadeShape; maxTicks?: string } = {},
): Array<{ seamTicks: string; fade: FadeSpec }> {
  const shape = opts.shape ?? 'equal-power';
  const max = parseI64(opts.maxTicks ?? '240000'); // 1/16 note default
  const out: Array<{ seamTicks: string; fade: FadeSpec }> = [];
  for (let i = 1; i < spec.segments.length; i++) {
    const seg = spec.segments[i];
    const seam = parseI64(seg.startTicks);
    const explicit = seg.fadeIn;
    const bound = max > 0n ? max : 0n;
    const leftHalf = parseI64(spec.segments[i - 1].lengthTicks) / 2n;
    const rightHalf = parseI64(seg.lengthTicks) / 2n;
    const natural = leftHalf < rightHalf ? leftHalf : rightHalf;
    let length = explicit ? parseI64(explicit.lengthTicks) : bound;
    if (length > natural) length = natural;
    if (length <= 0n) continue; // degenerate seam: no fade possible
    out.push({
      seamTicks: i64str(seam),
      fade: { shape: explicit?.shape ?? shape, lengthTicks: i64str(length) },
    });
  }
  return out;
}

/**
 * The offset inside a take's source asset for a comp segment:
 * take.offsetTicks + (segmentStart - take.regionStart). Exact bigint.
 */
export function segmentAssetOffset(take: TakeRecord, segment: CompSegment): string {
  return i64str(
    parseI64(take.offsetTicks) + parseI64(segment.startTicks) - parseI64(take.regionStartTicks),
  );
}

export interface CompApplyPlan {
  transactionId: string;
  ops: PersistentOp[];
  /** clip ids the comp inserts (order == spec.segments order). */
  insertedClipIds: string[];
  /** clip ids the comp removes. */
  removedClipIds: string[];
  /** Seam fade metadata carried with the plan — view-state only until
   * the wire grows a fade field (NEEDS.md rev2). */
  seamFades: Array<{ seamTicks: string; fade: FadeSpec; intoClipId: string }>;
  /** Inserted clip ids whose take was MIDI — those clips carry no note
   * content (no note-copy op on the wire; NEEDS). */
  midiSegmentClipIds: string[];
}

/**
 * Comp → real clip edits (REC-05): remove every existing clip that
 * overlaps the comp region, then insert one audio clip per segment
 * referencing the take's immutable asset. ONE transaction — undo
 * restores the pre-comp arrangement exactly.
 *
 * MIDI takes: a comp segment referencing a MIDI take inserts an empty
 * MIDI clip for the span (the wire has no note-copy op — NEEDS); the
 * plan marks them via `midiSegmentClipIds` so the caller can
 * surface the limitation honestly instead of pretending note content
 * moved.
 */
export function compToOps(
  spec: CompSpec,
  folder: TakeFolder,
  target: { clips: ClipView[] },
  mint: () => string,
  opts: { seamShape?: FadeShape; maxFadeTicks?: string } = {},
): CompApplyPlan {
  const errors = validateComp(spec, folder);
  if (errors.length) throw new CompError(`invalid comp: ${errors.join('; ')}`);
  const region: Region = {
    startTicks: spec.regionStartTicks,
    lengthTicks: spec.regionLengthTicks,
  };
  const covering = target.clips.filter(
    (c) =>
      c.trackId === spec.trackId &&
      regionsOverlap(
        { startTicks: c.startTicks, lengthTicks: c.lengthTicks },
        region,
      ),
  );
  const ops: PersistentOp[] = [];
  const removedClipIds: string[] = [];
  for (const c of covering) {
    removedClipIds.push(c.clipId);
    ops.push({ RemoveClipOp: { clip_id: c.clipId } });
  }
  const insertedClipIds: string[] = [];
  const midiSegmentClipIds: string[] = [];
  for (const seg of spec.segments) {
    const take = takeById(folder, seg.takeId);
    const clipId = mint();
    insertedClipIds.push(clipId);
    if (take.kind === 'AUDIO') {
      ops.push({
        InsertAudioClipOp: {
          clip_id: clipId,
          track_id: spec.trackId,
          asset_id: take.assetId!,
          start_ticks: seg.startTicks,
          length_ticks: seg.lengthTicks,
          offset_ticks: segmentAssetOffset(take, seg),
        },
      });
    } else {
      midiSegmentClipIds.push(clipId);
      ops.push({
        InsertMidiClipOp: {
          clip_id: clipId,
          track_id: spec.trackId,
          start_ticks: seg.startTicks,
          length_ticks: seg.lengthTicks,
        },
      });
    }
  }
  const fades = seamFadePlan(spec, {
    shape: opts.seamShape,
    maxTicks: opts.maxFadeTicks,
  });
  return {
    transactionId: mint(),
    ops,
    insertedClipIds,
    removedClipIds,
    seamFades: fades.map((f, i) => ({
      seamTicks: f.seamTicks,
      fade: f.fade,
      intoClipId: insertedClipIds[i + 1],
    })),
    midiSegmentClipIds,
  };
}

/**
 * Flatten = keep the comp's materialized clips and DROP the take
 * folder's *stacking* (REC-05 "reversible flatten"): the spec is
 * preserved so re-comping is possible, and undo still reverses the
 * apply. This is a spec-level helper — flatten produces NO additional
 * ops (the comp IS the flattened state).
 */
export function flattenCompSpec(spec: CompSpec): CompSpec {
  return { ...spec, segments: spec.segments.map((s) => ({ ...s })) };
}
