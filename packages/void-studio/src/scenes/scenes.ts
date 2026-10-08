// Scene slots + quantized launch semantics (W17/PAT-02, PAT-03,
// PAT-04; session view / live performance).
//
// Honest split per the spec: scheduling stays NATIVE — the engine
// owns sample-accurate launch/stop at quantize boundaries (there is
// no wire op for scene launch; NEEDS rev2). What lives here is the
// view model (track × scene slot grid), the launch-state machine the
// UI renders (pending→launched→playing→stopping→stopped), and the
// command mapping onto the transport ops that DO exist (SEEK,
// SET_CYCLE, SET_TEMPO_LIVE) so a scene-set change is expressible
// today as "seek+cycle to the scene's range".

import { i64str, parseI64 } from 'void-client';
import type { Region } from '../takes/takes';
import type { MeterEvent } from '../arrangement/tempo';
import { TICKS_PER_QUARTER } from '../viewport';

/** A scene: a named row of the launch grid, optionally bound to an
 * arrangement range (its stop-all/marker semantics). */
export interface Scene {
  sceneId: string;
  name: string;
  color?: string;
  /** Bound arrangement range — the region the scene's clips play
   * under. Optional: scenes can be slot-only (clips armed per cell). */
  region?: Region;
}

/** One cell: a clip (or armed record target) slotted at
 * track × scene. `content` is view-state only — a reference to the
 * clip/pattern the cell would launch. */
export interface SceneSlot {
  slotId: string;
  sceneId: string;
  trackId: string;
  /** What the slot launches. */
  content:
    | { kind: 'clip'; clipId: string }
    | { kind: 'pattern'; patternId: string }
    | { kind: 'empty' };
  /** Legato/quantize override for this slot. */
  quantize?: LaunchQuantize;
}

export type LaunchQuantize =
  | 'immediate'
  | 'bar'
  | 'beat'
  | { kind: 'custom'; ticks: string };

/** Grid = scenes (rows) × tracks (columns) → slots. */
export interface SceneGrid {
  scenes: Scene[];
  trackIds: string[];
  slots: Record<string, SceneSlot>; // `${sceneId}:${trackId}`
}

export function slotKey(sceneId: string, trackId: string): string {
  return `${sceneId}:${trackId}`;
}

export function makeGrid(scenes: Scene[], trackIds: string[]): SceneGrid {
  return { scenes, trackIds, slots: {} };
}

export function setSlot(grid: SceneGrid, slot: SceneSlot): SceneGrid {
  return { ...grid, slots: { ...grid.slots, [slotKey(slot.sceneId, slot.trackId)]: slot } };
}

export function sceneSlots(grid: SceneGrid, sceneId: string): SceneSlot[] {
  return grid.trackIds
    .map((t) => grid.slots[slotKey(sceneId, t)])
    .filter((s): s is SceneSlot => s !== undefined);
}

/** Non-empty slots of a scene — what a launch would fire. */
export function launchSet(grid: SceneGrid, sceneId: string): SceneSlot[] {
  return sceneSlots(grid, sceneId).filter((s) => s.content.kind !== 'empty');
}

// ---------------------------------------------------------------------------
// Launch-state machine (view model — engine scheduling is NEEDS rev2)
// ---------------------------------------------------------------------------

export type SlotLaunchState =
  | { phase: 'stopped' }
  | { phase: 'pending'; atTicks: string }
  | { phase: 'playing'; sinceTicks: string }
  | { phase: 'stopping'; atTicks: string };

export interface LaunchState {
  /** Per slot-id launch phase. */
  slots: Record<string, SlotLaunchState>;
  /** The currently-playing scene (exclusive scene launch, PAT-03). */
  playingSceneId: string | null;
}

export function makeLaunchState(): LaunchState {
  return { slots: {}, playingSceneId: null };
}

/**
 * Compute the tick at which a quantized launch lands. Pure —
 * `nowTicks` + quantize + meter map → the next boundary at-or-after
 * now. This is the SEMANTIC target the engine is asked to hit; the
 * sample-accurate firing is native work (NEEDS).
 */
export function quantizeLaunchAt(
  nowTicks: string,
  quantize: LaunchQuantize,
  meters: MeterEvent[] = [{ atTicks: '0', numerator: 4, denominator: 4 }],
): string {
  const now = parseI64(nowTicks);
  if (quantize === 'immediate') return i64str(now);
  const span =
    quantize === 'beat'
      ? TICKS_PER_QUARTER
      : quantize === 'bar'
        ? barTicks(now, meters)
        : parseI64(quantize.ticks);
  if (span <= 0n) return i64str(now);
  const rem = now % span;
  return i64str(rem === 0n ? now : now - rem + span);
}

function barTicks(atTicks: bigint, meters: MeterEvent[]): bigint {
  const sorted = [...meters].sort(
    (a, b) => Number(parseI64(a.atTicks) - parseI64(b.atTicks)),
  );
  let meter = sorted[0] ?? { atTicks: '0', numerator: 4, denominator: 4 };
  for (const m of sorted) if (parseI64(m.atTicks) <= atTicks) meter = m;
  return TICKS_PER_QUARTER * 4n * BigInt(meter.numerator) / BigInt(meter.denominator);
}

/** Launch a scene: compute per-slot states at the requested
 * quantization. Exclusive: the previously playing scene's slots go
 * 'stopping' at the same boundary (PAT-03 scene switch). */
export function launchScene(
  grid: SceneGrid,
  state: LaunchState,
  sceneId: string,
  nowTicks: string,
  quantize: LaunchQuantize,
  meters?: MeterEvent[],
): { state: LaunchState; atTicks: string } {
  const atTicks = quantizeLaunchAt(nowTicks, quantize, meters);
  const slots = { ...state.slots };
  for (const s of launchSet(grid, sceneId)) {
    const q = s.quantize ?? quantize;
    const at = q === quantize ? atTicks : quantizeLaunchAt(nowTicks, q, meters);
    slots[s.slotId] = { phase: 'pending', atTicks: at };
  }
  if (state.playingSceneId !== null && state.playingSceneId !== sceneId) {
    for (const s of launchSet(grid, state.playingSceneId)) {
      const cur = slots[s.slotId];
      if (cur === undefined || cur.phase === 'playing' || cur.phase === 'pending') {
        slots[s.slotId] = { phase: 'stopping', atTicks };
      }
    }
  }
  return { state: { slots, playingSceneId: sceneId }, atTicks };
}

/** Mark pending/stopping slots resolved at a boundary tick (the
 * engine's launch event feeds this). */
export function settleLaunch(state: LaunchState, atTicks: string): LaunchState {
  const slots = { ...state.slots };
  for (const [id, s] of Object.entries(slots)) {
    if (s.phase === 'pending' && parseI64(s.atTicks) <= parseI64(atTicks)) {
      slots[id] = { phase: 'playing', sinceTicks: s.atTicks };
    } else if (s.phase === 'stopping' && parseI64(s.atTicks) <= parseI64(atTicks)) {
      slots[id] = { phase: 'stopped' };
    }
  }
  return { ...state, slots };
}

/** Stop-all: every playing slot goes 'stopping' at the quantize
 * boundary; the scene row clears. */
export function stopAll(
  state: LaunchState,
  nowTicks: string,
  quantize: LaunchQuantize,
  meters?: MeterEvent[],
): { state: LaunchState; atTicks: string } {
  const atTicks = quantizeLaunchAt(nowTicks, quantize, meters);
  const slots = { ...state.slots };
  for (const [id, s] of Object.entries(slots)) {
    if (s.phase === 'playing' || s.phase === 'pending') {
      slots[id] = { phase: 'stopping', atTicks };
    }
  }
  return { state: { slots, playingSceneId: null }, atTicks };
}

// ---------------------------------------------------------------------------
// Command mapping: scene semantics → ops that exist today
// ---------------------------------------------------------------------------

/**
 * Map "launch this scene (bound to a range) now" onto transport ops:
 * SEEK to the scene start + SET_CYCLE over the scene range when the
 * scene loops. This is the honest wire surface for scene playback
 * until scene ops land (NEEDS rev2). Pure mapping — the caller sends
 * via the transport channel.
 */
export type SceneTransportCommand =
  | { op: 'SEEK'; position_ticks: string }
  | { op: 'SET_CYCLE'; cycle_start_ticks: string; cycle_end_ticks: string }
  | { op: 'PLAY' }
  | { op: 'STOP' };

export function sceneTransportOps(scene: Scene): SceneTransportCommand[] {
  if (scene.region === undefined) return [];
  const end = parseI64(scene.region.startTicks) + parseI64(scene.region.lengthTicks);
  return [
    { op: 'SEEK', position_ticks: scene.region.startTicks },
    {
      op: 'SET_CYCLE',
      cycle_start_ticks: scene.region.startTicks,
      cycle_end_ticks: i64str(end),
    },
  ];
}


