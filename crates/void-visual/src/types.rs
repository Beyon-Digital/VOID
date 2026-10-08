//! Core visual types — serde DTOs mirroring `protocol/visual/void_visual.fbs`
//! table-for-table. JSON forms carry int64/u64 as decimal strings at the
//! TypeScript boundary (packages/void-studio), native ints inside Rust.

use serde::{Deserialize, Serialize};

/// Musical time resolution (mirrors void-protocol).
pub const TICKS_PER_QUARTER: i64 = 960_000;

/// Visual channel: monitor preview vs audience-facing program output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VisualChannel {
    Preview,
    Program,
}

impl VisualChannel {
    pub const COUNT: usize = 2;
    pub fn idx(self) -> usize {
        match self {
            Self::Preview => 0,
            Self::Program => 1,
        }
    }
    pub fn other(self) -> Self {
        match self {
            Self::Preview => Self::Program,
            Self::Program => Self::Preview,
        }
    }
    /// fbs `VisualChannel` numeric value.
    pub fn wire(self) -> u8 {
        self.idx() as u8
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VisualLayerKind {
    Image,
    Video,
    Generator,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BlendMode {
    Normal,
    Add,
    Multiply,
    Screen,
}

impl BlendMode {
    pub const ALL: [BlendMode; 4] = [
        BlendMode::Normal,
        BlendMode::Add,
        BlendMode::Multiply,
        BlendMode::Screen,
    ];
}

/// Anchor kinds: beat-locked vs absolute-time (CONTRACTS.md §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AnchorKind {
    BeatTick,
    Sample,
    Timecode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TransitionKind {
    Cut,
    Fade,
    Wipe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum QuantizeMode {
    Immediate,
    NextBeat,
    NextBar,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OutputTarget {
    Offscreen,
    Window,
}

/// Layer transform: pixels + scale + rotation + master opacity.
/// All floats must be finite; validators reject NaN/Inf.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VisualTransform {
    pub x: f32,
    pub y: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub rotation_rad: f32,
    pub opacity: f32,
}

impl Default for VisualTransform {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            rotation_rad: 0.0,
            opacity: 1.0,
        }
    }
}

impl VisualTransform {
    pub fn validate(&self) -> bool {
        [
            self.x,
            self.y,
            self.scale_x,
            self.scale_y,
            self.rotation_rad,
            self.opacity,
        ]
        .iter()
        .all(|v| v.is_finite())
            && (0.0..=1.0).contains(&self.opacity)
    }
}

/// Built-in deterministic procedural preset (GENERATOR layer kind).
/// `params` is a bounded JSON object validated per preset id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GeneratorSpec {
    pub preset: String,
    pub seed: u64,
    pub param_json: String,
}

/// Immutable media reference — asset_id/sha256 address a verified blob
/// under `assets/sha256/`; `rel_path` is project-relative.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaRef {
    pub asset_id: String,
    pub sha256: String,
    pub media_kind: String,
    pub rel_path: String,
    /// Declared media length in ticks; 0 = still/looping source.
    pub duration_ticks: i64,
}

/// Timeline anchor — a named position a layer edge or cue can bind to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct VisualAnchor {
    pub id: String,
    pub kind: AnchorKind,
    pub position_ticks: i64,
    pub position_sample: i64,
    pub timecode_ns: i64,
}

/// Armed/running transition state on a channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransitionState {
    pub channel: VisualChannel,
    pub armed: bool,
    pub kind: TransitionKind,
    pub duration_ticks: i64,
    pub quantize: QuantizeMode,
    pub wipe_angle: f32,
    /// Tick the transition fires at (quantized) — -1 = fire point pending
    /// resolution at the next rendered tick.
    pub fires_at_ticks: i64,
    /// Tick the in-flight transition started.
    pub started_at_ticks: i64,
    /// True while the transition envelope is rendering.
    #[serde(default)]
    pub in_flight: bool,
    /// Stack being transitioned away from (kept for the merge pass).
    #[serde(default)]
    pub from_stack: Vec<String>,
}

impl TransitionState {
    pub fn idle(channel: VisualChannel) -> Self {
        Self {
            channel,
            armed: false,
            kind: TransitionKind::Cut,
            duration_ticks: 0,
            quantize: QuantizeMode::Immediate,
            wipe_angle: 0.0,
            fires_at_ticks: 0,
            started_at_ticks: 0,
            in_flight: false,
            from_stack: Vec::new(),
        }
    }
}

/// Output route for a channel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OutputRoute {
    pub channel: VisualChannel,
    pub target: OutputTarget,
    /// Opaque display/monitor token ("" = primary). Window target only.
    pub display_id: String,
    pub width: u32,
    pub height: u32,
    /// Rational frame rate (CONTRACTS.md §7), e.g. 30000/1001.
    pub fps_num: u32,
    pub fps_den: u32,
    /// Keep offscreen readback when the route is a window (default true).
    pub readback: bool,
    /// Route explicitly enabled; a disabled channel produces no frames.
    pub enabled: bool,
}

impl OutputRoute {
    pub fn offscreen(channel: VisualChannel, width: u32, height: u32) -> Self {
        Self {
            channel,
            target: OutputTarget::Offscreen,
            display_id: String::new(),
            width,
            height,
            fps_num: 60,
            fps_den: 1,
            readback: true,
            enabled: true,
        }
    }
}

/// One composited layer (document state — serialized into checkpoints).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: String,
    pub kind: VisualLayerKind,
    pub name: String,
    pub channel: VisualChannel,
    /// Position in the channel's stack (0 = bottom).
    pub index: i32,
    pub visible: bool,
    pub blend: BlendMode,
    pub transform: VisualTransform,
    pub media: Option<MediaRef>,
    pub generator: Option<GeneratorSpec>,
    /// Trim window in timeline ticks (fallback when unanchored).
    pub in_ticks: i64,
    /// Exclusive end. i64::MAX = unbounded (always active).
    pub out_ticks: i64,
    /// Media read-head offset in ticks.
    pub offset_ticks: i64,
    /// Linear opacity ramp lengths at the window edges.
    pub fade_in_ticks: i64,
    pub fade_out_ticks: i64,
    /// Optional anchor bindings overriding trim bounds.
    pub in_anchor: Option<String>,
    pub out_anchor: Option<String>,
}

impl Layer {
    pub fn new(id: String, kind: VisualLayerKind, name: String, channel: VisualChannel) -> Self {
        Self {
            id,
            kind,
            name,
            channel,
            index: -1,
            visible: true,
            blend: BlendMode::Normal,
            transform: VisualTransform::default(),
            media: None,
            generator: None,
            in_ticks: 0,
            out_ticks: i64::MAX,
            offset_ticks: 0,
            fade_in_ticks: 0,
            fade_out_ticks: 0,
            in_anchor: None,
            out_anchor: None,
        }
    }
}
