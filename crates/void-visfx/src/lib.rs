//! void-visfx — W23 non-native halves: declarative visual-generation
//! job adapter (→ void-jobs `visual_generation`), naga-validated WGSL
//! shader preset registry with budget/watchdog/fallback, deterministic
//! audio-feature mappers, and the camera-conducting policy model.
//!
//! Renderer, GPU and camera hardware are out of scope on this box —
//! the honest gaps live in docs/visfx/.

pub mod analysis;
pub mod camera;
pub mod error;
pub mod mappers;
pub mod record;
pub mod service;
pub mod shader;
pub mod spec;
pub mod store;

pub use camera::{
    CameraAsserts, CameraPolicy, CalibrationRecord, ConsentState, ConductorSession, ControlEvent,
    DropReason, GestureKind, ReleaseCause, TrackerFrame,
};
pub use error::{PolicyViolation, Result, VisFxError};
pub use mappers::{analyze, AnalyzerSet, FeatureFrames, FeatureKind, MapperKind, ParamMapper};
pub use record::{ArtifactBrief, VisGenRecord, VisGenStatus};
pub use service::{
    PendingVis, Revalidation, VisAcceptPlan, VisualGenService, MAX_PENDING_VIS,
    SYMBOLIC_RUNTIME_ID,
};
pub use shader::{
    CompileReport, FallbackPolicy, KernelOutcome, PresetRecord, PresetRef, PresetStatus,
    RejectReason, ResolvedShader, ResourceBudget, ShaderChain, ShaderRegistry, ValidationStage,
    WatchdogPolicy, DEFAULT_FALLBACK,
};
pub use spec::{
    parse_scene_doc, AudioBinding, DocAction, DocGenerator, DocLayer, DocMedia, DocTransform,
    FeatureSource, SceneDoc, SceneGenKind, SceneGenSpec, ALLOWED_ACTIONS, DOC_FILE, DOC_TAG,
    FORBIDDEN_FIELDS, SPEC_TAG,
};
pub use store::{utc_now, VisGenStore};
