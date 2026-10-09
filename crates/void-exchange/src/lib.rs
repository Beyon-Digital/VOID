//! void-exchange — W20 "Plugin compatibility, optional isolation and
//! exchange" spec-side implementation for VOID.
//!
//! Four subsystems, all real and all Linux-verifiable:
//!
//! - **Interchange** (`dawproject`, `midi`, `stems`): DAWproject subset
//!   import/export (ZIP+XML container), SMF read/write, stems bundle
//!   manifests. Every transfer emits a deterministic [`LossReport`].
//! - **Document** (`document`): the format-neutral `ExchangeDocument`
//!   the wire ops can express plus the richer fields foreign formats
//!   carry (fades, note channel/release, plugin state refs) — held
//!   honestly so round-trips and loss reports agree.
//! - **Compatibility registry** (`registry`): plugin descriptor records
//!   (format/arch/state-format-version), per-platform observation,
//!   quarantine, and typed compatibility verdicts. AAX is never hostable;
//!   unknown is never assumed compatible.
//! - **Missing-plugin state** (`preserve`) + **isolation policy**
//!   (`isolation`): opaque state blobs retained across save/reopen and
//!   rehydrated on reappearance; a validated policy model (bounded
//!   buffers / deadlines / restart budgets) a native plugin-worker host
//!   can enforce — plus typed `IsolationUnavailable` reasons for what
//!   this platform cannot do. No fake "isolated" flags.
//!
//! `plan` turns an imported document into ordered `void_control` wire ops
//! with the corresponding loss entries for wire gaps.

pub mod dawproject;
pub mod document;
pub mod error;
pub mod isolation;
pub mod loss;
pub mod midi;
pub mod plan;
pub mod preserve;
pub mod registry;
pub mod stems;

pub use document::{
    AssetRef, Clip, ClipContent, ExchangeDocument, LoopRange, Marker, Note, PluginParam,
    PluginParamValue, PluginSlot, PluginStateRef, TempoPoint, TimeSignaturePoint, Track, TrackKind,
    EXCHANGE_FORMAT, TICKS_PER_QUARTER,
};
pub use error::{ExchangeError, Result};
pub use isolation::{
    IsolationMode, IsolationPolicy, IsolationUnavailable, MissedDeadlineAction, WorkerDeathAction,
};
pub use loss::{ExchangeDirection, LossEntry, LossKind, LossReport};
pub use plan::{plan_import, ImportPlan, PlannedOp};
pub use preserve::{MissingPluginStore, PreservedPluginState, Resolution, StillMissing};
pub use registry::{
    CompatibilityReport, CompatibilityStatus, DeviceRole, PluginDescriptor, PluginFormat,
    PluginRegistry, RegistryEntry,
};
