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
    Forwarded {
        command_id: String,
    },
    /// Worker is unavailable for this command.
    WorkerUnavailable,
    Busy,
}

pub struct Coordinator {
    pub registry: ProjectRegistry,
    pub revisions: RevisionLedger,
    pub receipts: ReceiptStore,
}

impl Default for Coordinator {
    fn default() -> Self {
        Self::new()
    }
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
    ///
    /// `lifecycle` marks ops that establish the project binding
    /// (CreateProjectOp/OpenProjectOp): they are allowed through while the
    /// project is `Registered` (attach in flight); every other op on a
    /// `Registered` project still gets BUSY.
    pub fn preflight(
        &mut self,
        cmd: proto::PersistentCommand,
        payload_hash: &str,
        lifecycle: bool,
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
                if !lifecycle {
                    return Some((ErrorCode::BUSY, "engine still attaching".into()));
                }
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
#[allow(dead_code)]
pub fn enqueue(registry: &mut ProjectRegistry, project_id: &str, command_id: &str) -> bool {
    match registry.get_mut(project_id) {
        Some(p) => p.try_enqueue(command_id.to_string()),
        None => false,
    }
}

/// A project is eligible to address the engine only while attached in the
/// same epoch — the guard the coordinator applies before forwarding.
#[allow(dead_code)]
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

    fn create_project_cmd(
        command_id: &str,
        project_id: &str,
    ) -> flatbuffers::FlatBufferBuilder<'static> {
        let mut b = flatbuffers::FlatBufferBuilder::new();
        let name = b.create_string("t");
        let dir = b.create_string("/tmp/t.void");
        let op = proto::CreateProjectOp::create(
            &mut b,
            &proto::CreateProjectOpArgs {
                name: Some(name),
                container_dir: Some(dir),
                sample_rate: 48000,
                initial_bpm: 120.0,
            },
        );
        let cid = b.create_string(command_id);
        let tid = b.create_string(&uuid::Uuid::new_v4().to_string());
        let pid = b.create_string(project_id);
        let cmd = proto::PersistentCommand::create(
            &mut b,
            &proto::PersistentCommandArgs {
                command_id: Some(cid),
                transaction_id: Some(tid),
                project_id: Some(pid),
                engine_epoch: 0,
                expected_revision: 0,
                op_type: proto::PersistentOp::CreateProjectOp,
                op: Some(op.as_union_value()),
            },
        );
        b.finish(cmd, None);
        b
    }

    #[test]
    fn lifecycle_op_passes_registered_gate_others_still_busy() {
        let project_id = uuid::Uuid::new_v4().to_string();
        let cmd_a = create_project_cmd(&uuid::Uuid::new_v4().to_string(), &project_id);
        let cmd_b = create_project_cmd(&uuid::Uuid::new_v4().to_string(), &project_id);
        let a = flatbuffers::root::<proto::PersistentCommand>(cmd_a.finished_data()).unwrap();
        let b = flatbuffers::root::<proto::PersistentCommand>(cmd_b.finished_data()).unwrap();

        let mut c = Coordinator::new();
        c.registry.register(ProjectHandle {
            project_id: project_id.clone(),
            container_dir: PathBuf::from("/tmp/t.void"),
            state: ProjectState::Registered,
            pending: Default::default(),
        });

        // Lifecycle op on a Registered (attach-in-flight) project passes.
        assert!(c.preflight(a, "h-lifecycle", true).is_none());
        // Any other op on the same Registered project still gets BUSY.
        let (code, _msg) = c.preflight(b, "h-other", false).unwrap();
        assert_eq!(code, ErrorCode::BUSY);
    }
}
