//! ModuleRegistry — the on-disk, sha256-verified, versioned module
//! store. Layout:
//!
//!   <root>/index.json                          — versioned index
//!   <root>/modules/<module_id>/<version>/manifest.json
//!   <root>/modules/<module_id>/<version>/module.wasm
//!   <root>/.staging/<uuid>/                    — install staging
//!
//! Trust model: every load re-hashes the stored wasm + manifest bytes
//! and compares them to the index entry — a tampered file is a
//! `Tampered` error, never a silently loaded module. `install` binds
//! wasm↔manifest via the manifest's own `wasm_sha256` BEFORE anything
//! is persisted; staging+rename keeps partial installs invisible.
//! `revoke` marks an entry unusable for instantiate (files remain for
//! audit) and the runtime unloads live instances.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::error::{Result, WasmError};
use crate::manifest::{sha256_hex, version_cmp, MANIFEST_FILE, MODULE_FILE};
use crate::policy::HostPolicy;
use crate::spec::ModuleSpec;

const INDEX_FILE: &str = "index.json";
const INDEX_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryStatus {
    Active,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub wasm_sha256: String,
    pub manifest_sha256: String,
    pub status: EntryStatus,
    /// Monotonic install counter — audit ordering, never reused.
    pub installed_seq: u64,
    #[serde(default)]
    pub revoked_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ModuleIndex {
    index_version: u32,
    /// module_id → version → entry (ordered maps — deterministic).
    entries: BTreeMap<String, BTreeMap<String, RegistryEntry>>,
    next_seq: u64,
}

impl ModuleIndex {
    fn empty() -> Self {
        ModuleIndex {
            index_version: INDEX_VERSION,
            entries: BTreeMap::new(),
            next_seq: 1,
        }
    }
}

/// Read-only view of one registry row for listing/UI.
#[derive(Debug, Clone)]
pub struct RegistryRow {
    pub module_id: String,
    pub version: String,
    pub wasm_sha256: String,
    pub manifest_sha256: String,
    pub status: EntryStatus,
    pub installed_seq: u64,
}

pub struct ModuleRegistry {
    root: PathBuf,
    index: ModuleIndex,
}

impl ModuleRegistry {
    /// Open (or create) a registry rooted at `root`.
    pub fn open(root: &Path) -> Result<ModuleRegistry> {
        let index_path = root.join(INDEX_FILE);
        let index = if index_path.exists() {
            let text = std::fs::read_to_string(&index_path)?;
            let idx: ModuleIndex = serde_json::from_str(&text)
                .map_err(|e| WasmError::InvalidManifest(format!("registry index corrupt: {e}")))?;
            if idx.index_version != INDEX_VERSION {
                return Err(WasmError::InvalidManifest(format!(
                    "registry index_version {} unsupported",
                    idx.index_version
                )));
            }
            idx
        } else {
            ModuleIndex::empty()
        };
        std::fs::create_dir_all(root.join("modules"))?;
        Ok(ModuleRegistry {
            root: root.to_path_buf(),
            index,
        })
    }

    fn write_index(&self) -> Result<()> {
        let tmp = self.root.join(".index.json.tmp");
        let text = serde_json::to_string_pretty(&self.index)?;
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, self.root.join(INDEX_FILE))?;
        Ok(())
    }

    fn entry_dir(&self, module_id: &str, version: &str) -> PathBuf {
        self.root.join("modules").join(module_id).join(version)
    }

    /// Install a verified spec (from `ModuleSpec::from_parts` — parse,
    /// sha-bind, scan) into the registry. Rejects duplicates.
    /// Returns the recorded entry.
    pub fn install(&mut self, spec: &ModuleSpec) -> Result<RegistryEntry> {
        let id = spec.manifest.module_id.clone();
        let ver = spec.manifest.version.clone();
        if self
            .index
            .entries
            .get(&id)
            .is_some_and(|v| v.contains_key(&ver))
        {
            return Err(WasmError::DuplicateVersion(format!("{}@{}", id, ver)));
        }
        // Stage, then publish the dir; then index it. A crash between
        // dir-publish and index-write leaves an orphan dir — orphan
        // files without an index entry are never loaded (and a later
        // install of the same version would hit the dir existing —
        // overwrite-clean it first).
        let staging = self
            .root
            .join(".staging")
            .join(uuid::Uuid::new_v4().to_string());
        std::fs::create_dir_all(&staging)?;
        // Store the raw manifest bytes — the recorded sha binds to the
        // exact text presented at install, not a reserialization.
        std::fs::write(staging.join(MANIFEST_FILE), &spec.manifest_json)?;
        std::fs::write(staging.join(MODULE_FILE), &spec.wasm_bytes)?;
        let dest = self.entry_dir(&id, &ver);
        if dest.exists() {
            // Orphaned staging remnant — remove then move.
            std::fs::remove_dir_all(&dest)?;
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::rename(&staging, &dest)?;
        let entry = RegistryEntry {
            wasm_sha256: spec.manifest.wasm_sha256.clone(),
            manifest_sha256: spec.manifest_sha256.clone(),
            status: EntryStatus::Active,
            installed_seq: self.index.next_seq,
            revoked_reason: None,
        };
        self.index.next_seq += 1;
        self.index
            .entries
            .entry(id.clone())
            .or_default()
            .insert(ver.clone(), entry.clone());
        if let Err(e) = self.write_index() {
            // Roll the in-memory index back — the on-disk index stays
            // authoritative and never claims bytes it didn't verify.
            if let Some(v) = self.index.entries.get_mut(&id) {
                v.remove(&ver);
            }
            self.index.next_seq -= 1;
            return Err(e);
        }
        Ok(entry)
    }

    /// Read + re-verify one entry's bytes against the recorded hashes.
    /// This is the load path: tamper → `Tampered`, never silent.
    pub fn load_spec(
        &self,
        module_id: &str,
        version: &str,
        policy: &HostPolicy,
    ) -> Result<ModuleSpec> {
        let entry = self
            .index
            .entries
            .get(module_id)
            .and_then(|v| v.get(version))
            .ok_or_else(|| WasmError::NotFound(format!("{module_id}@{version}")))?;
        if entry.status == EntryStatus::Revoked {
            return Err(WasmError::Revoked(format!("{module_id}@{version}")));
        }
        let dir = self.entry_dir(module_id, version);
        let manifest_text = std::fs::read_to_string(dir.join(MANIFEST_FILE))?;
        let wasm_bytes = std::fs::read(dir.join(MODULE_FILE))?;
        // Re-hash both artifacts against the index BEFORE parsing.
        let m_sha = sha256_hex(manifest_text.as_bytes());
        if m_sha != entry.manifest_sha256 {
            return Err(WasmError::Tampered {
                what: format!("{module_id}@{version}/manifest.json"),
                expected: entry.manifest_sha256.clone(),
                actual: m_sha,
            });
        }
        let w_sha = sha256_hex(&wasm_bytes);
        if w_sha != entry.wasm_sha256 {
            return Err(WasmError::Tampered {
                what: format!("{module_id}@{version}/module.wasm"),
                expected: entry.wasm_sha256.clone(),
                actual: w_sha,
            });
        }
        ModuleSpec::from_parts(&manifest_text, &wasm_bytes, policy)
    }

    /// Resolve the newest active version of a module (semver order).
    pub fn latest_active(&self, module_id: &str) -> Result<String> {
        let versions = self
            .index
            .entries
            .get(module_id)
            .ok_or_else(|| WasmError::NotFound(module_id.into()))?;
        versions
            .iter()
            .filter(|(_, e)| e.status == EntryStatus::Active)
            .map(|(v, _)| v.clone())
            .max_by(|a, b| version_cmp(a, b))
            .ok_or_else(|| WasmError::NotFound(format!("{module_id} has no active version")))
    }

    /// Revoke one version (or every version) of a module. Returns the
    /// versions newly revoked — the runtime unloads those instances.
    pub fn revoke(
        &mut self,
        module_id: &str,
        version: Option<&str>,
        reason: &str,
    ) -> Result<Vec<String>> {
        let mut revoked = Vec::new();
        {
            let versions = self
                .index
                .entries
                .get_mut(module_id)
                .ok_or_else(|| WasmError::NotFound(module_id.into()))?;
            match version {
                Some(v) => {
                    let e = versions
                        .get_mut(v)
                        .ok_or_else(|| WasmError::NotFound(format!("{module_id}@{v}")))?;
                    if e.status == EntryStatus::Active {
                        e.status = EntryStatus::Revoked;
                        e.revoked_reason = Some(reason.to_string());
                        revoked.push(v.to_string());
                    }
                }
                None => {
                    for (v, e) in versions.iter_mut() {
                        if e.status == EntryStatus::Active {
                            e.status = EntryStatus::Revoked;
                            e.revoked_reason = Some(reason.to_string());
                            revoked.push(v.clone());
                        }
                    }
                }
            }
        }
        if revoked.is_empty() {
            return Err(WasmError::NotFound(format!(
                "{module_id}: no active version to revoke"
            )));
        }
        self.write_index()?;
        Ok(revoked)
    }

    /// Every row, deterministic order.
    pub fn list(&self) -> Vec<RegistryRow> {
        let mut out = Vec::new();
        for (id, versions) in &self.index.entries {
            for (version, e) in versions {
                out.push(RegistryRow {
                    module_id: id.clone(),
                    version: version.clone(),
                    wasm_sha256: e.wasm_sha256.clone(),
                    manifest_sha256: e.manifest_sha256.clone(),
                    status: e.status,
                    installed_seq: e.installed_seq,
                });
            }
        }
        out
    }

    /// Verify every entry's on-disk bytes — registry-wide tamper sweep.
    /// Returns the list of tampered/verified rows; errors abort early
    /// per-row (callers wanting a full report should iterate list()).
    pub fn verify_all(&self, policy: &HostPolicy) -> Result<Vec<String>> {
        let mut ok = Vec::new();
        for row in self.list() {
            if row.status != EntryStatus::Active {
                continue;
            }
            self.load_spec(&row.module_id, &row.version, policy)?;
            ok.push(format!("{}@{}", row.module_id, row.version));
        }
        Ok(ok)
    }
}
