//! Persisted history + provenance (CONTRACTS.md §5, T21).
//!
//! Tracktion owns the musical undo stack; VOID persists an ordered,
//! user-facing history of transaction IDs referencing engine undo tokens
//! plus companion app-state changes. The policy is explicit: a reopened
//! project preserves musical result, alternatives and provenance — the
//! transient engine undo stack does NOT survive restart unless the engine
//! build proves otherwise. `UNDO_BOUNDARY` is the documented statement
//! shown to the user after reopen.

use serde::{Deserialize, Serialize};

pub const HISTORY_POLICY: &str =
    "save/reopen preserves musical result, alternatives, provenance and recovery checkpoints";
pub const UNDO_BOUNDARY: &str =
    "engine undo tokens are recorded for provenance; the in-memory undo stack is not \
     restored across restart — undo starts at the reopened checkpoint";

/// One committed edit in the user-facing history. A drag or an accepted
/// AI proposal equals one transaction; audition/meter updates never land
/// here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionRecord {
    pub transaction_id: String,
    /// Coordinator revision at which it committed (decimal string per §1).
    pub revision: String,
    pub label: String,
    /// Opaque engine undo token (from the engine; meaningless after
    /// restart but kept for provenance).
    pub engine_undo_token: String,
    /// Companion app-state changes applied with the same transaction.
    #[serde(default)]
    pub app_changes: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersistedHistory {
    pub policy: String,
    pub undo_boundary: String,
    /// Cursor into the committed transaction stream.
    pub history_cursor: String,
    #[serde(default)]
    pub transactions: Vec<TransactionRecord>,
}

impl Default for PersistedHistory {
    fn default() -> Self {
        Self {
            policy: HISTORY_POLICY.into(),
            undo_boundary: UNDO_BOUNDARY.into(),
            history_cursor: "0".into(),
            transactions: Vec::new(),
        }
    }
}

impl PersistedHistory {
    pub fn record(&mut self, tx: TransactionRecord) {
        self.history_cursor = tx.revision.clone();
        self.transactions.push(tx);
    }
}

/// Canonical `app-state.json` content: visual/cue metadata + provenance +
/// history. Kept minimal and typed — arbitrary blobs belong elsewhere.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppState {
    pub project_id: String,
    pub revision: String,
    #[serde(default)]
    pub visual: serde_json::Value,
    pub history: PersistedHistory,
    #[serde(default)]
    pub alternatives: Vec<serde_json::Value>,
}

impl AppState {
    pub fn serialize(&self) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec_pretty(self)
    }
    pub fn parse(bytes: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(bytes)
    }
}
