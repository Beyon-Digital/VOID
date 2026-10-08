//! Visual scene document state.
//!
//! Ownership follows CONTRACTS.md §5: the scene is the visual
//! participant's committed state — mutated only through serialized
//! `VisualCommand`s (expected_revision gate, per-transaction journal for
//! joint audio/visual undo, receipt discipline identical to the control
//! protocol). Rendering never mutates it; `resolve()` is a pure read.

use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};

use crate::error::{VisualError, VisualErrorCode};
use crate::ops::{VisualAckStatus, VisualCommand, VisualOp, VisualReceipt};
use crate::tempo::TempoMap;
use crate::types::*;

/// Known generator presets (validated built-ins — arbitrary shader code
/// is W23 scope, not this channel).
pub const GENERATOR_PRESETS: &[&str] =
    &["color-bars", "checker", "gradient", "plasma", "pulse", "black"];

/// Max persisted undo transactions (bounded journal).
pub const UNDO_JOURNAL_MAX: usize = 256;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum Inverse {
    InsertLayer { layer: Layer, index: i32 },
    DeleteLayer { layer_id: String },
    RestoreLayer { layer: Layer },
    SetOrder { channel: VisualChannel, order: Vec<String> },
    SetRoute { route: OutputRoute },
    SetTransition { state: TransitionState },
    /// Prior anchor value (None = anchor did not exist → inverse removes).
    SetAnchor { anchor_id: String, prior: Option<VisualAnchor> },
    RestoreAll { snapshot: Box<crate::checkpoint::VisualCheckpoint> },
}

#[derive(Debug, Clone)]
struct JournalEntry {
    transaction_id: String,
    inverses: Vec<Inverse>,
}

/// A fully-evaluated layer for one output tick (render input).
#[derive(Debug, Clone)]
pub struct ResolvedLayer {
    pub layer_id: String,
    pub kind: VisualLayerKind,
    pub name: String,
    pub transform: VisualTransform,
    pub blend: BlendMode,
    /// Combined opacity: transform.opacity × fade envelope.
    pub opacity: f32,
    pub media: Option<MediaRef>,
    pub generator: Option<GeneratorSpec>,
    /// Read-head position inside the source media, in ticks.
    pub media_ticks: i64,
}

/// What one channel must render at one tick.
#[derive(Debug, Clone)]
pub struct CompositionPlan {
    pub channel: VisualChannel,
    pub tick: i64,
    /// Bottom→top resolved layers of the channel's current stack.
    pub stack: Vec<ResolvedLayer>,
    /// While a take transition is in flight: the incoming stack
    /// (the other channel's composition being taken over).
    pub incoming: Option<Vec<ResolvedLayer>>,
    /// 0..1 mix toward `incoming` when in flight.
    pub transition_progress: f32,
    pub transition_kind: TransitionKind,
    pub wipe_angle: f32,
}

/// Events produced by `advance()` for telemetry/reporting.
#[derive(Debug, Clone, PartialEq)]
pub enum SceneEvent {
    TransitionFired { channel: VisualChannel, at_ticks: i64 },
    TransitionCompleted { channel: VisualChannel, at_ticks: i64 },
}

/// The visual scene: layers, anchors, transitions, routes — the
/// participant state that joins the project checkpoint.
#[derive(Debug)]
pub struct Scene {
    pub project_id: String,
    pub engine_epoch: u64,
    pub revision: u64,
    layers: BTreeMap<String, Layer>,
    /// Per-channel stack order, bottom→top.
    order: [Vec<String>; VisualChannel::COUNT],
    anchors: BTreeMap<String, VisualAnchor>,
    transitions: [TransitionState; VisualChannel::COUNT],
    routes: [OutputRoute; VisualChannel::COUNT],
    tempo_map: TempoMap,
    tempo_map_revision: u64,
    /// command_id → (payload_hash, stored receipt) for exactly-once.
    receipts: HashMap<String, (String, VisualReceipt)>,
    journal: Vec<JournalEntry>,
    redo: Vec<JournalEntry>,
    /// Checkpoint events pending pickup (SnapshotVisualStateOp results).
    pub pending_checkpoints: Vec<CheckpointPublish>,
}

/// Result of SnapshotVisualStateOp — the payload the coordinator embeds
/// in the checkpoint's app-state plus its integrity hash.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckpointPublish {
    pub command_id: String,
    pub checkpoint_id: String,
    pub revision: u64,
    pub state_sha256: String,
    pub snapshot: crate::checkpoint::VisualCheckpoint,
}

impl Scene {
    pub fn new(project_id: &str, engine_epoch: u64) -> Self {
        Self {
            project_id: project_id.to_string(),
            engine_epoch,
            revision: 0,
            layers: BTreeMap::new(),
            order: [Vec::new(), Vec::new()],
            anchors: BTreeMap::new(),
            transitions: [
                TransitionState::idle(VisualChannel::Preview),
                TransitionState::idle(VisualChannel::Program),
            ],
            routes: [
                OutputRoute::offscreen(VisualChannel::Preview, 640, 360),
                OutputRoute::offscreen(VisualChannel::Program, 640, 360),
            ],
            tempo_map: TempoMap::default(),
            tempo_map_revision: 0,
            receipts: HashMap::new(),
            journal: Vec::new(),
            redo: Vec::new(),
            pending_checkpoints: Vec::new(),
        }
    }

    pub fn tempo_map(&self) -> &TempoMap {
        &self.tempo_map
    }

    /// Engine-owned tempo map mirror (VisualClockSyncRequest — not a
    /// document edit, carries no revision gate).
    pub fn sync_tempo_map(&mut self, points: Vec<crate::tempo::TempoPoint>, sample_rate: u32, revision: u64) {
        self.tempo_map = TempoMap::new(points, sample_rate);
        self.tempo_map_revision = revision;
    }

    pub fn tempo_map_revision(&self) -> u64 {
        self.tempo_map_revision
    }

    // ------------------------------------------------------------------
    // Read-side queries (view projections)
    // ------------------------------------------------------------------

    /// All layers across channels (for GPU resource reconciliation).
    pub fn all_layers(&self) -> impl Iterator<Item = &Layer> {
        self.layers.values()
    }

    pub fn layer(&self, id: &str) -> Option<&Layer> {
        self.layers.get(id)
    }

    pub fn anchor(&self, id: &str) -> Option<&VisualAnchor> {
        self.anchors.get(id)
    }

    pub fn route(&self, channel: VisualChannel) -> &OutputRoute {
        &self.routes[channel.idx()]
    }

    pub fn transition(&self, channel: VisualChannel) -> &TransitionState {
        &self.transitions[channel.idx()]
    }

    /// Stack order (bottom→top layer ids) for a channel.
    pub fn stack_order(&self, channel: VisualChannel) -> &[String] {
        &self.order[channel.idx()]
    }

    /// Layers of a channel in stack order.
    pub fn stack(&self, channel: VisualChannel) -> Vec<&Layer> {
        self.order[channel.idx()]
            .iter()
            .filter_map(|id| self.layers.get(id))
            .collect()
    }

    // ------------------------------------------------------------------
    // Resolution — pure reads evaluated at a musical tick
    // ------------------------------------------------------------------

    /// Effective active window of a layer at `tick`: anchor bounds win
    /// over trim bounds.
    pub fn layer_window(&self, layer: &Layer) -> (i64, i64) {
        let in_t = match &layer.in_anchor {
            Some(aid) => self
                .anchors
                .get(aid)
                .map(|a| {
                    self.tempo_map
                        .anchor_ticks(a.kind, a.position_ticks, a.position_sample, a.timecode_ns)
                })
                .unwrap_or(layer.in_ticks),
            None => layer.in_ticks,
        };
        let out_t = match &layer.out_anchor {
            Some(aid) => self
                .anchors
                .get(aid)
                .map(|a| {
                    self.tempo_map
                        .anchor_ticks(a.kind, a.position_ticks, a.position_sample, a.timecode_ns)
                })
                .unwrap_or(layer.out_ticks),
            None => layer.out_ticks,
        };
        (in_t, out_t)
    }

    fn resolve_ids(&self, ids: &[String], tick: i64) -> Vec<ResolvedLayer> {
        let mut out = Vec::new();
        for id in ids {
            let Some(layer) = self.layers.get(id) else { continue };
            if !layer.visible {
                continue;
            }
            let (in_t, out_t) = self.layer_window(layer);
            if tick < in_t || tick >= out_t {
                continue;
            }
            // Fade envelope at the window edges.
            let mut opacity = layer.transform.opacity;
            if layer.fade_in_ticks > 0 {
                let p = (tick - in_t) as f64 / layer.fade_in_ticks as f64;
                opacity *= (p.clamp(0.0, 1.0)) as f32;
            }
            if layer.fade_out_ticks > 0 {
                let p = (out_t - tick) as f64 / layer.fade_out_ticks as f64;
                opacity *= (p.clamp(0.0, 1.0)) as f32;
            }
            if opacity <= 0.0 {
                continue;
            }
            out.push(ResolvedLayer {
                layer_id: layer.id.clone(),
                kind: layer.kind,
                name: layer.name.clone(),
                transform: layer.transform,
                blend: layer.blend,
                opacity,
                media: layer.media.clone(),
                generator: layer.generator.clone(),
                media_ticks: (tick - in_t).saturating_add(layer.offset_ticks),
            });
        }
        out
    }

    /// Resolved composition for a channel at `tick`.
    pub fn resolve(&self, channel: VisualChannel, tick: i64) -> CompositionPlan {
        let tr = &self.transitions[channel.idx()];
        let in_flight = tr.in_flight;
        let incoming = if in_flight {
            // During a take, "incoming" is the other channel's live stack.
            Some(self.resolve_ids(self.stack_order(channel.other()), tick))
        } else {
            None
        };
        let progress = if in_flight {
            match tr.kind {
                TransitionKind::Cut => 1.0,
                _ => {
                    if tr.duration_ticks <= 0 {
                        1.0
                    } else {
                        ((tick - tr.started_at_ticks) as f32 / tr.duration_ticks as f32)
                            .clamp(0.0, 1.0)
                    }
                }
            }
        } else {
            0.0
        };
        CompositionPlan {
            channel,
            tick,
            stack: self.resolve_ids(self.stack_order(channel), tick),
            incoming,
            transition_progress: progress,
            transition_kind: if in_flight { tr.kind } else { TransitionKind::Cut },
            wipe_angle: if in_flight { tr.wipe_angle } else { 0.0 },
        }
    }

    /// Advance transition state to `tick`: fire pending quantized takes at
    /// their boundary, complete in-flight transitions (program stack
    /// becomes the incoming stack).
    pub fn advance(&mut self, tick: i64) -> Vec<SceneEvent> {
        let mut events = Vec::new();
        for ch in [VisualChannel::Preview, VisualChannel::Program] {
            let i = ch.idx();
            let mut tr = self.transitions[i].clone();
            if !tr.armed && !tr.in_flight {
                continue;
            }
            // Pending take: resolve the quantized fire point against the
            // beat grid at the current clock tick.
            if tr.armed && tr.fires_at_ticks == -1 {
                tr.fires_at_ticks = match tr.quantize {
                    QuantizeMode::Immediate => tick,
                    QuantizeMode::NextBeat => self.tempo_map.next_beat_after(tick),
                    QuantizeMode::NextBar => self.tempo_map.next_bar_after(tick),
                };
            }
            // Armed + waiting for a quantized fire point.
            if tr.armed && tr.fires_at_ticks >= 0 && tick >= tr.fires_at_ticks {
                tr.started_at_ticks = tr.fires_at_ticks;
                tr.from_stack = self.order[i].clone();
                tr.armed = false;
                tr.in_flight = true;
                events.push(SceneEvent::TransitionFired {
                    channel: ch,
                    at_ticks: tr.fires_at_ticks,
                });
            }
            self.transitions[i] = tr.clone();
            // In flight → complete when the envelope ends.
            if tr.in_flight {
                let done = match tr.kind {
                    TransitionKind::Cut => true, // completes on the frame it fires
                    _ => tr.duration_ticks <= 0
                        || tick >= tr.started_at_ticks + tr.duration_ticks,
                };
                if done {
                    // Incoming stack ownership moves to this channel. The
                    // source channel gives up those layers entirely — a
                    // layer has exactly one owning channel.
                    let from_ids = self.order[ch.other().idx()].clone();
                    self.order[i] = from_ids.clone();
                    self.order[ch.other().idx()] = self.order[ch.other().idx()]
                        .iter()
                        .filter(|id| !from_ids.contains(*id))
                        .cloned()
                        .collect();
                    self.transitions[i] = TransitionState::idle(ch);
                    self.reindex();
                    events.push(SceneEvent::TransitionCompleted {
                        channel: ch,
                        at_ticks: tick,
                    });
                }
            }
        }
        events
    }

    fn reindex(&mut self) {
        for ch in [VisualChannel::Preview, VisualChannel::Program] {
            let ids = self.order[ch.idx()].clone();
            for (pos, id) in ids.iter().enumerate() {
                if let Some(l) = self.layers.get_mut(id) {
                    l.index = pos as i32;
                    l.channel = ch;
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // Command application (serialized mutation lane)
    // ------------------------------------------------------------------

    /// Apply one command under receipt discipline. Validation and the
    /// revision gate run before mutation; a REJECTED command never
    /// changes the scene.
    pub fn apply(&mut self, cmd: &VisualCommand) -> VisualReceipt {
        // Idempotency first: same id + same payload = DUPLICATE.
        let payload_hash = cmd.payload_hash();
        if let Some((stored_hash, stored)) = self.receipts.get(&cmd.command_id) {
            if *stored_hash == payload_hash {
                let mut r = stored.clone();
                r.status = VisualAckStatus::Duplicate;
                return r;
            }
            return VisualReceipt::rejected(
                cmd,
                VisualErrorCode::CommandIdReuse,
                self.revision,
                "command_id reused with a different payload",
            );
        }
        if cmd.expected_revision != self.revision {
            return VisualReceipt::rejected(
                cmd,
                VisualErrorCode::StaleRevision,
                self.revision,
                "expected_revision does not match current visual revision",
            );
        }
        if cmd.engine_epoch != self.engine_epoch {
            return VisualReceipt::rejected(
                cmd,
                VisualErrorCode::StaleEpoch,
                self.revision,
                "engine_epoch mismatch",
            );
        }
        if let Err(e) = self.validate(&cmd.op) {
            return VisualReceipt::rejected(cmd, e.code(), self.revision, &e.to_string());
        }

        let inverses = self.apply_op(&cmd.op);
        if !matches!(cmd.op, VisualOp::VisualRedoOp { .. })
            && !inverses.is_empty()
        {
            self.journal.push(JournalEntry {
                transaction_id: cmd.transaction_id.clone(),
                inverses,
            });
            if self.journal.len() > UNDO_JOURNAL_MAX {
                self.journal.remove(0);
            }
            self.redo.clear();
        }
        self.revision += 1;
        let receipt = VisualReceipt::applied(cmd, self.revision);
        self.receipts
            .insert(cmd.command_id.clone(), (payload_hash, receipt.clone()));
        receipt
    }

    /// Validate an op without mutating.
    fn validate(&self, op: &VisualOp) -> crate::error::Result<()> {
        match op {
            VisualOp::AddVisualLayerOp {
                layer_id,
                kind,
                index,
                generator,
                ..
            } => {
                require_id(layer_id, "layer_id")?;
                if self.layers.contains_key(layer_id) {
                    return Err(VisualError::BadRequest(format!(
                        "layer {layer_id} already exists"
                    )));
                }
                if *kind == VisualLayerKind::Generator {
                    let g = generator.as_ref().ok_or_else(|| {
                        VisualError::BadRequest("GENERATOR layer needs generator spec".into())
                    })?;
                    if !GENERATOR_PRESETS.contains(&g.preset.as_str()) {
                        return Err(VisualError::BadRequest(format!(
                            "unknown generator preset {}",
                            g.preset
                        )));
                    }
                    if g.param_json.len() > 16 * 1024 {
                        return Err(VisualError::BadRequest(
                            "generator params exceed 16 KiB".into(),
                        ));
                    }
                    serde_json::from_str::<serde_json::Value>(&g.param_json).map_err(|_| {
                        VisualError::BadRequest("generator param_json is not JSON".into())
                    })?;
                }
                if *kind != VisualLayerKind::Generator && generator.is_some() {
                    return Err(VisualError::BadRequest(
                        "generator spec only valid on GENERATOR layers".into(),
                    ));
                }
                let _ = index;
            }
            VisualOp::SetLayerStackOp { layer_ids, channel } => {
                for id in layer_ids {
                    let l = self.layers.get(id).ok_or_else(|| {
                        VisualError::NotFound(format!("layer {id}"))
                    })?;
                    if l.channel != *channel {
                        return Err(VisualError::BadRequest(format!(
                            "layer {id} is not on channel {channel:?}"
                        )));
                    }
                }
                let mut dedup = layer_ids.clone();
                dedup.sort();
                dedup.dedup();
                if dedup.len() != layer_ids.len() {
                    return Err(VisualError::BadRequest("duplicate layer id in stack".into()));
                }
            }
            VisualOp::AttachVisualMediaOp {
                layer_id,
                sha256,
                media_kind,
                rel_path,
                asset_id,
                duration_ticks,
            } => {
                let l = self.must_layer(layer_id)?;
                if l.kind == VisualLayerKind::Generator {
                    return Err(VisualError::BadRequest(
                        "generator layers cannot carry media".into(),
                    ));
                }
                require_id(asset_id, "asset_id")?;
                if sha256.len() != 64 || !sha256.chars().all(|c| c.is_ascii_hexdigit()) {
                    return Err(VisualError::BadRequest("sha256 must be 64 hex chars".into()));
                }
                if media_kind.is_empty() || media_kind.len() > 32 {
                    return Err(VisualError::BadRequest("bad media_kind".into()));
                }
                check_rel_path(rel_path)?;
                if *duration_ticks < 0 {
                    return Err(VisualError::BadRequest("negative duration".into()));
                }
            }
            VisualOp::SetLayerTrimOp {
                layer_id,
                in_ticks,
                out_ticks,
                ..
            } => {
                self.must_layer(layer_id)?;
                if *in_ticks >= *out_ticks && *out_ticks != i64::MAX {
                    return Err(VisualError::BadRequest("in_ticks >= out_ticks".into()));
                }
            }
            VisualOp::SetLayerFadeOp {
                layer_id,
                fade_in_ticks,
                fade_out_ticks,
            } => {
                self.must_layer(layer_id)?;
                if *fade_in_ticks < 0 || *fade_out_ticks < 0 {
                    return Err(VisualError::BadRequest("negative fade".into()));
                }
            }
            VisualOp::SetLayerTransformOp {
                layer_id,
                transform,
            } => {
                self.must_layer(layer_id)?;
                if !transform.validate() {
                    return Err(VisualError::BadRequest(
                        "transform: non-finite value or opacity outside 0..1".into(),
                    ));
                }
            }
            VisualOp::SetVisualAnchorOp {
                anchor_id,
                position_ticks,
                position_sample,
                timecode_ns,
                ..
            } => {
                require_id(anchor_id, "anchor_id")?;
                let _ = (position_ticks, position_sample, timecode_ns);
            }
            VisualOp::BindLayerAnchorOp {
                layer_id,
                in_anchor_id,
                out_anchor_id,
            } => {
                self.must_layer(layer_id)?;
                let in_t = if in_anchor_id.is_empty() {
                    None
                } else {
                    let a = self.anchors.get(in_anchor_id).ok_or_else(|| {
                        VisualError::NotFound(format!("anchor {in_anchor_id}"))
                    })?;
                    Some(self.tempo_map.anchor_ticks(
                        a.kind,
                        a.position_ticks,
                        a.position_sample,
                        a.timecode_ns,
                    ))
                };
                let out_t = if out_anchor_id.is_empty() {
                    None
                } else {
                    let a = self.anchors.get(out_anchor_id).ok_or_else(|| {
                        VisualError::NotFound(format!("anchor {out_anchor_id}"))
                    })?;
                    Some(self.tempo_map.anchor_ticks(
                        a.kind,
                        a.position_ticks,
                        a.position_sample,
                        a.timecode_ns,
                    ))
                };
                if let (Some(i), Some(o)) = (in_t, out_t) {
                    if i >= o {
                        return Err(VisualError::BadRequest(
                            "in_anchor resolves after out_anchor".into(),
                        ));
                    }
                }
            }
            VisualOp::SetTransitionOp {
                duration_ticks,
                wipe_angle,
                ..
            } => {
                if *duration_ticks < 0 || !wipe_angle.is_finite() {
                    return Err(VisualError::BadRequest(
                        "bad transition duration/angle".into(),
                    ));
                }
            }
            VisualOp::SetOutputRouteOp {
                width,
                height,
                fps_num,
                fps_den,
                ..
            } => {
                if *width == 0 || *height == 0 || *width > 16384 || *height > 16384 {
                    return Err(VisualError::BadRequest("output size out of range".into()));
                }
                if *fps_num == 0 || *fps_den == 0 {
                    return Err(VisualError::BadRequest("invalid frame rate".into()));
                }
            }
            VisualOp::RestoreVisualStateOp {
                state_sha256,
                snapshot_json,
                ..
            } => {
                if state_sha256.len() != 64 {
                    return Err(VisualError::BadRequest("state_sha256 must be 64 hex".into()));
                }
                if snapshot_json.len() > 8 * 1024 * 1024 {
                    return Err(VisualError::BadRequest("snapshot too large".into()));
                }
                use sha2::{Digest, Sha256};
                let hash = crate::ops::hex(&Sha256::digest(snapshot_json.as_bytes()));
                if hash != *state_sha256 {
                    return Err(VisualError::BadRequest(
                        "state_sha256 does not match snapshot payload".into(),
                    ));
                }
            }
            VisualOp::RemoveVisualLayerOp { layer_id }
            | VisualOp::SetLayerNameOp { layer_id, .. }
            | VisualOp::SetLayerVisibleOp { layer_id, .. }
            | VisualOp::SetLayerBlendOp { layer_id, .. } => {
                self.must_layer(layer_id)?;
            }
            VisualOp::RemoveVisualAnchorOp { anchor_id } => {
                if !self.anchors.contains_key(anchor_id) {
                    return Err(VisualError::NotFound(format!("anchor {anchor_id}")));
                }
            }
            VisualOp::TakeTransitionOp { .. }
            | VisualOp::CancelTransitionOp { .. }
            | VisualOp::SnapshotVisualStateOp { .. }
            | VisualOp::ClearVisualSceneOp {}
            | VisualOp::VisualUndoOp { .. }
            | VisualOp::VisualRedoOp { .. } => {}
        }
        Ok(())
    }

    fn must_layer(&self, id: &str) -> crate::error::Result<&Layer> {
        self.layers
            .get(id)
            .ok_or_else(|| VisualError::NotFound(format!("layer {id}")))
    }

    /// Apply a validated op; returns journal inverses.
    fn apply_op(&mut self, op: &VisualOp) -> Vec<Inverse> {
        match op {
            VisualOp::SetLayerStackOp { layer_ids, channel } => {
                let prior = self.order[channel.idx()].clone();
                // Layers leaving this channel's stack stay owned but are
                // unlisted; layers listed get indexed.
                self.order[channel.idx()] = layer_ids.clone();
                self.reindex();
                vec![Inverse::SetOrder {
                    channel: *channel,
                    order: prior,
                }]
            }
            VisualOp::AddVisualLayerOp {
                layer_id,
                kind,
                name,
                index,
                channel,
                generator,
            } => {
                let mut l = Layer::new(layer_id.clone(), *kind, name.clone(), *channel);
                l.generator = generator.clone();
                self.layers.insert(layer_id.clone(), l);
                let order = &mut self.order[channel.idx()];
                let idx = if *index < 0 || *index as usize > order.len() {
                    order.len()
                } else {
                    *index as usize
                };
                order.insert(idx, layer_id.clone());
                self.reindex();
                vec![Inverse::DeleteLayer {
                    layer_id: layer_id.clone(),
                }]
            }
            VisualOp::RemoveVisualLayerOp { layer_id } => {
                let Some(l) = self.layers.remove(layer_id) else {
                    return vec![];
                };
                for order in self.order.iter_mut() {
                    order.retain(|id| id != layer_id);
                }
                self.reindex();
                vec![Inverse::InsertLayer { layer: l, index: 0 }]
            }
            VisualOp::SetLayerNameOp { layer_id, name } => {
                let prior = self.layers.get(layer_id).cloned();
                if let Some(l) = self.layers.get_mut(layer_id) {
                    l.name = name.clone();
                }
                prior.map(|l| Inverse::RestoreLayer { layer: l }).into_iter().collect()
            }
            VisualOp::SetLayerVisibleOp { layer_id, visible } => {
                let prior = self.layers.get(layer_id).cloned();
                if let Some(l) = self.layers.get_mut(layer_id) {
                    l.visible = *visible;
                }
                prior.map(|l| Inverse::RestoreLayer { layer: l }).into_iter().collect()
            }
            VisualOp::SetLayerBlendOp {
                layer_id,
                blend_mode,
            } => {
                let prior = self.layers.get(layer_id).cloned();
                if let Some(l) = self.layers.get_mut(layer_id) {
                    l.blend = *blend_mode;
                }
                prior.map(|l| Inverse::RestoreLayer { layer: l }).into_iter().collect()
            }
            VisualOp::SetLayerTransformOp {
                layer_id,
                transform,
            } => {
                let prior = self.layers.get(layer_id).cloned();
                if let Some(l) = self.layers.get_mut(layer_id) {
                    l.transform = *transform;
                }
                prior.map(|l| Inverse::RestoreLayer { layer: l }).into_iter().collect()
            }
            VisualOp::AttachVisualMediaOp {
                layer_id,
                asset_id,
                sha256,
                media_kind,
                rel_path,
                duration_ticks,
            } => {
                let prior = self.layers.get(layer_id).cloned();
                if let Some(l) = self.layers.get_mut(layer_id) {
                    l.media = Some(MediaRef {
                        asset_id: asset_id.clone(),
                        sha256: sha256.clone(),
                        media_kind: media_kind.clone(),
                        rel_path: rel_path.clone(),
                        duration_ticks: *duration_ticks,
                    });
                }
                prior.map(|l| Inverse::RestoreLayer { layer: l }).into_iter().collect()
            }
            VisualOp::SetLayerTrimOp {
                layer_id,
                in_ticks,
                out_ticks,
                offset_ticks,
            } => {
                let prior = self.layers.get(layer_id).cloned();
                if let Some(l) = self.layers.get_mut(layer_id) {
                    l.in_ticks = *in_ticks;
                    l.out_ticks = *out_ticks;
                    l.offset_ticks = *offset_ticks;
                }
                prior.map(|l| Inverse::RestoreLayer { layer: l }).into_iter().collect()
            }
            VisualOp::SetLayerFadeOp {
                layer_id,
                fade_in_ticks,
                fade_out_ticks,
            } => {
                let prior = self.layers.get(layer_id).cloned();
                if let Some(l) = self.layers.get_mut(layer_id) {
                    l.fade_in_ticks = *fade_in_ticks;
                    l.fade_out_ticks = *fade_out_ticks;
                }
                prior.map(|l| Inverse::RestoreLayer { layer: l }).into_iter().collect()
            }
            VisualOp::SetVisualAnchorOp {
                anchor_id,
                kind,
                position_ticks,
                position_sample,
                timecode_ns,
            } => {
                let prior = self.anchors.get(anchor_id).cloned();
                self.anchors.insert(
                    anchor_id.clone(),
                    VisualAnchor {
                        id: anchor_id.clone(),
                        kind: *kind,
                        position_ticks: *position_ticks,
                        position_sample: *position_sample,
                        timecode_ns: *timecode_ns,
                    },
                );
                vec![Inverse::SetAnchor {
                    anchor_id: anchor_id.clone(),
                    prior,
                }]
            }
            VisualOp::RemoveVisualAnchorOp { anchor_id } => {
                let prior = self.anchors.remove(anchor_id);
                // Unbind layers referencing it (back to trim bounds).
                for l in self.layers.values_mut() {
                    if l.in_anchor.as_deref() == Some(anchor_id) {
                        l.in_anchor = None;
                    }
                    if l.out_anchor.as_deref() == Some(anchor_id) {
                        l.out_anchor = None;
                    }
                }
                vec![Inverse::SetAnchor {
                    anchor_id: anchor_id.clone(),
                    prior,
                }]
            }
            VisualOp::BindLayerAnchorOp {
                layer_id,
                in_anchor_id,
                out_anchor_id,
            } => {
                let prior = self.layers.get(layer_id).cloned();
                if let Some(l) = self.layers.get_mut(layer_id) {
                    l.in_anchor = if in_anchor_id.is_empty() {
                        None
                    } else {
                        Some(in_anchor_id.clone())
                    };
                    l.out_anchor = if out_anchor_id.is_empty() {
                        None
                    } else {
                        Some(out_anchor_id.clone())
                    };
                }
                prior.map(|l| Inverse::RestoreLayer { layer: l }).into_iter().collect()
            }
            VisualOp::SetTransitionOp {
                channel,
                kind,
                duration_ticks,
                quantize,
                wipe_angle,
            } => {
                let i = channel.idx();
                let prior = self.transitions[i].clone();
                let mut tr = prior.clone();
                tr.armed = true;
                tr.kind = *kind;
                tr.duration_ticks = *duration_ticks;
                tr.quantize = *quantize;
                tr.wipe_angle = *wipe_angle;
                tr.fires_at_ticks = 0;
                tr.started_at_ticks = 0;
                tr.in_flight = false;
                tr.from_stack.clear();
                self.transitions[i] = tr;
                vec![Inverse::SetTransition { state: prior }]
            }
            VisualOp::CancelTransitionOp { channel } => {
                let prior = self.transitions[channel.idx()].clone();
                self.transitions[channel.idx()] = TransitionState::idle(*channel);
                vec![Inverse::SetTransition { state: prior }]
            }
            // A take marks the transition pending; `advance(tick)`
            // resolves the quantized fire point (or fires immediately).
            VisualOp::TakeTransitionOp { channel } => {
                let prior = self.transitions[channel.idx()].clone();
                let i = channel.idx();
                let tr = &mut self.transitions[i];
                if !tr.armed {
                    tr.kind = TransitionKind::Cut;
                    tr.duration_ticks = 0;
                    tr.quantize = QuantizeMode::Immediate;
                }
                tr.armed = true;
                tr.fires_at_ticks = -1; // pending — resolved in advance()
                tr.started_at_ticks = 0;
                tr.in_flight = false;
                vec![Inverse::SetTransition { state: prior }]
            }
            VisualOp::SetOutputRouteOp {
                channel,
                target,
                display_id,
                width,
                height,
                fps_num,
                fps_den,
                readback,
            } => {
                let prior = self.routes[channel.idx()].clone();
                self.routes[channel.idx()] = OutputRoute {
                    channel: *channel,
                    target: *target,
                    display_id: display_id.clone(),
                    width: *width,
                    height: *height,
                    fps_num: *fps_num,
                    fps_den: *fps_den,
                    readback: *readback,
                    enabled: true,
                };
                vec![Inverse::SetRoute { route: prior }]
            }
            VisualOp::SnapshotVisualStateOp { checkpoint_id } => {
                let snapshot = self.checkpoint();
                let payload = crate::checkpoint::to_canonical_json(&snapshot);
                use sha2::{Digest, Sha256};
                self.pending_checkpoints.push(CheckpointPublish {
                    command_id: String::new(), // stamped by apply caller
                    checkpoint_id: checkpoint_id.clone(),
                    revision: self.revision,
                    state_sha256: crate::ops::hex(&Sha256::digest(payload.as_bytes())),
                    snapshot,
                });
                vec![]
            }
            VisualOp::RestoreVisualStateOp { snapshot_json, .. } => {
                let prior = Box::new(self.checkpoint());
                if let Ok(cp) = crate::checkpoint::from_json(snapshot_json) {
                    self.restore(&cp);
                }
                vec![Inverse::RestoreAll { snapshot: prior }]
            }
            VisualOp::ClearVisualSceneOp {} => {
                let prior = Box::new(self.checkpoint());
                self.layers.clear();
                self.order = [Vec::new(), Vec::new()];
                self.anchors.clear();
                self.transitions = [
                    TransitionState::idle(VisualChannel::Preview),
                    TransitionState::idle(VisualChannel::Program),
                ];
                vec![Inverse::RestoreAll { snapshot: prior }]
            }
            VisualOp::VisualUndoOp { transaction_id } => {
                self.undo(transaction_id);
                vec![]
            }
            VisualOp::VisualRedoOp { transaction_id } => {
                self.redo_op(transaction_id);
                vec![]
            }
        }
    }

    // ------------------------------------------------------------------
    // Joint undo (T83): per-transaction inverse journal
    // ------------------------------------------------------------------

    fn apply_inverse(&mut self, inv: &Inverse) {
        match inv {
            Inverse::InsertLayer { layer, .. } => {
                self.layers.insert(layer.id.clone(), layer.clone());
                let i = layer.index.max(0) as usize;
                let order = &mut self.order[layer.channel.idx()];
                order.insert(i.min(order.len()), layer.id.clone());
            }
            Inverse::DeleteLayer { layer_id } => {
                self.layers.remove(layer_id);
                for order in self.order.iter_mut() {
                    order.retain(|id| id != layer_id);
                }
            }
            Inverse::RestoreLayer { layer } => {
                let ch = layer.channel;
                let exists = self.layers.contains_key(&layer.id);
                self.layers.insert(layer.id.clone(), layer.clone());
                if !exists && !self.order[ch.idx()].contains(&layer.id) {
                    let i = layer.index.max(0) as usize;
                    self.order[ch.idx()].insert(i.min(self.order[ch.idx()].len()), layer.id.clone());
                }
            }
            Inverse::SetOrder { channel, order } => {
                self.order[channel.idx()] = order.clone();
            }
            Inverse::SetRoute { route } => {
                self.routes[route.channel.idx()] = route.clone();
            }
            Inverse::SetTransition { state } => {
                self.transitions[state.channel.idx()] = state.clone();
            }
            Inverse::SetAnchor { anchor_id, prior } => match prior {
                Some(a) => {
                    self.anchors.insert(a.id.clone(), a.clone());
                }
                None => {
                    self.anchors.remove(anchor_id);
                }
            },
            Inverse::RestoreAll { snapshot } => {
                self.restore(snapshot);
            }
        }
        self.reindex();
    }

    /// Rewind the visual half of transaction `tx` ("" = latest).
    pub fn undo(&mut self, tx: &str) -> bool {
        let pos = if tx.is_empty() {
            self.journal.len().checked_sub(1)
        } else {
            self.journal.iter().rposition(|e| e.transaction_id == tx)
        };
        let Some(pos) = pos else { return false };
        let entry = self.journal.remove(pos);
        // Journal the redo entry before mutating.
        let mut redo_inverses = Vec::new();
        for inv in &entry.inverses {
            redo_inverses.push(self.inverse_of(inv));
        }
        for inv in entry.inverses.iter().rev() {
            self.apply_inverse(inv);
        }
        self.redo.push(JournalEntry {
            transaction_id: entry.transaction_id,
            inverses: redo_inverses,
        });
        true
    }

    fn redo_op(&mut self, tx: &str) -> bool {
        let pos = if tx.is_empty() {
            self.redo.len().checked_sub(1)
        } else {
            self.redo.iter().rposition(|e| e.transaction_id == tx)
        };
        let Some(pos) = pos else { return false };
        let entry = self.redo.remove(pos);
        for inv in entry.inverses.iter() {
            self.apply_inverse(inv);
        }
        true
    }

    /// Compute the inverse that would re-do an undone step (inverse of an
    /// inverse = the forward op's post-state, captured from the journal
    /// entry itself).
    fn inverse_of(&self, inv: &Inverse) -> Inverse {
        match inv {
            Inverse::InsertLayer { layer, .. } => Inverse::DeleteLayer {
                layer_id: layer.id.clone(),
            },
            Inverse::DeleteLayer { layer_id } => self
                .layers
                .get(layer_id)
                .map(|l| Inverse::InsertLayer {
                    layer: l.clone(),
                    index: l.index,
                })
                .unwrap_or_else(|| Inverse::DeleteLayer {
                    layer_id: layer_id.clone(),
                }),
            Inverse::RestoreLayer { layer } => self
                .layers
                .get(&layer.id)
                .map(|l| Inverse::RestoreLayer { layer: l.clone() })
                .unwrap_or_else(|| inv.clone()),
            Inverse::SetOrder { channel, .. } => Inverse::SetOrder {
                channel: *channel,
                order: self.order[channel.idx()].clone(),
            },
            Inverse::SetRoute { .. } => Inverse::SetRoute {
                route: self.routes[inv_channel(inv).idx()].clone(),
            },
            Inverse::SetTransition { state } => Inverse::SetTransition {
                state: self.transitions[state.channel.idx()].clone(),
            },
            Inverse::SetAnchor { anchor_id, .. } => Inverse::SetAnchor {
                anchor_id: anchor_id.clone(),
                prior: self.anchors.get(anchor_id).cloned(),
            },
            Inverse::RestoreAll { .. } => Inverse::RestoreAll {
                snapshot: Box::new(self.checkpoint()),
            },
        }
    }

    // ------------------------------------------------------------------
    // Checkpoint join
    // ------------------------------------------------------------------

    pub fn checkpoint(&self) -> crate::checkpoint::VisualCheckpoint {
        let mut layers: Vec<Layer> = Vec::new();
        for ch in [VisualChannel::Preview, VisualChannel::Program] {
            for id in &self.order[ch.idx()] {
                if let Some(l) = self.layers.get(id) {
                    layers.push(l.clone());
                }
            }
        }
        // Unlisted layers (not on any stack) still persist.
        for (id, l) in &self.layers {
            if !layers.iter().any(|x| &x.id == id) {
                layers.push(l.clone());
            }
        }
        crate::checkpoint::VisualCheckpoint {
            format: crate::checkpoint::CHECKPOINT_FORMAT.to_string(),
            version: 0,
            revision: self.revision,
            engine_epoch: self.engine_epoch,
            layers,
            anchors: self.anchors.values().cloned().collect(),
            transitions: self.transitions.to_vec(),
            routes: self.routes.to_vec(),
            tempo_map_revision: self.tempo_map_revision,
        }
    }

    pub fn restore(&mut self, cp: &crate::checkpoint::VisualCheckpoint) {
        self.layers.clear();
        self.order = [Vec::new(), Vec::new()];
        self.anchors.clear();
        for l in &cp.layers {
            self.layers.insert(l.id.clone(), l.clone());
        }
        for l in &cp.layers {
            let i = self.order[l.channel.idx()].len();
            let pos = if l.index >= 0 { (l.index as usize).min(i) } else { i };
            self.order[l.channel.idx()].insert(pos, l.id.clone());
        }
        for a in &cp.anchors {
            self.anchors.insert(a.id.clone(), a.clone());
        }
        self.transitions = [
            cp.transitions
                .get(0)
                .cloned()
                .unwrap_or_else(|| TransitionState::idle(VisualChannel::Preview)),
            cp.transitions
                .get(1)
                .cloned()
                .unwrap_or_else(|| TransitionState::idle(VisualChannel::Program)),
        ];
        self.routes = [
            cp.routes
                .iter()
                .find(|r| r.channel == VisualChannel::Preview)
                .cloned()
                .unwrap_or_else(|| OutputRoute::offscreen(VisualChannel::Preview, 640, 360)),
            cp.routes
                .iter()
                .find(|r| r.channel == VisualChannel::Program)
                .cloned()
                .unwrap_or_else(|| OutputRoute::offscreen(VisualChannel::Program, 640, 360)),
        ];
        self.tempo_map_revision = cp.tempo_map_revision;
        self.reindex();
        self.revision = cp.revision;
    }
}

fn inv_channel(inv: &Inverse) -> VisualChannel {
    match inv {
        Inverse::SetRoute { route } => route.channel,
        Inverse::SetTransition { state } => state.channel,
        _ => VisualChannel::Preview,
    }
}

fn require_id(id: &str, name: &str) -> crate::error::Result<()> {
    if id.is_empty() || id.len() > 64 {
        return Err(VisualError::BadRequest(format!("bad {name}")));
    }
    Ok(())
}

fn check_rel_path(p: &str) -> crate::error::Result<()> {
    if p.is_empty()
        || p.len() > 1024
        || p.starts_with('/')
        || p.starts_with('\\')
        || p.contains("..")
        || p.contains('\0')
        || p.contains(':')
    {
        return Err(VisualError::BadRequest(format!("unsafe rel_path {p:?}")));
    }
    Ok(())
}
