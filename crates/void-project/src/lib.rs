//! void-project — the `.void` project container (W05, CONTRACTS.md §4):
//! immutable checkpoints behind an atomic `CURRENT` pointer, startup
//! recovery that verifies before trusting, copy-on-write schema
//! migration, persisted history/provenance, and receipt logs.
//!
//! Save success means `SAVE_DURABLE(R)` — a complete verified published
//! checkpoint — never queued writes.

mod checkpoint;
mod current;
mod error;
mod fsutil;
mod history;
mod layout;
mod manifest;
mod meta;
mod migrate;
mod receipts;
mod recovery;
mod verify;

pub use checkpoint::{
    save, SaveReceipt, SaveRequest, SaveSession, SaveStep, SnapshotBundle, SnapshotProvider,
};
pub use current::{read_current, CurrentPointer};
pub use error::{ProjectError, Result};
pub use history::{AppState, PersistedHistory, TransactionRecord, HISTORY_POLICY, UNDO_BOUNDARY};
pub use layout::is_project_root;
pub use manifest::{CheckpointManifest, FileRef, MANIFEST_STATE_COMPLETE};
pub use meta::{ProjectMeta, FORMAT_READ_MIN, FORMAT_VERSION};
pub use migrate::{
    create, inspect, migrate, open, rollback, MigrateStep, Migration, OpenMode, OpenOutcome,
};
pub use receipts::{committed_command_ids, parse_log, serialize_log, ReceiptLog, ReceiptRecord};
pub use recovery::{
    authoritative_pointer, recover, uncertain_commands, PointerStatus, RecordingChunk,
    RecordingJournal, RecordingSalvage, RecoveryCandidate, RecoveryReport,
};
pub use verify::{verify_checkpoint, verify_checkpoint_dir, VerifiedCheckpoint};
