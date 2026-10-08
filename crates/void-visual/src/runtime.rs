//! Visual runtime: owns the scene + renderer on ONE dedicated thread.
//! Callers enqueue ops on a bounded channel and push clock snapshots
//! into a single-slot mailbox — neither can block the audio path.
//! Frame production is driven by the latest ClockSnapshot; if the GPU
//! falls behind, pending frames are DROPPED (counted) — audio never
//! waits on visuals.

use std::sync::mpsc;
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use crate::clock::{ClockSnapshot, ClockTracker};
use crate::error::{Result, VisualError};
use crate::media::FfmpegPuller;
use crate::ops::{VisualCommand, VisualReceipt};
use crate::render::{AdapterInfo, ProducedFrame, Renderer};
use crate::scene::Scene;
use crate::slot::Slot;
use crate::types::*;

/// Bounded op queue depth (CONTRACTS.md: pending mutations ≤128 → BUSY).
pub const OP_QUEUE_DEPTH: usize = 128;

/// Live counters — audio-facing proof that drops happen on the visual
/// side and never propagate back.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct DropCounters {
    /// Clock snapshots overwritten before consumption.
    pub dropped_clocks: u64,
    /// Produced frames displaced before the consumer read them.
    pub dropped_frames: u64,
    /// Renders skipped because a newer clock was already waiting.
    pub skipped_renders: u64,
    pub produced_frames: u64,
}

/// What the runtime emits per produced frame.
#[derive(Debug, Clone)]
pub struct FrameEvent {
    pub frame: ProducedFrame,
    pub counters: DropCounters,
}

/// Alert surfaced to telemetry (decode failure, adapter loss, etc).
#[derive(Debug, Clone)]
pub enum VisualAlert {
    DecodeFailed { layer_id: String, detail: String },
    RenderFailed { detail: String },
    DeviceLost { detail: String },
}

enum EngineMsg {
    Command(VisualCommand),
    Shutdown,
}

/// Handle the audio/control side holds. All calls are non-blocking.
pub struct VisualRuntime {
    tx: mpsc::SyncSender<EngineMsg>,
    clock: Arc<Slot<ClockSnapshot>>,
    frame_out: Arc<Slot<FrameEvent>>,
    receipts: mpsc::Receiver<VisualReceipt>,
    alerts: mpsc::Receiver<VisualAlert>,
    counters: Arc<std::sync::Mutex<DropCounters>>,
    join: Option<JoinHandle<()>>,
    pub adapter_info: AdapterInfo,
}

impl VisualRuntime {
    /// Spawn a headless runtime; `width`/`height` size the initial
    /// offscreen targets (routes can resize via SetOutputRoute).
    pub fn spawn(
        project_id: &str,
        engine_epoch: u64,
        width: u32,
        height: u32,
        assets_root: std::path::PathBuf,
        ffmpeg_bin: String,
    ) -> Result<Self> {
        let renderer = Renderer::new_headless(width, height)?;
        let adapter_info = renderer.adapter_info().clone();
        let (tx, rx) = mpsc::sync_channel::<EngineMsg>(OP_QUEUE_DEPTH);
        let (receipt_tx, receipt_rx) = mpsc::channel();
        let (alert_tx, alert_rx) = mpsc::channel();
        let clock = Slot::new();
        let frame_out = Slot::new();
        let counters = Arc::new(std::sync::Mutex::new(DropCounters::default()));
        let engine = Engine {
            scene: Scene::new(project_id, engine_epoch),
            renderer,
            clock_tracker: ClockTracker::new(engine_epoch),
            rx,
            clock: clock.clone(),
            frame_out: frame_out.clone(),
            receipt_tx,
            alert_tx,
            counters: counters.clone(),
            assets_root,
            ffmpeg_bin,
            video_pullers: std::collections::HashMap::new(),
        };
        let join = thread::Builder::new()
            .name("void-visual".into())
            .spawn(move || engine.run())
            .map_err(|e| VisualError::DeviceUnavailable(e.to_string()))?;
        Ok(Self {
            tx,
            clock,
            frame_out,
            receipts: receipt_rx,
            alerts: alert_rx,
            counters,
            join: Some(join),
            adapter_info,
        })
    }

    /// Enqueue a command. Non-blocking — full queue returns Busy per the
    /// pending-mutation cap; the receipt arrives on `take_receipt()`.
    pub fn send_command(&self, cmd: VisualCommand) -> Result<()> {
        self.tx.try_send(EngineMsg::Command(cmd)).map_err(|e| match e {
            mpsc::TrySendError::Full(_) => VisualError::Busy,
            mpsc::TrySendError::Disconnected(_) => {
                VisualError::DeviceUnavailable("visual engine stopped".into())
            }
        })
    }

    /// Latest clock snapshot. Overwrites any unconsumed snapshot — the
    /// overwrite IS the drop (audio side is never delayed).
    pub fn push_clock(&self, snap: ClockSnapshot) {
        if self.clock.send(snap).is_some() {
            self.counters.lock().unwrap().dropped_clocks += 1;
        }
    }

    /// Take the latest produced frame; older frames were dropped and
    /// counted engine-side.
    pub fn take_frame(&self) -> Option<FrameEvent> {
        self.frame_out.recv()
    }

    pub fn take_receipt(&self) -> Option<VisualReceipt> {
        self.receipts.try_recv().ok()
    }

    pub fn take_alert(&self) -> Option<VisualAlert> {
        self.alerts.try_recv().ok()
    }

    pub fn counters(&self) -> DropCounters {
        *self.counters.lock().unwrap()
    }

    /// Read one composed frame from a channel's offscreen target is NOT
    /// exposed here — rendered pixels ride on `FrameEvent` when
    /// `OutputRoute.readback` is set. This keeps the API non-blocking.
    pub fn shutdown(&mut self) {
        let _ = self.tx.send(EngineMsg::Shutdown);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}

impl Drop for VisualRuntime {
    fn drop(&mut self) {
        self.shutdown();
    }
}

struct Engine {
    scene: Scene,
    renderer: Renderer,
    clock_tracker: ClockTracker,
    rx: mpsc::Receiver<EngineMsg>,
    clock: Arc<Slot<ClockSnapshot>>,
    frame_out: Arc<Slot<FrameEvent>>,
    receipt_tx: mpsc::Sender<VisualReceipt>,
    alert_tx: mpsc::Sender<VisualAlert>,
    counters: Arc<std::sync::Mutex<DropCounters>>,
    assets_root: std::path::PathBuf,
    ffmpeg_bin: String,
    video_pullers: std::collections::HashMap<String, FfmpegPuller>,
}

impl Engine {
    fn run(mut self) {
        loop {
            // Block for work; drain everything queued before rendering.
            match self.rx.recv() {
                Ok(EngineMsg::Shutdown) | Err(_) => return,
                Ok(EngineMsg::Command(cmd)) => {
                    let receipt = self.scene.apply(&cmd);
                    let _ = self.receipt_tx.send(receipt);
                    self.after_scene_change();
                }
            }
            while let Ok(msg) = self.rx.try_recv() {
                match msg {
                    EngineMsg::Shutdown => return,
                    EngineMsg::Command(cmd) => {
                        let receipt = self.scene.apply(&cmd);
                        let _ = self.receipt_tx.send(receipt);
                        self.after_scene_change();
                    }
                }
            }
            let Some(clock) = self.clock.recv() else {
                continue;
            };
            if self.clock_tracker.push(clock.clone()).is_err() {
                continue;
            }
            let tick = self.scene.tempo_map().tick_at_sample(clock.timeline_sample);
            for event in self.scene.advance(tick) {
                match event {
                    crate::scene::SceneEvent::TransitionFired { channel, .. } => {
                        tracing::debug!(?channel, "transition fired");
                    }
                    crate::scene::SceneEvent::TransitionCompleted { channel, .. } => {
                        tracing::debug!(?channel, "transition completed");
                    }
                }
            }
            for ch in [VisualChannel::Preview, VisualChannel::Program] {
                let route = self.scene.route(ch).clone();
                if !route.enabled || route.target != OutputTarget::Offscreen {
                    continue;
                }
                // A newer clock already waiting means this render would be
                // late — skip it (counted; audio side unaffected).
                if !self.clock.peek_is_empty() {
                    self.counters.lock().unwrap().skipped_renders += 1;
                    break;
                }
                self.renderer.ensure_target(ch.idx(), route.width, route.height);
                self.pump_video(tick);
                let plan = self.scene.resolve(ch, tick);
                let tpq = TICKS_PER_QUARTER as i64;
                let beat_phase = (((tick % tpq) + tpq) % tpq) as f32 / tpq as f32;
                match self.renderer.render(
                    &plan,
                    clock.timeline_sample,
                    clock.device_sample_counter,
                    clock.host_clock_ns,
                    clock.sequence,
                    beat_phase,
                    route.readback,
                ) {
                    Ok(frame) => {
                        let counters = {
                            let mut c = self.counters.lock().unwrap();
                            c.produced_frames += 1;
                            *c
                        };
                        if self
                            .frame_out
                            .send(FrameEvent { frame, counters })
                            .is_some()
                        {
                            self.counters.lock().unwrap().dropped_frames += 1;
                        }
                    }
                    Err(e) => {
                        let _ = self.alert_tx.send(VisualAlert::RenderFailed {
                            detail: e.to_string(),
                        });
                    }
                }
            }
        }
    }

    /// Reconcile GPU-side resources after any scene mutation: upload
    /// textures for attached media, spawn/kill video pullers, drop
    /// textures for removed/replaced media.
    fn after_scene_change(&mut self) {
        for layer in self.scene.all_layers() {
            let Some(media) = &layer.media else { continue };
            let key = crate::render::texture_key(&layer.id, &media.sha256);
            match layer.kind {
                VisualLayerKind::Image => {
                    if self.renderer.has_texture(&key) {
                        continue;
                    }
                    match crate::media::verify_asset(&self.assets_root, media)
                        .and_then(|p| crate::media::decode_image(&p))
                    {
                        Ok(frame) => self.renderer.set_layer_texture(&key, &frame),
                        Err(e) => {
                            let _ = self.alert_tx.send(VisualAlert::DecodeFailed {
                                layer_id: layer.id.clone(),
                                detail: e.to_string(),
                            });
                        }
                    }
                }
                VisualLayerKind::Video => {
                    if self.video_pullers.contains_key(&layer.id) {
                        continue;
                    }
                    let spawn_result = crate::media::verify_asset(&self.assets_root, media)
                        .and_then(|path| {
                            let route = self.scene.route(VisualChannel::Program);
                            FfmpegPuller::spawn(
                                &path,
                                0.0,
                                route.width.min(1920),
                                route.height.min(1080),
                                route.fps_num.max(1),
                                route.fps_den.max(1),
                                &self.ffmpeg_bin,
                            )
                        });
                    match spawn_result {
                        Ok(p) => {
                            self.video_pullers.insert(layer.id.clone(), p);
                        }
                        Err(e) => {
                            let _ = self.alert_tx.send(VisualAlert::DecodeFailed {
                                layer_id: layer.id.clone(),
                                detail: e.to_string(),
                            });
                        }
                    }
                }
                VisualLayerKind::Generator => {}
            }
        }
    }

    /// Feed video layers' decoded frames into textures at the position
    /// derived from the musical tick (beat-anchored, not wall time).
    fn pump_video(&mut self, tick: i64) {
        let tempo = self.scene.tempo_map().clone();
        let mut dead: Vec<String> = Vec::new();
        for (layer_id, puller) in self.video_pullers.iter_mut() {
            // Tick → source seconds via the tempo map (120 bpm = 0.5s/qtr
            // constant tempo; tempo-aware indexing converts tick→beat→sec).
            let quarters = tick as f64 / TICKS_PER_QUARTER as f64;
            let bpm = tempo.tempo_at(tick).bpm;
            let secs = quarters * 60.0 / bpm.max(1.0);
            let frame_idx =
                (secs * puller.fps_num as f64 / puller.fps_den.max(1) as f64).floor() as u64;
            if let Some(f) = puller.frame_at(frame_idx) {
                if let Some(layer) = self.scene.layer(layer_id) {
                    if let Some(m) = &layer.media {
                        let key = crate::render::texture_key(layer_id, &m.sha256);
                        self.renderer.set_layer_texture(&key, &f);
                    }
                }
            }
            if let Some(fail) = puller.is_failed() {
                let _ = self.alert_tx.send(VisualAlert::DecodeFailed {
                    layer_id: layer_id.clone(),
                    detail: fail.clone(),
                });
                dead.push(layer_id.clone());
            }
        }
        for id in dead {
            if let Some(mut p) = self.video_pullers.remove(&id) {
                p.kill();
            }
        }
    }
}
