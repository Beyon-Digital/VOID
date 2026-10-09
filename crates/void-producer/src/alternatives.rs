//! Project alternatives model (T80-side).
//!
//! An alternative is a named, branched project state — "Song v2" over
//! "Song v1" — modeled as a checkpoint lineage with an asset
//! protection set. This is the *model* layer (data + rules): the engine
//! lane owns actual Edit branching (NEEDS gap, recorded). The rules
//! that are real here: alternatives are immutable once sealed (a sealed
//! alternative's checkpoint_id/asset set never changes), switching is
//! a pointer move recorded in the ledger, and consolidation MUST NOT
//! evict an asset that any listed alternative still references
//! (`protected_asset_set`).

use crate::error::{ProducerError, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alternative {
    pub alternative_id: String,
    pub name: String,
    /// Checkpoint (immutable) this alternative is anchored to.
    pub checkpoint_id: String,
    /// Parent alternative this branched from (None = the main line).
    pub parent_id: Option<String>,
    /// Asset hashes this alternative references — the protection set.
    pub asset_hashes: BTreeSet<String>,
    /// Sealed = lineage-frozen: nothing may be added/removed. Seal is
    /// one-way (like proposal terminal states — a record, not a toggle).
    pub sealed: bool,
    pub created_utc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlternativeLedger {
    pub format_version: u32,
    pub project_id: String,
    /// Currently-checked-out alternative id (None = main line).
    pub active: Option<String>,
    pub alternatives: Vec<Alternative>,
    /// Switch audit trail — every pointer move recorded.
    pub history: Vec<AlternativeSwitch>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlternativeSwitch {
    pub from: Option<String>,
    pub to: Option<String>,
    pub at_utc: String,
}

fn now() -> String {
    void_proposals::store::utc_now()
}

impl AlternativeLedger {
    pub fn new(project_id: &str) -> Self {
        Self {
            format_version: 1,
            project_id: project_id.to_string(),
            active: None,
            alternatives: Vec::new(),
            history: Vec::new(),
        }
    }
    pub fn get(&self, id: &str) -> Option<&Alternative> {
        self.alternatives.iter().find(|a| a.alternative_id == id)
    }
    /// Create a new alternative anchored at `checkpoint_id`. Name must
    /// be unique and non-empty; assets recorded are the protection set.
    pub fn create(
        &mut self,
        name: &str,
        checkpoint_id: &str,
        parent_id: Option<&str>,
        asset_hashes: impl IntoIterator<Item = String>,
    ) -> Result<Alternative> {
        let name = name.trim();
        if name.is_empty() || name.len() > 64 {
            return Err(ProducerError::InvalidSpec("bad alternative name".into()));
        }
        if self.alternatives.iter().any(|a| a.name == name) {
            return Err(ProducerError::InvalidSpec(format!(
                "alternative name {name:?} already exists"
            )));
        }
        if let Some(pid) = parent_id {
            let parent = self
                .get(pid)
                .ok_or_else(|| ProducerError::NotFound(pid.into()))?;
            if !parent.sealed {
                return Err(ProducerError::InvalidSpec(
                    "branch only from a sealed alternative (anchor must be frozen)".into(),
                ));
            }
        }
        let alt = Alternative {
            alternative_id: format!("alt_{}", uuid::Uuid::new_v4().simple()),
            name: name.to_string(),
            checkpoint_id: checkpoint_id.to_string(),
            parent_id: parent_id.map(|s| s.to_string()),
            asset_hashes: asset_hashes.into_iter().collect(),
            sealed: false,
            created_utc: now(),
        };
        self.alternatives.push(alt.clone());
        Ok(alt)
    }
    /// Seal an alternative — one-way. Sealing freezes the checkpoint
    /// anchor + asset set so consolidation and switches stay honest.
    pub fn seal(&mut self, id: &str) -> Result<()> {
        let a = self
            .alternatives
            .iter_mut()
            .find(|a| a.alternative_id == id)
            .ok_or_else(|| ProducerError::NotFound(id.into()))?;
        if a.sealed {
            return Err(ProducerError::InvalidTransition {
                from: "sealed",
                to: "sealed",
            });
        }
        a.sealed = true;
        Ok(())
    }
    /// Switch the active pointer. `Some(id)` must exist; `None` = main.
    pub fn switch(&mut self, to: Option<&str>) -> Result<()> {
        if let Some(id) = to {
            self.get(id)
                .ok_or_else(|| ProducerError::NotFound(id.into()))?;
        }
        let from = self.active.clone();
        if from.as_deref() == to {
            return Ok(()); // no-op is honest no-op, not an error
        }
        self.active = to.map(|s| s.to_string());
        self.history.push(AlternativeSwitch {
            from,
            to: to.map(|s| s.to_string()),
            at_utc: now(),
        });
        Ok(())
    }
    /// Union of every alternative's asset set — the "do not evict"
    /// list consolidation consumes.
    pub fn protected_asset_set(&self) -> Vec<String> {
        let mut all = BTreeSet::new();
        for a in &self.alternatives {
            all.extend(a.asset_hashes.iter().cloned());
        }
        all.into_iter().collect()
    }
    /// Delete only while unsealed and inactive and not a parent of a
    /// remaining alternative — real constraints, not a soft flag.
    pub fn delete(&mut self, id: &str) -> Result<()> {
        let idx = self
            .alternatives
            .iter()
            .position(|a| a.alternative_id == id)
            .ok_or_else(|| ProducerError::NotFound(id.into()))?;
        let a = &self.alternatives[idx];
        if a.sealed {
            return Err(ProducerError::InvalidSpec(
                "sealed alternatives cannot be deleted".into(),
            ));
        }
        if self
            .alternatives
            .iter()
            .any(|x| x.parent_id.as_deref() == Some(id))
        {
            return Err(ProducerError::InvalidSpec(
                "alternative is a parent of another".into(),
            ));
        }
        if self.active.as_deref() == Some(id) {
            return Err(ProducerError::InvalidSpec(
                "cannot delete the active alternative".into(),
            ));
        }
        self.alternatives.remove(idx);
        Ok(())
    }
}

/// JSON persistence under `<root>/alternatives.json` (tmp+rename).
pub fn save_ledger(root: &Path, l: &AlternativeLedger) -> Result<()> {
    std::fs::create_dir_all(root)?;
    let dst = root.join("alternatives.json");
    let tmp = dst.with_extension("tmp");
    std::fs::write(&tmp, serde_json::to_vec_pretty(l)?)?;
    std::fs::rename(&tmp, &dst)?;
    Ok(())
}

pub fn load_ledger(root: &Path) -> Result<AlternativeLedger> {
    let p = root.join("alternatives.json");
    if !p.exists() {
        return Err(ProducerError::NotFound("alternatives.json".into()));
    }
    Ok(serde_json::from_slice(&std::fs::read(p)?)?)
}

/// Load or create an empty ledger for `project_id`.
pub fn open_or_create(root: &Path, project_id: &str) -> Result<AlternativeLedger> {
    match load_ledger(root) {
        Ok(l) => Ok(l),
        Err(ProducerError::NotFound(_)) => Ok(AlternativeLedger::new(project_id)),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_seal_branch_switch() {
        let mut l = AlternativeLedger::new("p");
        let a = l.create("v1", "ck1", None, vec!["sha-a".into()]).unwrap();
        assert!(l.create("v1", "ckX", None, vec![]).is_err()); // dup name
        assert!(l
            .create("v2", "ck2", Some(&a.alternative_id), vec![])
            .is_err()); // parent unsealed
        l.seal(&a.alternative_id).unwrap();
        assert!(l.seal(&a.alternative_id).is_err()); // already sealed
        let b = l
            .create("v2", "ck2", Some(&a.alternative_id), vec!["sha-b".into()])
            .unwrap();
        l.switch(Some(&a.alternative_id)).unwrap();
        l.switch(Some(&b.alternative_id)).unwrap();
        l.switch(None).unwrap();
        assert_eq!(l.history.len(), 3);
        assert_eq!(l.protected_asset_set(), vec!["sha-a", "sha-b"]);
        // Can't delete sealed or parented.
        assert!(l.delete(&a.alternative_id).is_err());
        l.seal(&b.alternative_id).unwrap();
        assert!(l.delete(&b.alternative_id).is_err());
    }

    #[test]
    fn unsealed_active_delete_rules() {
        let mut l = AlternativeLedger::new("p");
        let a = l.create("wip", "ck", None, vec![]).unwrap();
        l.switch(Some(&a.alternative_id)).unwrap();
        assert!(l.delete(&a.alternative_id).is_err()); // active
        l.switch(None).unwrap();
        l.delete(&a.alternative_id).unwrap();
        assert!(l.get(&a.alternative_id).is_none());
    }

    #[test]
    fn roundtrip_disk() {
        let dir = tempfile::tempdir().unwrap();
        let mut l = AlternativeLedger::new("p");
        let a = l.create("v1", "ck1", None, vec!["s".into()]).unwrap();
        l.switch(Some(&a.alternative_id)).unwrap();
        save_ledger(dir.path(), &l).unwrap();
        let back = load_ledger(dir.path()).unwrap();
        assert_eq!(back.alternatives.len(), 1);
        assert_eq!(back.active.as_deref(), Some(a.alternative_id.as_str()));
    }
}
