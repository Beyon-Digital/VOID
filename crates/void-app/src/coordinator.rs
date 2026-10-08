//! Command coordinator: validate -> dedup -> epoch/revision -> dispatch
//! to the engine worker -> receipt -> commit revision (CONTRACTS.md §3).

use crate::project::{ProjectRegistry, ProjectState};
use crate::revision::RevisionLedger;
use void_protocol::proto::{self, AckStatus, ErrorCode};
use void_protocol::receipts::{DedupOutcome, ReceiptStore, StoredReceipt};
use void_protocol::validate;

#[derive(Debug)]
pub enum DispatchOutcome {
    /// Receipt produced (applied or rejected) — forward to requester.
    Receipt {
        command_id: String,
        status: AckStatus,
        error: ErrorCode,
        revision: u64,
        message: String,
    },
    /// Command accepted and sent to the worker; receipt arrives async.
    Forwarded { command_id: String },
    /// Worker is unavailable for this command.
    WorkerUnavailable,
    Busy,
}

pub struct Coordinator {
    pub registry: ProjectRegistry,
    pub revisions: RevisionLedger,
    pub receipts: ReceiptStore,
}

impl Coordinator {
    pub fn new() -> Self {
        Self {
            registry: ProjectRegistry::default(),
            revisions: RevisionLedger::default(),
            receipts: ReceiptStore::default(),
        }
    }

    /// Pre-dispatch gate: everything that can be decided without touching
    /// the worker. Returns Some(rejection-fields) to fail fast, or None to
    /// proceed to dispatch.
    pub fn preflight(
        &mut self,
        cmd: proto::PersistentCommand,
        payload_hash: &str,
    ) -> Option<(ErrorCode, String)> {
        // Schema/value validation (IDs, finite floats, ranges, versions).
        if let Err(r) = validate::validate_persistent_command(cmd) {
            return Some((r.code, r.message));
        }

        let project_id = cmd.project_id().unwrap_or_default().to_string();
        let command_id = cmd.command_id().unwrap_or_default().to_string();

        // Dedup against the live receipt store.
        match self.receipts.check(&command_id, payload_hash) {
            DedupOutcome::Duplicate(_) => return Some((ErrorCode::NONE, String::new())), // signal DUPLICATE
            DedupOutcome::IdReuse => {
                return Some((
                    ErrorCode::COMMAND_ID_REUSE,
                    "command id reused with different payload".into(),
                ))
            }
            DedupOutcome::New => {}
        }

        // Project + epoch checks.
        let Some(project) = self.registry.get(&project_id) else {
            return Some((ErrorCode::NOT_FOUND, "unknown project".into()));
        };
        match project.state {
            ProjectState::Attached { engine_epoch } => {
                if engine_epoch != cmd.engine_epoch() {
                    return Some((
                        ErrorCode::STALE_EPOCH,
                        format!("engine epoch changed (current {engine_epoch})"),
                    ));
                }
            }
            ProjectState::Recoverable | ProjectState::Closed => {
                return Some((ErrorCode::WORKER_FAILED, "engine not attached".into()));
            }
            ProjectState::Registered => {
                return Some((ErrorCode::BUSY, "engine still attaching".into()));
            }
        }

        // Revision check.
        if !self
            .revisions
            .check_expected(&project_id, cmd.expected_revision())
        {
            return Some((
                ErrorCode::STALE_REVISION,
                format!(
                    "expected {} but project is at {}",
                    cmd.expected_revision(),
                    self.revisions.current(&project_id)
                ),
            ));
        }

        None
    }

    /// Record a worker receipt and advance the ledger on APPLIED.
    pub fn on_receipt(&mut self, project_id: &str, receipt: &StoredReceipt) {
        self.receipts.record(receipt.clone());
        if receipt.status == AckStatus::APPLIED {
            let _ = self.revisions.commit(project_id, receipt.revision);
        }
    }
}

/// Enqueue a command onto the project's serialized mutation lane.
pub fn enqueue(registry: &mut ProjectRegistry, project_id: &str, command_id: &str) -> bool {
    match registry.get_mut(project_id) {
        Some(p) => p.try_enqueue(command_id.to_string()),
        None => false,
    }
}

/// A project is eligible to address the engine only while attached in the
/// same epoch — the guard the coordinator applies before forwarding.
pub fn engine_ready(project_state: &ProjectState) -> bool {
    matches!(project_state, ProjectState::Attached { .. })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::ProjectHandle;
    use std::path::PathBuf;

    #[test]
    fn registry_enqueue_bounds() {
        let mut reg = ProjectRegistry::default();
        reg.register(ProjectHandle {
            project_id: "p1".into(),
            container_dir: PathBuf::from("/tmp/p1.void"),
            state: ProjectState::Attached { engine_epoch: 1 },
            pending: Default::default(),
        });
        for i in 0..void_protocol::limits::PENDING_MUTATIONS_MAX {
            assert!(enqueue(&mut reg, "p1", &format!("c{i}")));
        }
        assert!(!enqueue(&mut reg, "p1", "overflow"));
    }

    #[test]
    fn ledger_commits_through_coordinator() {
        let mut c = Coordinator::new();
        c.receipts.record(StoredReceipt {
            command_id: "c1".into(),
            payload_hash: "h".into(),
            status: AckStatus::APPLIED,
            revision: 5,
            engine_epoch: 1,
        });
        c.on_receipt(
            "p",
            &StoredReceipt {
                command_id: "c1".into(),
                payload_hash: "h".into(),
                status: AckStatus::APPLIED,
                revision: 5,
                engine_epoch: 1,
            },
        );
        assert_eq!(c.revisions.current("p"), 5);
    }
}
