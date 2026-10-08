// Visual channel view model (W22, T81-T83).
//
// Everything here is a PROJECTION of the engine's visual state — the
// studio never clones the visual document and never holds pixel buffers
// (frame pixels live on the native side; the UI only carries identity,
// layout and status).

/** Preview = what the operator is cueing; Program = what is on air. */
export type VisualChannel = 'preview' | 'program';

export const VISUAL_CHANNELS: readonly VisualChannel[] = ['preview', 'program'];

export type VisualLayerKind = 'image' | 'video' | 'generator';
export type VisualBlendMode = 'normal' | 'add' | 'multiply' | 'screen';
export type VisualTransitionKind = 'cut' | 'fade' | 'wipe';
export type VisualQuantizeMode = 'immediate' | 'next_beat' | 'next_bar';
export type VisualAnchorKind = 'beat' | 'sample' | 'timecode';
export type VisualOutputTarget = 'offscreen' | 'window';

/** Layer projection — identity + layout/trim/fade view state. */
export interface VisualLayerView {
  layerId: string;
  kind: VisualLayerKind;
  name: string;
  channel: VisualChannel;
  index: number;
  visible: boolean;
  blend: VisualBlendMode;
  opacity: number;
  x: number;
  y: number;
  scaleX: number;
  scaleY: number;
  rotationRad: number;
  /** Trim window in ticks; -1 = unset; anchor ids when bound. */
  inTicks: number;
  outTicks: number;
  inAnchorId: string | null;
  outAnchorId: string | null;
  /** Attached media identity (asset id + sha256, never the bytes). */
  assetId: string | null;
  mediaSha256: string | null;
  generatorPreset: string | null;
}

export interface VisualAnchorView {
  anchorId: string;
  kind: VisualAnchorKind;
  positionTicks: number;
}

export interface VisualTransitionView {
  channel: VisualChannel;
  armed: boolean;
  inFlight: boolean;
  kind: VisualTransitionKind;
  durationTicks: number;
  quantize: VisualQuantizeMode;
  progress: number;
}

export interface VisualOutputView {
  channel: VisualChannel;
  target: VisualOutputTarget;
  displayId: string;
  width: number;
  height: number;
  /** Rational frame rate, e.g. 30000/1001. */
  fpsNum: number;
  fpsDen: number;
}

/** Frame metadata for telemetry display — NO pixel payload. */
export interface VisualFrameMeta {
  channel: VisualChannel;
  clockSequence: number;
  positionTicks: number;
  timelineSample: number;
  deviceSampleCounter: number;
  renderUs: number;
  frameSha256: string;
}

export interface VisualDropCounters {
  droppedClocks: number;
  droppedFrames: number;
  skippedRenders: number;
  producedFrames: number;
}

function asStr(v: unknown, d = ''): string {
  return typeof v === 'string' ? v : d;
}
function asNum(v: unknown, d = 0): number {
  return typeof v === 'number' && Number.isFinite(v) ? v : d;
}
function asBool(v: unknown, d = false): boolean {
  return typeof v === 'boolean' ? v : d;
}

const LAYER_KINDS = new Set(['image', 'video', 'generator']);
const BLENDS = new Set(['normal', 'add', 'multiply', 'screen']);
const CHANNELS = new Set(['preview', 'program']);
const TRANSITION_KINDS = new Set(['cut', 'fade', 'wipe']);
const QUANTIZE = new Set(['immediate', 'next_beat', 'next_bar']);
const ANCHOR_KINDS = new Set(['beat', 'sample', 'timecode']);
const TARGETS = new Set(['offscreen', 'window']);

/** Defensive parse of one layer entry from a VisualStateSnapshot JSON. */
export function layerFromSnapshot(raw: unknown): VisualLayerView | null {
  if (raw === null || typeof raw !== 'object') return null;
  const l = raw as Record<string, unknown>;
  const kind = asStr(l.kind).toLowerCase();
  const channel = asStr(l.channel).toLowerCase();
  if (!LAYER_KINDS.has(kind) || !CHANNELS.has(channel)) return null;
  const blend = asStr(l.blend).toLowerCase();
  const t = (l.transform ?? {}) as Record<string, unknown>;
  return {
    layerId: asStr(l.layer_id || l.id),
    kind: kind as VisualLayerKind,
    name: asStr(l.name, '(layer)'),
    channel: channel as VisualChannel,
    index: asNum(l.index),
    visible: asBool(l.visible, true),
    blend: (BLENDS.has(blend) ? blend : 'normal') as VisualBlendMode,
    opacity: asNum(t.opacity, 1),
    x: asNum(t.x),
    y: asNum(t.y),
    scaleX: asNum(t.scale_x, 1),
    scaleY: asNum(t.scale_y, 1),
    rotationRad: asNum(t.rotation_rad),
    inTicks: asNum(l.in_ticks, -1),
    outTicks: asNum(l.out_ticks, -1),
    inAnchorId: typeof l.in_anchor_id === 'string' && l.in_anchor_id ? l.in_anchor_id : null,
    outAnchorId: typeof l.out_anchor_id === 'string' && l.out_anchor_id ? l.out_anchor_id : null,
    assetId: typeof l.asset_id === 'string' ? l.asset_id : null,
    mediaSha256: typeof l.sha256 === 'string' ? l.sha256 : null,
    generatorPreset: typeof l.generator_preset === 'string' ? l.generator_preset : null,
  };
}

export function anchorFromSnapshot(raw: unknown): VisualAnchorView | null {
  if (raw === null || typeof raw !== 'object') return null;
  const a = raw as Record<string, unknown>;
  const kind = asStr(a.kind).toLowerCase();
  if (!ANCHOR_KINDS.has(kind)) return null;
  return {
    anchorId: asStr(a.anchor_id || a.id),
    kind: kind as VisualAnchorKind,
    positionTicks: asNum(a.position_ticks),
  };
}

export function transitionFromSnapshot(
  channel: VisualChannel,
  raw: unknown,
): VisualTransitionView | null {
  if (raw === null || typeof raw !== 'object') return null;
  const t = raw as Record<string, unknown>;
  const kind = asStr(t.kind).toLowerCase();
  const quantize = asStr(t.quantize).toLowerCase();
  return {
    channel,
    armed: asBool(t.armed),
    inFlight: asBool(t.in_flight),
    kind: (TRANSITION_KINDS.has(kind) ? kind : 'cut') as VisualTransitionKind,
    durationTicks: asNum(t.duration_ticks),
    quantize: (QUANTIZE.has(quantize) ? quantize : 'immediate') as VisualQuantizeMode,
    progress: asNum(t.progress),
  };
}

export function routeFromSnapshot(
  channel: VisualChannel,
  raw: unknown,
): VisualOutputView | null {
  if (raw === null || typeof raw !== 'object') return null;
  const r = raw as Record<string, unknown>;
  const target = asStr(r.target).toLowerCase();
  return {
    channel,
    target: (TARGETS.has(target) ? target : 'offscreen') as VisualOutputTarget,
    displayId: asStr(r.display_id),
    width: asNum(r.width, 640),
    height: asNum(r.height, 360),
    fpsNum: asNum(r.fps_num, 60),
    fpsDen: asNum(r.fps_den, 1),
  };
}
