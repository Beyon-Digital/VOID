//! void-jobs — app-private SQLite index + job state machine (W05/W12,
//! CONTRACTS.md §4/§6). Reconstructable index for committed checkpoints,
//! recent projects and assets; transactional store for AI/analysis jobs.
//! The container's CURRENT pointer is always the save authority.

mod admission;
mod budget;
mod db;
mod error;
mod job;
pub mod layout;
mod proto;
mod provenance;
mod reconcile;
mod runner;
mod worker;

pub use admission::{decide, is_heavy, Admit};
pub use budget::{monotonic_ns, JobBudget};
pub use db::JobDb;
pub use error::{JobError, Result};
pub use job::{transition, JobKind, JobRecord, JobSpec, JobStatus, Reservations};
pub use proto::{
    JobEvent, JobProgress, WorkerEvent, WorkerOutcome, WorkerResult, WORKER_PROTOCOL_VERSION,
};
pub use provenance::{JobBudgetRecord, JobProvenance, Measured, OutputArtifact, RuntimeProvenance};
pub use reconcile::{reconcile, ReconcileOutcome};
pub use runner::{ArtifactRecord, EventSink, JobRunner, RunContext, RunOutcome};
pub use worker::{CancelToken, WorkerRuntime};
