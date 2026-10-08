//! void-proposals — W13 predictive-composition proposal lifecycle.
//!
//! Pipeline: `RegionContext` (scoped digest, sha256 identity) →
//! `ProposalService::request` (admit + submit a void-jobs `Symbolic`
//! job, persist a `pending` record) → `JobRunner::run` →
//! `collect` (parse + validate the worker's proposals.json → `ready`,
//! provenance recorded) → `plan_accept` (revalidate → typed
//! InsertNoteOp plan, one transaction) → `commit_accepted` / `reject`.
//!
//! Candidates are data, never commands: no ids, paths, or ops cross
//! from the worker document into the engine (T56). Stale records are
//! never revived — revalidation mints a new record via `supersedes`
//! (T55). Nothing is applied that was not revalidated inside accept.

pub mod accept;
pub mod context;
pub mod document;
pub mod error;
pub mod record;
pub mod service;
pub mod store;

pub use accept::{
    commit_accepted, mark_stale_one, plan_accept, reject, AcceptPlan, PlannedInsert, Revalidation,
};
pub use context::{NoteEvent, RegionContext, TickRange};
pub use document::{parse_document, CandidateDoc, ProposalDocument, DOC_TAG};
pub use error::{ProposalError, Result};
pub use record::{
    AcceptedRecord, Candidate, ProposalProvenance, ProposalRecord, ProposalStatus, ProposedNote,
    StaleCause,
};
pub use service::{
    file_sha256, GenerateRequest, PendingProposal, ProposalService, MAX_PROPOSALS,
    SYMBOLIC_RUNTIME_ID,
};
pub use store::{utc_now, ProposalStore};
