//! void-jobs — app-private SQLite index + job state machine (W05/W12,
//! CONTRACTS.md §4/§6). Reconstructable index for committed checkpoints,
//! recent projects and assets; transactional store for AI/analysis jobs.
//! The container's CURRENT pointer is always the save authority.

mod db;
mod error;
mod job;
mod reconcile;

pub use db::JobDb;
pub use error::{JobError, Result};
pub use job::{transition, JobKind, JobRecord, JobSpec, JobStatus, Reservations};
pub use reconcile::{reconcile, ReconcileOutcome};
