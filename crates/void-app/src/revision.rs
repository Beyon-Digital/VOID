//! Coordinator revision ledger (CONTRACTS.md §1): projectRevision is a
//! monotonic u64 owned here. Only the coordinator advances it, and only
//! after the engine acknowledges the musical edit.

use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NonMonotonic;

impl std::fmt::Display for NonMonotonic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("non-monotonic revision")
    }
}

impl std::error::Error for NonMonotonic {}

#[derive(Default)]
pub struct RevisionLedger {
    revisions: HashMap<String, u64>,
}

impl RevisionLedger {
    /// Current committed revision for a project (0 = nothing committed).
    pub fn current(&self, project_id: &str) -> u64 {
        self.revisions.get(project_id).copied().unwrap_or(0)
    }

    /// Advance to the engine-acknowledged revision. Rejects non-monotonic
    /// regressions.
    pub fn commit(&mut self, project_id: &str, revision: u64) -> Result<u64, NonMonotonic> {
        let entry = self.revisions.entry(project_id.to_string()).or_insert(0);
        if revision <= *entry {
            return Err(NonMonotonic);
        }
        *entry = revision;
        Ok(revision)
    }

    /// Expected-revision check for an incoming command.
    pub fn check_expected(&self, project_id: &str, expected: u64) -> bool {
        self.current(project_id) == expected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monotonic_only() {
        let mut l = RevisionLedger::default();
        assert_eq!(l.current("p"), 0);
        assert!(l.commit("p", 1).is_ok());
        assert!(l.commit("p", 1).is_err());
        assert!(l.commit("p", 3).is_ok());
        assert!(l.commit("p", 2).is_err());
        assert_eq!(l.current("p"), 3);
    }
}
