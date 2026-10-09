// S09 Visuals — pure view logic (no React, no DOM).
//
// Everything here derives honest render state from the visuals store
// projections + studio telemetry. The interaction contract
// (docs/ui-handoff/VOID_Design_System_and_Screen_Map.json
// `interactionContracts.visualOutput`) fixes the output state names:
// previewOnly | displaySelected | armed | live | blackout | unavailable.

import {
  TICKS_PER_QUARTER,
  type VisualChannel,
  type VisualDropCounters,
  type VisualFrameMeta,
  type VisualLayerView,
  type VisualOutputTarget,
  type VisualsViewState,
} from 'void-studio';
import type { ClockSnapshot, ReadItem } from 'void-client';
import { parseAssetItem, type AssetRow } from 'void-studio';

// ---------------------------------------------------------------------------
// Channel seams — honest capabilities of THIS build.
//
// void-visual speaks a separate `voidvis` wire (protocol/visual rev0 draft):
// it is not in the shipped PersistentOp union and no Tauri invoke bridges it
// (commands.rs exposes only engine_status/spawn_engine/stop_engine/
// send_command/send_transport/read_view). Same for the visual snapshot
// feed, the display enumerator, job submission, and the ffmpeg probe. These
// constants exist so every gated control can name the missing seam.
// ---------------------------------------------------------------------------

/** True once a webview→voidvis command path exists (send_visual_command). */
export const VISUAL_COMMAND_CHANNEL = false;
/** True once the engine feeds VisualStateSnapshot / frame meta to the UI. */
export const VISUAL_SNAPSHOT_FEED = false;
/** True once `submit_job` (void-jobs) is a shipped persistent op + invoke. */
export const JOB_SUBMIT_CHANNEL = false;
/** True once the shell can run `ffmpeg -encoders` and report to the UI. */
export const FFMPEG_PROBE = false;

export const REASON_NO_CHANNEL =
  'visual command channel not bridged — the shipped protocol has no voidvis op (integrator: add the draft wire)';
export const REASON_NO_SNAPSHOT =
  'no visual snapshot feed — layer state arrives when the engine read path lands';
export const REASON_NO_DISPLAY_ENUM =
  'display enumeration deferred — the winit surface path is not wired (docs/visual/README.md)';
export const REASON_NO_JOB_SUBMIT =
  'job submission unavailable — submit_job is not in the shipped protocol';
export const REASON_NO_FFMPEG_PROBE =
  'no ffmpeg probe channel — encoder detection cannot run from this build';
export const REASON_NO_INGEST =
  'asset ingest not in protocol — AttachAssetOp only registers blobs already inside container/assets (NEEDS.md §rev2)';

// ---------------------------------------------------------------------------
// Output state machine (previewOnly → displaySelected → armed → live;
// blackout is an independent cut; unavailable when the engine is detached).
// ---------------------------------------------------------------------------

export type VisualOutputUiState =
  | 'unavailable'
  | 'previewOnly'
  | 'displaySelected'
  | 'armed'
  | 'live'
  | 'blackout';

/** Operator intent — view state only; the native route op rides the seam. */
export interface VisualOutputIntent {
  selectedTarget: VisualOutputTarget | null;
  armed: boolean;
  blackout: boolean;
  /** The last seam note (e.g. route op could not be sent). */
  note: string | null;
}

export const INITIAL_OUTPUT_INTENT: VisualOutputIntent = {
  selectedTarget: null,
  armed: false,
  blackout: false,
  note: null,
};

export interface OutputView {
  state: VisualOutputUiState;
  /** Human-readable reason/verb — always shown with the state. */
  reason: string;
  /** True when program frames are currently arriving (armed + frames). */
  framesFlowing: boolean;
}

export function deriveOutputView(input: {
  attached: boolean;
  intent: VisualOutputIntent;
  lastProgramFrame: VisualFrameMeta | null;
}): OutputView {
  const { attached, intent, lastProgramFrame } = input;
  if (!attached) {
    return { state: 'unavailable', reason: 'engine detached — no program output', framesFlowing: false };
  }
  if (intent.blackout) {
    return { state: 'blackout', reason: 'blackout engaged — program output cut', framesFlowing: false };
  }
  if (intent.armed) {
    if (lastProgramFrame) {
      return { state: 'live', reason: 'program output live', framesFlowing: true };
    }
    return {
      state: 'armed',
      reason: VISUAL_SNAPSHOT_FEED
        ? 'armed — awaiting first program frame'
        : `armed — ${REASON_NO_SNAPSHOT}`,
      framesFlowing: false,
    };
  }
  if (intent.selectedTarget) {
    return { state: 'displaySelected', reason: 'target selected — arm to route output', framesFlowing: false };
  }
  return { state: 'previewOnly', reason: 'output disarmed — nothing is sent to a display', framesFlowing: false };
}

export function outputStateLabel(state: VisualOutputUiState): string {
  switch (state) {
    case 'unavailable':
      return 'UNAVAILABLE';
    case 'previewOnly':
      return 'OUTPUT DISARMED';
    case 'displaySelected':
      return 'TARGET SELECTED';
    case 'armed':
      return 'OUTPUT ARMED';
    case 'live':
      return 'OUTPUT LIVE';
    case 'blackout':
      return 'BLACKOUT';
  }
}

// ---------------------------------------------------------------------------
// Layers / channels
// ---------------------------------------------------------------------------

/** Layers of one channel in stack order (bottom → top). */
export function orderedChannelLayers(
  s: Pick<VisualsViewState, 'layers' | 'order'>,
  channel: VisualChannel,
): VisualLayerView[] {
  return s.order[channel].map((id) => s.layers[id]).filter((l): l is VisualLayerView => !!l);
}

/** Layer trim window in ticks; -1/unset falls back to [0, defaultEnd). */
export function layerWindowTicks(
  layer: VisualLayerView,
  defaultEndTicks: number,
): { startTicks: number; endTicks: number } {
  const start = layer.inTicks >= 0 ? layer.inTicks : 0;
  const end = layer.outTicks >= 0 ? layer.outTicks : defaultEndTicks;
  return { startTicks: start, endTicks: Math.max(end, start) };
}

/**
 * Timeline end (in ticks) covering every layer plus a floor of 4 bars.
 * Never a figma constant — derived from real layer windows.
 */
export function timelineSpanTicks(layers: VisualLayerView[]): number {
  const floor = Number(TICKS_PER_QUARTER) * 16; // 4 bars of 4/4
  let max = floor;
  for (const l of layers) {
    if (l.outTicks > max) max = l.outTicks;
    if (l.outTicks < 0 && l.inTicks >= 0 && l.inTicks > max) max = l.inTicks;
  }
  return max;
}

export function layerKindLabel(kind: VisualLayerView['kind']): string {
  switch (kind) {
    case 'generator':
      return 'PROCEDURAL';
    case 'image':
      return 'IMAGE';
    case 'video':
      return 'VIDEO';
  }
}

/** One-line frame identity — sha prefix + timing facts only (no pixels). */
export function frameMetaLine(frame: VisualFrameMeta): string {
  return `frame ${frame.frameSha256.slice(0, 12)}… · sample ${frame.timelineSample} · render ${frame.renderUs}µs · clock seq ${frame.clockSequence}`;
}

/** Drop-counter strip — real counters verbatim; zero reads as healthy. */
export function dropCountersLine(c: VisualDropCounters): string {
  return `frames produced ${c.producedFrames} · dropped ${c.droppedFrames} · clocks dropped ${c.droppedClocks} · skipped renders ${c.skippedRenders}`;
}

/** UI-T24 surfacing: frames the renderer dropped while armed. */
export function framesDroppedWhileArmed(c: VisualDropCounters): boolean {
  return c.droppedFrames > 0 || c.droppedClocks > 0 || c.skippedRenders > 0;
}

// ---------------------------------------------------------------------------
// Assets → visual asset rows
// ---------------------------------------------------------------------------

/** Media types that can feed a visual layer (defensive: image/*, video/*). */
export function mediaTypeIsVisual(mediaType: string | undefined): boolean {
  if (!mediaType) return false;
  const t = mediaType.toLowerCase();
  return t === 'image' || t === 'video' || t.startsWith('image/') || t.startsWith('video/');
}

export function mediaTypeIsAudio(mediaType: string | undefined): boolean {
  if (!mediaType) return false;
  const t = mediaType.toLowerCase();
  return t === 'audio' || t.startsWith('audio/');
}

/** ASSET_LIST items → visual asset rows (present + missing both listed). */
export function visualAssetRows(items: readonly ReadItem[]): AssetRow[] {
  const out: AssetRow[] = [];
  for (const it of items) {
    const r = parseAssetItem(it);
    if (r && mediaTypeIsVisual(r.mediaType)) out.push(r);
  }
  return out;
}

/** Real asset blobs usable as AV export inputs, split by stream role. */
export function assetBlobsByRole(items: readonly ReadItem[]): {
  video: AssetRow[];
  audio: AssetRow[];
} {
  const video: AssetRow[] = [];
  const audio: AssetRow[] = [];
  for (const it of items) {
    const r = parseAssetItem(it);
    if (!r || r.state !== 'present' || !r.expectedSha256) continue;
    if (mediaTypeIsVisual(r.mediaType)) video.push(r);
    else if (mediaTypeIsAudio(r.mediaType)) audio.push(r);
  }
  return { video, audio };
}

// ---------------------------------------------------------------------------
// Clock → visuals (native audio timing drives the schedule view)
// ---------------------------------------------------------------------------

/** Honest transport line — the same "pos N samples" the arrangement shows. */
export function clockLine(clock: ClockSnapshot | undefined): string {
  if (!clock) return 'no clock — engine telemetry not attached';
  return `audio clock seq ${clock.sequence} · pos ${clock.timeline_sample} samples @ ${clock.sample_rate} Hz · ${clock.transport.toLowerCase()}`;
}

/** Loop window in ticks is real clock data (engine sends it as ticks). */
export function loopWindowTicks(clock: ClockSnapshot | undefined): { start: number; end: number } | null {
  if (!clock) return null;
  const start = Number(clock.loop_start_ticks);
  const end = Number(clock.loop_end_ticks);
  if (!Number.isFinite(start) || !Number.isFinite(end) || end <= start) return null;
  return { start, end };
}
