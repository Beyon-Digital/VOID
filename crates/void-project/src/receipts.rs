//! `command-receipts.json` — committed receipt records persisted with each
//! checkpoint (CONTRACTS.md §3). On recovery these are the ground truth for
//! dedup; commands newer than the checkpoint are OUTCOME_UNKNOWN
//! candidates, never replayed.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use void_protocol::proto::AckStatus;
use void_protocol::receipts::StoredReceipt;

/// Wire-safe serialization of a stored receipt (FlatBuffers enums are
/// represented by name — stable across codegen revisions).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptRecord {
    pub command_id: String,
    pub payload_hash: String,
    /// `APPLIED` | `DUPLICATE` | `REJECTED` | `OUTCOME_UNKNOWN` | ...
    pub status: String,
    /// Decimal string per §1.
    pub revision: String,
    pub engine_epoch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReceiptLog {
    pub format_version: u32,
    pub project_id: String,
    /// Highest coordinator revision the records cover (decimal string).
    pub upto_revision: String,
    #[serde(default)]
    pub receipts: Vec<ReceiptRecord>,
}

pub fn serialize_log(log: &ReceiptLog) -> serde_json::Result<Vec<u8>> {
    serde_json::to_vec_pretty(log)
}

pub fn parse_log(bytes: &[u8]) -> serde_json::Result<ReceiptLog> {
    serde_json::from_slice(bytes)
}

pub fn committed_command_ids(log: &ReceiptLog) -> HashSet<String> {
    log.receipts
        .iter()
        .filter(|r| r.status == "APPLIED" || r.status == "DUPLICATE")
        .map(|r| r.command_id.clone())
        .collect()
}

impl From<&StoredReceipt> for ReceiptRecord {
    fn from(r: &StoredReceipt) -> Self {
        Self {
            command_id: r.command_id.clone(),
            payload_hash: r.payload_hash.clone(),
            status: ack_status_name(r.status).to_string(),
            revision: r.revision.to_string(),
            engine_epoch: r.engine_epoch.to_string(),
        }
    }
}

pub fn ack_status_name(s: AckStatus) -> &'static str {
    match s {
        AckStatus::APPLIED => "APPLIED",
        AckStatus::DUPLICATE => "DUPLICATE",
        AckStatus::REJECTED => "REJECTED",
        AckStatus::OUTCOME_UNKNOWN => "OUTCOME_UNKNOWN",
        _ => "UNKNOWN",
    }
}
