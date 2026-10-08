//! Command receipt dedup store (CONTRACTS.md §3): the same command_id +
//! identical canonical payload hash returns the original receipt; the same
//! id with a different payload is COMMAND_ID_REUSE.

use crate::proto::AckStatus;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

/// Canonical payload hash for dedup — SHA-256 over the serialized op bytes
/// plus op variant tag, hex-encoded.
pub fn payload_hash(op_variant: &str, payload_bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(op_variant.as_bytes());
    h.update([0u8]);
    h.update(payload_bytes);
    format!("{:x}", h.finalize())
}

#[derive(Debug, Clone)]
pub struct StoredReceipt {
    pub command_id: String,
    pub payload_hash: String,
    pub status: AckStatus,
    pub revision: u64,
    pub engine_epoch: u64,
}

pub enum DedupOutcome {
    /// New command — proceed.
    New,
    /// Same id + same payload hash: return the stored receipt.
    Duplicate(StoredReceipt),
    /// Same id + different payload: reject COMMAND_ID_REUSE.
    IdReuse,
}

/// In-memory receipt store for the live session; committed records also
/// persist into checkpoints (crates/void-project).
#[derive(Default)]
pub struct ReceiptStore {
    by_command: HashMap<String, StoredReceipt>,
}

impl ReceiptStore {
    pub fn check(&self, command_id: &str, hash: &str) -> DedupOutcome {
        match self.by_command.get(command_id) {
            None => DedupOutcome::New,
            Some(r) if r.payload_hash == hash => DedupOutcome::Duplicate(r.clone()),
            Some(_) => DedupOutcome::IdReuse,
        }
    }

    pub fn record(&mut self, receipt: StoredReceipt) {
        self.by_command.insert(receipt.command_id.clone(), receipt);
    }

    pub fn get(&self, command_id: &str) -> Option<&StoredReceipt> {
        self.by_command.get(command_id)
    }

    pub fn len(&self) -> usize {
        self.by_command.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedup_semantics() {
        let mut store = ReceiptStore::default();
        let h = payload_hash("AddTrackOp", b"track-1");
        assert!(matches!(store.check("cmd-1", &h), DedupOutcome::New));
        store.record(StoredReceipt {
            command_id: "cmd-1".into(),
            payload_hash: h.clone(),
            status: AckStatus::APPLIED,
            revision: 7,
            engine_epoch: 1,
        });
        assert!(matches!(
            store.check("cmd-1", &h),
            DedupOutcome::Duplicate(_)
        ));
        let other = payload_hash("AddTrackOp", b"track-2");
        assert!(matches!(
            store.check("cmd-1", &other),
            DedupOutcome::IdReuse
        ));
        assert!(matches!(store.check("cmd-2", &other), DedupOutcome::New));
    }
}
