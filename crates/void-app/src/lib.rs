//! VOID application coordinator (W02/W03): owns project registry,
//! coordinator revisions, command serialization, receipt dedup, and
//! worker dispatch. Musical state belongs to the engine worker — this
//! layer never mirrors the song graph.

mod coordinator;
mod project;
mod revision;

pub use coordinator::{Coordinator, DispatchOutcome};
pub use project::{ProjectHandle, ProjectRegistry, ProjectState};
pub use revision::RevisionLedger;
