//! void-producer — W21 "Accompaniment, arrangement and producer gate"
//! (non-native half).
//!
//! Modules:
//!   * `engine` + `music` + `rng` — deterministic accompaniment
//!     generators (drums/bass/keys/synth) with the locked-region
//!     invariant, inpaint/continue/vary modes (T78);
//!   * `service` — generation → `void_proposals` ProposalRecord glue
//!     (reuse of the existing lifecycle, not a fork);
//!   * `pcm` + `loudness` + `mastering` — real BS.1770 measurement
//!     (K-weighting, integrated LUFS, LRA, 4× true peak), gain/EQ
//!     suggestion records, A-B audition spec, explicit-accept
//!     lifecycle (T79);
//!   * `stems` — per-track/bus stem batch → `void_export` ExportSpecs;
//!   * `consolidate` — external-media collection planner over
//!     `void_assets`;
//!   * `alternatives` — project-alternative ledger (seal/branch/switch,
//!     asset protection set);
//!   * `import` — foreign-project → VOID import planner (id remap,
//!     loss report, no dangling sends).

pub mod alternatives;
pub mod consolidate;
pub mod engine;
pub mod error;
pub mod import;
pub mod loudness;
pub mod mastering;
pub mod music;
pub mod pcm;
pub mod rng;
pub mod service;
pub mod stems;

pub use alternatives::{open_or_create, save_ledger, Alternative, AlternativeLedger};
pub use consolidate::{
    execute_consolidation, plan_consolidation, ConsolidatedRef, ConsolidationPlan, ExternalRef,
    RefStatus,
};
pub use engine::{generate, GenMode, GeneratedCandidate, GenerationSpec, Role};
pub use error::{ProducerError, Result};
pub use import::{plan_import, ForeignProject, ImportLoss, ImportPlan, PlannedOp};
pub use loudness::{measure, LoudnessReport};
pub use mastering::{
    analyze, analyze_wav, draft, suggest_ops, AuditionSpec, MasteringOp, MasteringProposal,
    MasteringStatus, MasteringStore, MasteringTarget,
};
pub use music::{
    locked_region_bytes, ChordEvent, ChordQuality, GenNote, GrooveTemplate, Range, Scale,
    ScaleKind, TICKS_PER_QUARTER,
};
pub use pcm::{read_wav, write_wav_f32, PcmBuffer};
pub use rng::XorShift128;
pub use service::{
    accept_plan_checked, request_accompaniment, verify_locked_unchanged, AccompanimentRequest,
    GenerationSpecBase,
};
pub use stems::{
    plan_stem_batch, StemBatchPlan, StemBatchTemplate, StemKind, StemMember, StemSource,
};
