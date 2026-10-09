//! void-spatial — W26 spatial routing and delivery model
//! (T92/T93 spec side).
//!
//! This crate is the honest *model* layer for spatial work:
//!
//! - [`layout`] — the mono → 22.2 layout ladder with real per-bus
//!   speaker assignments (ITU-R BS.2051 positions incl. the canonical
//!   22.2 system-H table) and the conversion [`layout::legality`]
//!   matrix: direct fits, declared-downmix folds, illegal upmixes.
//! - [`objects`] — object/bed container metadata (position, extent,
//!   gain, divergence) with bounded validation, mirrored on the ADM
//!   118-object bound.
//! - [`monitoring`] — declared speaker sets, per-speaker level
//!   calibration and declared fallbacks; `monitor_for` resolves
//!   direct placement / declared fold / typed unavailable.
//! - [`gate`] — [`gate::SpatialGate`]: every output path is
//!   `Available` **with its validator record** or `Unavailable` with
//!   a typed [`gate::SpatialUnavailable`] reason. Nothing answers
//!   "available" on faith, and stereo is never a proxy for a wider
//!   target.
//!
//! What this crate deliberately does NOT do: render audio, host a
//! licensed Atmos toolchain, or probe hardware. Those are engine/
//! validator/hardware concerns recorded in `docs/show/NEEDS.md` —
//! the gate names them [`gate::SpatialUnavailable`] rather than
//! faking them.

pub mod error;
pub mod gate;
pub mod layout;
pub mod monitoring;
pub mod objects;

pub use error::SpatialError;
pub use gate::{
    GateEnvironment, GateVerdict, SpatialGate, SpatialOutputPath, SpatialUnavailable,
    ValidatorKind, ValidatorRecord,
};
pub use layout::{legality, Legality, SpatialLayout, Speaker, SpeakerAssignment};
pub use monitoring::{
    can_drive, FallbackMode, MonitorUnavailable, MonitorVerdict, MonitoringConfig, MAX_TRIM_DB,
};
pub use objects::{
    ObjectContainerSpec, ObjectDescriptor, ObjectExtent, ObjectPosition, MAX_OBJECTS,
};
