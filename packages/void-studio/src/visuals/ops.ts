// Visual op payload builders — mirrors `protocol/visual/void_visual.fbs`
// (draft rev0) wire shapes. Every builder validates BEFORE constructing
// the payload so a bad gesture never reaches the wire; i64/u64 values
// stay decimal-string safe on the JSON side per CONTRACTS.md §1.

import type {
  VisualAnchorKind,
  VisualBlendMode,
  VisualChannel,
  VisualLayerKind,
  VisualOutputTarget,
  VisualQuantizeMode,
  VisualTransitionKind,
} from './model';

/** Tagged-union op payload — same variant names as the fbs. */
export type VisualOpPayload =
  | { SetLayerStackOp: { layer_ids: string[]; channel: string } }
  | { AddVisualLayerOp: { layer_id: string; kind: string; name: string; index: number; channel: string; generator?: { preset: string; seed: number; param_json: string } } }
  | { RemoveVisualLayerOp: { layer_id: string } }
  | { SetLayerNameOp: { layer_id: string; name: string } }
  | { SetLayerVisibleOp: { layer_id: string; visible: boolean } }
  | { SetLayerBlendOp: { layer_id: string; blend_mode: string } }
  | { SetLayerTransformOp: { layer_id: string; transform: VisualTransformDto } }
  | { AttachVisualMediaOp: { layer_id: string; asset_id: string; sha256: string; media_kind: string; rel_path: string; duration_ticks: string } }
  | { SetLayerTrimOp: { layer_id: string; in_ticks: string; out_ticks: string; offset_ticks: string } }
  | { SetLayerFadeOp: { layer_id: string; fade_in_ticks: string; fade_out_ticks: string } }
  | { SetVisualAnchorOp: { anchor_id: string; kind: string; position_ticks: string; position_sample: string; timecode_ns: string } }
  | { RemoveVisualAnchorOp: { anchor_id: string } }
  | { BindLayerAnchorOp: { layer_id: string; in_anchor_id: string; out_anchor_id: string } }
  | { SetTransitionOp: { channel: string; kind: string; duration_ticks: string; quantize: string; wipe_angle: number } }
  | { TakeTransitionOp: { channel: string } }
  | { CancelTransitionOp: { channel: string } }
  | { SetOutputRouteOp: { channel: string; target: string; display_id: string; width: number; height: number; fps_num: number; fps_den: number; readback: boolean } }
  | { SnapshotVisualStateOp: { checkpoint_id: string } }
  | { RestoreVisualStateOp: { checkpoint_id: string; state_sha256: string; snapshot_json: string } }
  | { ClearVisualSceneOp: Record<string, never> }
  | { VisualUndoOp: { transaction_id: string } }
  | { VisualRedoOp: { transaction_id: string } };

export interface VisualTransformDto {
  x: number;
  y: number;
  scale_x: number;
  scale_y: number;
  rotation_rad: number;
  opacity: number;
}

function requireId(what: string, id: string): string {
  if (!id || id.length > 64 || /[\0\n\r]/.test(id)) {
    throw new Error(`${what}: invalid id`);
  }
  return id;
}

function requireFinite(what: string, v: number): number {
  if (!Number.isFinite(v)) throw new Error(`${what}: must be finite, got ${v}`);
  return v;
}

/** i64 fields travel as decimal strings on the JSON side. */
function i64(v: number | bigint): string {
  return typeof v === 'bigint' ? v.toString() : Math.trunc(v).toString();
}

export function setLayerStackOp(layerIds: string[], channel: VisualChannel): VisualOpPayload {
  return { SetLayerStackOp: { layer_ids: layerIds.map((id) => requireId('layer_id', id)), channel } };
}

export function addVisualLayerOp(
  layerId: string,
  kind: VisualLayerKind,
  name: string,
  index: number,
  channel: VisualChannel,
  generator?: { preset: string; seed: number; paramJson: string },
): VisualOpPayload {
  return {
    AddVisualLayerOp: {
      layer_id: requireId('layer_id', layerId),
      kind,
      name,
      index: Math.trunc(index),
      channel,
      ...(generator
        ? { generator: { preset: generator.preset, seed: Math.trunc(generator.seed), param_json: generator.paramJson } }
        : {}),
    },
  };
}

export function removeVisualLayerOp(layerId: string): VisualOpPayload {
  return { RemoveVisualLayerOp: { layer_id: requireId('layer_id', layerId) } };
}

export function setLayerNameOp(layerId: string, name: string): VisualOpPayload {
  return { SetLayerNameOp: { layer_id: requireId('layer_id', layerId), name } };
}

export function setLayerVisibleOp(layerId: string, visible: boolean): VisualOpPayload {
  return { SetLayerVisibleOp: { layer_id: requireId('layer_id', layerId), visible } };
}

export function setLayerBlendOp(layerId: string, blend: VisualBlendMode): VisualOpPayload {
  return { SetLayerBlendOp: { layer_id: requireId('layer_id', layerId), blend_mode: blend } };
}

export function setLayerTransformOp(layerId: string, t: VisualTransformDto): VisualOpPayload {
  for (const [k, v] of Object.entries(t)) {
    requireFinite(`transform.${k}`, v as number);
  }
  if (t.opacity < 0 || t.opacity > 1) {
    throw new Error(`transform.opacity must be in [0,1], got ${t.opacity}`);
  }
  return { SetLayerTransformOp: { layer_id: requireId('layer_id', layerId), transform: { ...t } } };
}

const SHA256_RE = /^[0-9a-f]{64}$/i;

/** Media attach: asset is identified immutably by sha256 — never a path blob. */
export function attachVisualMediaOp(
  layerId: string,
  assetId: string,
  sha256: string,
  mediaKind: 'image' | 'video',
  relPath: string,
  durationTicks: number | bigint,
): VisualOpPayload {
  if (!SHA256_RE.test(sha256)) {
    throw new Error('sha256 must be 64 hex chars');
  }
  if (!relPath || relPath.includes('..') || relPath.startsWith('/')) {
    throw new Error(`rel_path must be a safe relative path, got "${relPath}"`);
  }
  return {
    AttachVisualMediaOp: {
      layer_id: requireId('layer_id', layerId),
      asset_id: requireId('asset_id', assetId),
      sha256: sha256.toLowerCase(),
      media_kind: mediaKind,
      rel_path: relPath,
      duration_ticks: i64(durationTicks),
    },
  };
}

export function setLayerTrimOp(
  layerId: string,
  inTicks: number | bigint,
  outTicks: number | bigint,
  offsetTicks: number | bigint,
): VisualOpPayload {
  return {
    SetLayerTrimOp: {
      layer_id: requireId('layer_id', layerId),
      in_ticks: i64(inTicks),
      out_ticks: i64(outTicks),
      offset_ticks: i64(offsetTicks),
    },
  };
}

export function setLayerFadeOp(
  layerId: string,
  fadeInTicks: number | bigint,
  fadeOutTicks: number | bigint,
): VisualOpPayload {
  return {
    SetLayerFadeOp: {
      layer_id: requireId('layer_id', layerId),
      fade_in_ticks: i64(fadeInTicks),
      fade_out_ticks: i64(fadeOutTicks),
    },
  };
}

export function setVisualAnchorOp(
  anchorId: string,
  kind: VisualAnchorKind,
  positionTicks: number | bigint,
): VisualOpPayload {
  return {
    SetVisualAnchorOp: {
      anchor_id: requireId('anchor_id', anchorId),
      kind,
      position_ticks: i64(positionTicks),
      position_sample: '-1',
      timecode_ns: '-1',
    },
  };
}

export function removeVisualAnchorOp(anchorId: string): VisualOpPayload {
  return { RemoveVisualAnchorOp: { anchor_id: requireId('anchor_id', anchorId) } };
}

/** Bind a layer's trim window to anchors; '' unbinds that side. */
export function bindLayerAnchorOp(
  layerId: string,
  inAnchorId: string,
  outAnchorId: string,
): VisualOpPayload {
  if (inAnchorId) requireId('in_anchor_id', inAnchorId);
  if (outAnchorId) requireId('out_anchor_id', outAnchorId);
  return {
    BindLayerAnchorOp: {
      layer_id: requireId('layer_id', layerId),
      in_anchor_id: inAnchorId,
      out_anchor_id: outAnchorId,
    },
  };
}

export function setTransitionOp(
  channel: VisualChannel,
  kind: VisualTransitionKind,
  durationTicks: number | bigint,
  quantize: VisualQuantizeMode,
  wipeAngle: number,
): VisualOpPayload {
  requireFinite('wipe_angle', wipeAngle);
  return {
    SetTransitionOp: {
      channel,
      kind,
      duration_ticks: i64(durationTicks),
      quantize,
      wipe_angle: wipeAngle,
    },
  };
}

export function takeTransitionOp(channel: VisualChannel): VisualOpPayload {
  return { TakeTransitionOp: { channel } };
}

export function cancelTransitionOp(channel: VisualChannel): VisualOpPayload {
  return { CancelTransitionOp: { channel } };
}

/** Rational fps output route — e.g. fpsNum=30000, fpsDen=1001 (29.97). */
export function setOutputRouteOp(
  channel: VisualChannel,
  target: VisualOutputTarget,
  displayId: string,
  width: number,
  height: number,
  fpsNum: number,
  fpsDen: number,
  readback: boolean,
): VisualOpPayload {
  if (width <= 0 || height <= 0 || width > 16384 || height > 16384) {
    throw new Error(`output dims must be in 1..16384, got ${width}x${height}`);
  }
  if (fpsNum <= 0 || fpsDen <= 0) {
    throw new Error('fps must be a positive rational');
  }
  return {
    SetOutputRouteOp: {
      channel,
      target,
      display_id: displayId,
      width: Math.trunc(width),
      height: Math.trunc(height),
      fps_num: Math.trunc(fpsNum),
      fps_den: Math.trunc(fpsDen),
      readback,
    },
  };
}

export function snapshotVisualStateOp(checkpointId: string): VisualOpPayload {
  return { SnapshotVisualStateOp: { checkpoint_id: requireId('checkpoint_id', checkpointId) } };
}

export function restoreVisualStateOp(
  checkpointId: string,
  stateSha256: string,
  snapshotJson: string,
): VisualOpPayload {
  if (!SHA256_RE.test(stateSha256)) {
    throw new Error('state_sha256 must be 64 hex chars');
  }
  return {
    RestoreVisualStateOp: {
      checkpoint_id: requireId('checkpoint_id', checkpointId),
      state_sha256: stateSha256.toLowerCase(),
      snapshot_json: snapshotJson,
    },
  };
}

export function clearVisualSceneOp(): VisualOpPayload {
  return { ClearVisualSceneOp: {} };
}

/** Joint undo: transaction_id groups ops committed under one gesture. */
export function visualUndoOp(transactionId: string): VisualOpPayload {
  return { VisualUndoOp: { transaction_id: transactionId } };
}

export function visualRedoOp(transactionId: string): VisualOpPayload {
  return { VisualRedoOp: { transaction_id: transactionId } };
}
