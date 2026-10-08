//! void-visual — VOID's native visual composition engine (W22).
//!
//! A real wgpu renderer that runs as a separate output path: it owns a
//! dedicated thread, receives commands on a bounded channel and clock
//! snapshots through a latest-wins slot, renders preview and program
//! channel stacks to offscreen targets, and drops/degrades visual work
//! under load rather than ever delaying the audio clock.
//!
//! Evidence path: `OutputRoute { target: Offscreen, readback: true }`
//! renders real pixels and returns them + sha256 — headless-verifiable
//! on llvmpipe/lavapipe or any GPU.

pub mod checkpoint;
pub mod clock;
pub mod error;
pub mod media;
pub mod ops;
pub mod render;
pub mod runtime;
pub mod scene;
pub mod shaders;
pub mod slot;
pub mod tempo;
pub mod types;

pub use checkpoint::VisualCheckpoint;
pub use clock::{ClockAccept, ClockReject, ClockSnapshot, ClockTracker};
pub use error::{Result, VisualError, VisualErrorCode};
pub use media::{FfmpegPuller, RgbaFrame};
pub use ops::{VisualAckStatus, VisualCommand, VisualOp, VisualReceipt};
pub use render::{AdapterInfo, ProducedFrame, Renderer};
pub use runtime::{DropCounters, FrameEvent, VisualAlert, VisualRuntime, OP_QUEUE_DEPTH};
pub use scene::{CompositionPlan, ResolvedLayer, Scene, SceneEvent};
pub use types::*;

/// Contract revision this crate implements (`protocol/visual/void_visual.fbs`).
pub const PROTOCOL_REVISION: u32 = 0;
pub const PROTOCOL_NAMESPACE: &str = "voidvis";
