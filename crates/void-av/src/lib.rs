//! `void-av` — audiovisual export pipeline (W24, T87–T89 non-native
//! half): FFmpeg argv-only encode, exact rational frame-rate math,
//! declarative codec matrix with rights flags, staged render → probe
//! verify → provenance → atomic publish on the void-jobs state machine.
//!
//! Patterned on `void-export`; the JobDb/checkpoint/atomic-publish
//! machinery is reused directly, and the renderer/probe subprocesses
//! obey the same argv-only, env_clear, kill-on-cancel contract.

pub mod codec;
pub mod error;
pub mod ffmpeg;
pub mod job;
pub mod layout;
pub mod probe;
pub mod provenance;
pub mod rate;
pub mod session;
pub mod spec;

pub use codec::{codec, require_encoders, CodecRights, CodecSpec, CODEC_MATRIX};
pub use error::{AvError, Result};
pub use ffmpeg::{
    codec_gate, AvRenderer, CodecGateReport, FfmpegRunner, RenderOutcome, ResolvedInput,
};
pub use job::{job_spec_for, AvRunner, AvTools};
pub use probe::{FfprobeRunner, MediaProbe, ProbeInfo};
pub use provenance::{AvArtifactRecord, AvProvenance, CueReport, NONDETERMINISM_NOTE};
pub use rate::{
    place_cue, rational_round, CuePlacement, FrameRate, FrameRounding, NTSC_24, NTSC_30,
};
pub use session::{AvSession, AvStep, AvVerifyReport};
pub use spec::{
    parse_u64, AvCue, AvExportSpec, AvFramePlan, AvInput, AvInputRole, AvTailPolicy, PlannedCue,
    MAX_OUTPUT_BYTES, MAX_RENDER_SAMPLES, MAX_RENDER_VIDEO_FRAMES, MAX_TIMEOUT_MS,
};
