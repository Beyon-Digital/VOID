//! Pack install / tamper / remove / relink lifecycle (W19 / SND-06, T73).
//!
//! Layout under the content root:
//!
//! ```text
//! <root>/
//!   index.json                     installed-pack ledger (atomically written)
//!   packs/<packId>/<version>/      installed payload (manifest.json + files)
//!   staging/<uuid>/                install staging — verified before publish
//!   quarantine/<packId>/<version>/ tampered files moved here, never deleted
//! ```
//!
//! Rules:
//! - Install verifies the manifest AND every declared file's sha256 before
//!   anything lands in `packs/`. A failed required file = failed install;
//!   staging is removed.
//! - Post-install tampering is detected by `rescan`: a required file whose
//!   bytes no longer match the manifest is moved to `quarantine/` and the
//!   pack status flips to `quarantined`. Optional files degrade the pack.
//! - A project that depends on a missing/quarantined pack opens with an
//!   explicit `MissingContentReport` — never silence (T73 expected
//!   outcome, CONTRACTS §4 relink semantics).
//! - Relink restores a file only when candidate bytes hash to the
//!   *expected* sha256. A different file is refused unless the caller
//!   passes `explicit_replace` — then it is recorded as a user replacement
//!   and the pack is flagged `contains_user_replacements` (the manifest no
//!   longer describes that file verbatim — an honest, recorded deviation).

use crate::error::{ContentError, Result};
use crate::manifest::{is_pack_id, ContentPackManifest};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

const INDEX_FILE: &str = "index.json";
const PACKS_DIR: &str = "packs";
const STAGING_DIR: &str = "staging";
const QUARANTINE_DIR: &str = "quarantine";

/// Per-file state recorded in the ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileState {
    /// Present and hash-verified.
    Ok,
    /// Bytes no longer match the manifest — file moved to quarantine.
    Tampered,
    /// File absent from the pack directory.
    Missing,
    /// User explicitly replaced the bytes (hash differs, recorded).
    Replaced,
}

/// Pack health after a verification pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PackStatus {
    /// Every required file verifies.
    Verified,
    /// Only optional files are missing/tampered/replaced, or the user
    /// replaced bytes deliberately. Usable with a visible caveat.
    Degraded,
    /// A required file is tampered or missing — pack is unusable until
    /// relinked or reinstalled.
    Quarantined,
}

/// Ledger row for one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRecord {
    pub state: FileState,
    /// Manifest-declared hash (the expectation).
    pub expected_sha256: String,
    /// Observed/replacement hash when it differs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actual_sha256: Option<String>,
}

/// Ledger row for one installed version of a pack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledPack {
    pub pack_id: String,
    pub version: String,
    pub name: String,
    pub family: String,
    pub license_id: String,
    pub status: PackStatus,
    #[serde(default)]
    pub contains_user_replacements: bool,
    /// file path -> state.
    pub files: BTreeMap<String, FileRecord>,
}

/// The on-disk ledger (`index.json`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct LibraryIndex {
    pub format_version: u32,
    /// packId -> version -> record.
    #[serde(default)]
    pub packs: BTreeMap<String, BTreeMap<String, InstalledPack>>,
}

/// One missing-content entry for a project open report (T73).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingContent {
    pub pack_id: String,
    /// Required version the project references, or `None` for "any".
    pub version: Option<String>,
    /// Pack-relative path of the missing/tampered file.
    pub path: String,
    /// The sha the project needs — the relink expectation.
    pub expected_sha256: String,
    /// Human-facing name for the relink UI.
    pub display_name: String,
    /// Why it's missing.
    pub reason: FileState,
}

/// What a project open reports about the packs it depends on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MissingContentReport {
    /// Entries for every file that is not usable right now.
    pub missing: Vec<MissingContent>,
    /// Packs that resolved cleanly (usable as-is).
    pub ok_packs: Vec<String>,
    /// True when every dependency resolved — caller may open silently.
    pub complete: bool,
}

/// A pack a project depends on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackDependency {
    pub pack_id: String,
    /// Optional exact version; `None` accepts the newest installed.
    pub version: Option<String>,
}

/// The content library root. All mutating operations persist the index
/// atomically via `void_assets::write_atomic`.
pub struct ContentStore {
    root: PathBuf,
}

impl ContentStore {
    /// Open (and create) the content root.
    pub fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(root.join(PACKS_DIR))?;
        fs::create_dir_all(root.join(STAGING_DIR))?;
        fs::create_dir_all(root.join(QUARANTINE_DIR))?;
        let store = Self { root };
        if !store.root.join(INDEX_FILE).exists() {
            store.save_index(&LibraryIndex {
                format_version: 1,
                ..Default::default()
            })?;
        }
        Ok(store)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn index_path(&self) -> PathBuf {
        self.root.join(INDEX_FILE)
    }

    pub fn load_index(&self) -> Result<LibraryIndex> {
        match fs::read(self.index_path()) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(LibraryIndex {
                format_version: 1,
                ..Default::default()
            }),
            Err(e) => Err(e.into()),
        }
    }

    fn save_index(&self, index: &LibraryIndex) -> Result<()> {
        void_assets::write_atomic(
            &self.index_path(),
            serde_json::to_string_pretty(index)?.as_bytes(),
        )?;
        Ok(())
    }

    fn pack_dir(&self, pack_id: &str, version: &str) -> PathBuf {
        self.root.join(PACKS_DIR).join(pack_id).join(version)
    }

    /// The newest installed version of a pack by semver order.
    pub fn installed_version(&self, pack_id: &str) -> Result<Option<String>> {
        let index = self.load_index()?;
        Ok(index
            .packs
            .get(pack_id)
            .and_then(|vs| {
                vs.keys()
                    .filter_map(|v| semver::Version::parse(v).ok())
                    .max()
            })
            .map(|v| v.to_string()))
    }

    /// Install a pack directory containing `manifest.json` + payload.
    ///
    /// Full verification happens in `staging/`; only a clean pack is
    /// published to `packs/`. Reinstalling the same id@version is an
    /// error (`PackExists`) — remove first.
    pub fn install(&self, pack_dir: &Path) -> Result<InstalledPack> {
        let manifest_bytes = fs::read(pack_dir.join("manifest.json"))
            .map_err(|_| ContentError::InvalidManifest("manifest.json missing".into()))?;
        let manifest = ContentPackManifest::parse_and_verify(&manifest_bytes)?;

        let index = self.load_index()?;
        if index
            .packs
            .get(&manifest.pack_id)
            .is_some_and(|vs| vs.contains_key(&manifest.version))
        {
            return Err(ContentError::PackExists {
                pack_id: manifest.pack_id.clone(),
                version: manifest.version.clone(),
            });
        }

        // Stage: copy every declared file into staging/<uuid>/ and hash it.
        let staging = self.root.join(STAGING_DIR).join(uuid_v4());
        fs::create_dir_all(&staging)?;
        let install_result = self.verify_payload(&manifest, pack_dir, Some(&staging));
        let (files, fatal) = match install_result {
            Ok(f) => (f, false),
            Err(e) => {
                let _ = fs::remove_dir_all(&staging);
                return Err(e);
            }
        };
        let _ = fatal;

        // Any required file that did not verify → refuse the install.
        let bad_required = manifest
            .files
            .iter()
            .filter(|f| f.required)
            .any(|f| files.get(&f.path).map(|r| r.state) != Some(FileState::Ok));
        if bad_required {
            let _ = fs::remove_dir_all(&staging);
            let first_bad = manifest
                .files
                .iter()
                .find(|f| f.required && files.get(&f.path).map(|r| r.state) != Some(FileState::Ok));
            let rec = first_bad.and_then(|f| files.get(&f.path));
            return Err(match rec.map(|r| r.state) {
                Some(FileState::Tampered) => ContentError::FileTampered {
                    path: first_bad.unwrap().path.clone(),
                    declared: first_bad.unwrap().sha256.clone(),
                    actual: rec
                        .and_then(|r| r.actual_sha256.clone())
                        .unwrap_or_default(),
                },
                _ => ContentError::FileMissing(first_bad.unwrap().path.clone()),
            });
        }

        // Publish: staging tree -> packs/<id>/<version>/.
        let dest = self.pack_dir(&manifest.pack_id, &manifest.version);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)?;
        }
        // Move manifest first, then staged payload files.
        fs::create_dir_all(&dest)?;
        fs::copy(pack_dir.join("manifest.json"), dest.join("manifest.json"))?;
        for f in &manifest.files {
            let src = staging.join(&f.path);
            let tgt = dest.join(&f.path);
            if let Some(parent) = tgt.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(&src, &tgt)?;
        }
        let _ = fs::remove_dir_all(&staging);
        if let Some(parent) = dest.parent() {
            let _ = void_assets::sync_dir(parent);
        }

        let has_optional_gap = manifest
            .files
            .iter()
            .any(|f| !f.required && files.get(&f.path).map(|r| r.state) != Some(FileState::Ok));
        let record = InstalledPack {
            pack_id: manifest.pack_id.clone(),
            version: manifest.version.clone(),
            name: manifest.name.clone(),
            family: manifest.family.clone(),
            license_id: manifest.license_id.clone(),
            status: if has_optional_gap {
                PackStatus::Degraded
            } else {
                PackStatus::Verified
            },
            contains_user_replacements: false,
            files,
        };
        let mut index = self.load_index()?;
        index
            .packs
            .entry(manifest.pack_id.clone())
            .or_default()
            .insert(manifest.version.clone(), record.clone());
        self.save_index(&index)?;
        Ok(record)
    }

    /// Hash every manifest file under `src_dir`, optionally copying it to
    /// `stage_dir` first (install path stages bytes then hashes the copy —
    /// never trust the source twice).
    fn verify_payload(
        &self,
        manifest: &ContentPackManifest,
        src_dir: &Path,
        stage_dir: Option<&Path>,
    ) -> Result<BTreeMap<String, FileRecord>> {
        let mut out = BTreeMap::new();
        for f in &manifest.files {
            let src = src_dir.join(&f.path);
            let staged = match stage_dir {
                Some(stage) => {
                    let tgt = stage.join(&f.path);
                    if let Some(parent) = tgt.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    if src.exists() {
                        fs::copy(&src, &tgt)?;
                        tgt
                    } else {
                        src.clone()
                    }
                }
                None => src,
            };
            let rec = if !staged.exists() {
                FileRecord {
                    state: FileState::Missing,
                    expected_sha256: f.sha256.clone(),
                    actual_sha256: None,
                }
            } else {
                let actual = void_assets::file_sha256(&staged)?;
                let len_ok = staged
                    .metadata()
                    .map(|m| m.len().to_string() == f.bytes)
                    .unwrap_or(false);
                if actual == f.sha256 && len_ok {
                    FileRecord {
                        state: FileState::Ok,
                        expected_sha256: f.sha256.clone(),
                        actual_sha256: None,
                    }
                } else {
                    FileRecord {
                        state: FileState::Tampered,
                        expected_sha256: f.sha256.clone(),
                        actual_sha256: Some(actual),
                    }
                }
            };
            out.insert(f.path.clone(), rec);
        }
        Ok(out)
    }

    /// Re-verify an installed pack in place. Tampered files move to
    /// `quarantine/` (bytes preserved for forensics, never served).
    /// Returns the updated record.
    pub fn rescan(&self, pack_id: &str, version: &str) -> Result<InstalledPack> {
        let mut index = self.load_index()?;
        let record = index
            .packs
            .get_mut(pack_id)
            .and_then(|vs| vs.get_mut(version))
            .ok_or_else(|| ContentError::PackNotFound(format!("{pack_id}@{version}")))?;

        let manifest_bytes = fs::read(self.pack_dir(pack_id, version).join("manifest.json"))
            .map_err(|_| ContentError::InvalidManifest("installed manifest unreadable".into()))?;
        let manifest = ContentPackManifest::parse_and_verify(&manifest_bytes)?;

        let dir = self.pack_dir(pack_id, version);
        let mut files = self.verify_payload(&manifest, &dir, None)?;

        // Preserve user replacements: a file whose bytes still match the
        // *recorded* replacement hash stays `Replaced` — it must not
        // regress to `Tampered` just because it differs from the manifest.
        for (path, frec) in files.iter_mut() {
            if let Some(prev) = record.files.get(path) {
                if prev.state == FileState::Replaced
                    && frec.state == FileState::Tampered
                    && prev.actual_sha256.is_some()
                    && frec.actual_sha256 == prev.actual_sha256
                {
                    frec.state = FileState::Replaced;
                }
            }
        }

        // Quarantine tampered files: move bytes out of the served tree.
        for (path, rec) in &files {
            if rec.state == FileState::Tampered {
                let src = dir.join(path);
                let dst = self
                    .root
                    .join(QUARANTINE_DIR)
                    .join(pack_id)
                    .join(version)
                    .join(path);
                if src.exists() {
                    if let Some(parent) = dst.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::rename(&src, &dst)?;
                }
            }
        }

        let required_bad = manifest.files.iter().filter(|f| f.required).any(|f| {
            matches!(
                files.get(&f.path).map(|r| r.state),
                Some(FileState::Tampered) | Some(FileState::Missing) | None
            )
        });
        let optional_bad = manifest.files.iter().filter(|f| !f.required).any(|f| {
            matches!(
                files.get(&f.path).map(|r| r.state),
                Some(FileState::Tampered)
                    | Some(FileState::Missing)
                    | Some(FileState::Replaced)
                    | None
            )
        });
        record.files = files;
        record.status = if required_bad {
            PackStatus::Quarantined
        } else if optional_bad || record.contains_user_replacements {
            PackStatus::Degraded
        } else {
            PackStatus::Verified
        };
        let updated = record.clone();
        self.save_index(&index)?;
        Ok(updated)
    }

    /// Remove an installed pack version (files + ledger row).
    /// Dependents then resolve through `dependency_report` — removal is
    /// recorded, not silent.
    pub fn remove(&self, pack_id: &str, version: &str) -> Result<()> {
        if !is_pack_id(pack_id) {
            return Err(ContentError::PackNotFound(pack_id.into()));
        }
        let mut index = self.load_index()?;
        let removed = index
            .packs
            .get_mut(pack_id)
            .and_then(|vs| vs.remove(version));
        if removed.is_none() {
            return Err(ContentError::PackNotFound(format!("{pack_id}@{version}")));
        }
        if index.packs.get(pack_id).is_some_and(|vs| vs.is_empty()) {
            index.packs.remove(pack_id);
        }
        let dir = self.pack_dir(pack_id, version);
        if dir.exists() {
            fs::remove_dir_all(&dir)?;
        }
        // Drop empty <packId> dir.
        let parent = self.root.join(PACKS_DIR).join(pack_id);
        if parent.exists()
            && fs::read_dir(&parent)
                .map(|mut d| d.next().is_none())
                .unwrap_or(false)
        {
            let _ = fs::remove_dir(&parent);
        }
        self.save_index(&index)
    }

    /// Build the explicit missing-content report a dependent project
    /// gets at open time (T73). Every dependency resolves to either an
    /// `ok_packs` entry or concrete `missing` rows — never silence.
    pub fn dependency_report(&self, deps: &[PackDependency]) -> Result<MissingContentReport> {
        let index = self.load_index()?;
        let mut missing = Vec::new();
        let mut ok_packs = Vec::new();
        for dep in deps {
            let resolved_version = match &dep.version {
                Some(v) => Some(v.clone()),
                None => index
                    .packs
                    .get(&dep.pack_id)
                    .and_then(|vs| {
                        vs.keys()
                            .filter_map(|v| semver::Version::parse(v).ok())
                            .max()
                    })
                    .map(|v| v.to_string()),
            };
            let record = resolved_version
                .as_deref()
                .and_then(|v| index.packs.get(&dep.pack_id).and_then(|vs| vs.get(v)));
            match record {
                Some(rec) if rec.status == PackStatus::Verified => {
                    ok_packs.push(format!("{}@{}", dep.pack_id, rec.version));
                }
                Some(rec) => {
                    // Degraded/quarantined: list every non-ok file.
                    for (path, frec) in &rec.files {
                        if frec.state != FileState::Ok {
                            missing.push(MissingContent {
                                pack_id: dep.pack_id.clone(),
                                version: Some(rec.version.clone()),
                                path: path.clone(),
                                expected_sha256: frec.expected_sha256.clone(),
                                display_name: format!("{} {path}", rec.name),
                                reason: frec.state,
                            });
                        }
                    }
                }
                None => {
                    // Pack entirely absent — the dependency itself is
                    // missing; report one anchor row per dep so the UI
                    // has something explicit to act on.
                    missing.push(MissingContent {
                        pack_id: dep.pack_id.clone(),
                        version: resolved_version.or_else(|| dep.version.clone()),
                        path: String::new(),
                        expected_sha256: String::new(),
                        display_name: dep.pack_id.clone(),
                        reason: FileState::Missing,
                    });
                }
            }
        }
        Ok(MissingContentReport {
            complete: missing.is_empty(),
            missing,
            ok_packs,
        })
    }

    /// Relink attempt for one missing/tampered file.
    ///
    /// `candidate` bytes must hash to `expected_sha256` to restore.
    /// `explicit_replace` accepts different bytes — recorded as a
    /// `Replaced` state and the pack flagged `contains_user_replacements`
    /// (the manifest stays untouched; the deviation is ledger-visible).
    pub fn relink(
        &self,
        pack_id: &str,
        version: &str,
        path: &str,
        candidate: &[u8],
        explicit_replace: bool,
    ) -> Result<FileState> {
        let mut index = self.load_index()?;
        let record = index
            .packs
            .get_mut(pack_id)
            .and_then(|vs| vs.get_mut(version))
            .ok_or_else(|| ContentError::PackNotFound(format!("{pack_id}@{version}")))?;
        let frec = record
            .files
            .get_mut(path)
            .ok_or_else(|| ContentError::FileMissing(path.into()))?;
        if frec.state == FileState::Ok {
            return Err(ContentError::NotMissing);
        }
        let actual = void_assets::hex_sha256(candidate);
        let dir = self.pack_dir(pack_id, version);
        if actual == frec.expected_sha256 {
            let tgt = dir.join(path);
            if let Some(parent) = tgt.parent() {
                fs::create_dir_all(parent)?;
            }
            void_assets::write_atomic(&tgt, candidate)?;
            frec.state = FileState::Ok;
            frec.actual_sha256 = None;
        } else {
            if !explicit_replace {
                return Err(ContentError::HashMismatch {
                    expected: frec.expected_sha256.clone(),
                    actual,
                });
            }
            let tgt = dir.join(path);
            if let Some(parent) = tgt.parent() {
                fs::create_dir_all(parent)?;
            }
            void_assets::write_atomic(&tgt, candidate)?;
            frec.state = FileState::Replaced;
            frec.actual_sha256 = Some(actual);
            record.contains_user_replacements = true;
        }
        // Recompute pack status from the file table.
        let any_tampered_or_missing = record
            .files
            .values()
            .any(|f| matches!(f.state, FileState::Tampered | FileState::Missing));
        let any_replaced = record
            .files
            .values()
            .any(|f| f.state == FileState::Replaced);
        record.status = if any_tampered_or_missing {
            // Whether it's required matters — check the manifest.
            let required_bad = fs::read(dir.join("manifest.json"))
                .ok()
                .and_then(|b| ContentPackManifest::parse_and_verify(&b).ok())
                .map(|m| {
                    m.files.iter().any(|f| {
                        f.required
                            && record.files.get(&f.path).is_some_and(|r| {
                                matches!(r.state, FileState::Tampered | FileState::Missing)
                            })
                    })
                })
                .unwrap_or(true);
            if required_bad {
                PackStatus::Quarantined
            } else {
                PackStatus::Degraded
            }
        } else if any_replaced || record.contains_user_replacements {
            PackStatus::Degraded
        } else {
            PackStatus::Verified
        };
        let new_state = record.files.get(path).map(|r| r.state).unwrap();
        self.save_index(&index)?;
        Ok(new_state)
    }

    /// List installed packs (ledger view).
    pub fn list(&self) -> Result<Vec<InstalledPack>> {
        let index = self.load_index()?;
        Ok(index
            .packs
            .values()
            .flat_map(|vs| vs.values().cloned())
            .collect())
    }
}

fn uuid_v4() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::sign_manifest;

    fn make_pack(dir: &Path, files: &[(&str, &[u8])]) -> PathBuf {
        let pack = dir.join("pack-src");
        let mut fjson = Vec::new();
        for (path, bytes) in files {
            let p = pack.join(path);
            fs::create_dir_all(p.parent().unwrap()).unwrap();
            fs::write(&p, bytes).unwrap();
            fjson.push(serde_json::json!({
                "path": path,
                "sha256": void_assets::hex_sha256(bytes),
                "bytes": bytes.len().to_string(),
                "kind": "sample",
                "required": true,
            }));
        }
        let doc = sign_manifest(serde_json::json!({
            "formatVersion": 1,
            "packId": "test-pack",
            "version": "1.0.0",
            "name": "Test Pack",
            "family": "STOCK-03",
            "licenseId": "void-original",
            "voiceBudget": "16",
            "cacheBudgetBytes": "1048576",
            "files": fjson,
        }));
        fs::write(
            pack.join("manifest.json"),
            serde_json::to_string_pretty(&doc).unwrap(),
        )
        .unwrap();
        pack
    }

    #[test]
    fn install_verify_tamper_rescan() {
        let tmp = tempfile::tempdir().unwrap();
        let store = ContentStore::open(tmp.path().join("lib")).unwrap();
        let pack = make_pack(tmp.path(), &[("a.wav", b"aaa"), ("b.wav", b"bbb")]);
        let rec = store.install(&pack).unwrap();
        assert_eq!(rec.status, PackStatus::Verified);

        // Tamper with an installed file.
        fs::write(
            store.pack_dir("test-pack", "1.0.0").join("a.wav"),
            b"hacked",
        )
        .unwrap();
        let rec = store.rescan("test-pack", "1.0.0").unwrap();
        assert_eq!(rec.status, PackStatus::Quarantined);
        assert_eq!(rec.files["a.wav"].state, FileState::Tampered);
        assert!(store
            .root
            .join(QUARANTINE_DIR)
            .join("test-pack/1.0.0/a.wav")
            .exists());
        assert!(!store.pack_dir("test-pack", "1.0.0").join("a.wav").exists());

        // Report: explicit, not silence.
        let rep = store
            .dependency_report(&[PackDependency {
                pack_id: "test-pack".into(),
                version: None,
            }])
            .unwrap();
        assert!(!rep.complete);
        assert_eq!(rep.missing[0].path, "a.wav");
        assert_eq!(rep.missing[0].reason, FileState::Tampered);

        // Wrong relink bytes refused.
        let err = store
            .relink("test-pack", "1.0.0", "a.wav", b"wrong", false)
            .unwrap_err();
        assert!(matches!(err, ContentError::HashMismatch { .. }));

        // Correct bytes restore.
        let st = store
            .relink("test-pack", "1.0.0", "a.wav", b"aaa", false)
            .unwrap();
        assert_eq!(st, FileState::Ok);
        let rep = store
            .dependency_report(&[PackDependency {
                pack_id: "test-pack".into(),
                version: None,
            }])
            .unwrap();
        assert!(rep.complete);
    }

    #[test]
    fn remove_reports_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let store = ContentStore::open(tmp.path().join("lib")).unwrap();
        let pack = make_pack(tmp.path(), &[("a.wav", b"aaa")]);
        store.install(&pack).unwrap();
        store.remove("test-pack", "1.0.0").unwrap();
        let rep = store
            .dependency_report(&[PackDependency {
                pack_id: "test-pack".into(),
                version: None,
            }])
            .unwrap();
        assert!(!rep.complete);
        assert_eq!(rep.missing[0].pack_id, "test-pack");
        assert_eq!(rep.missing[0].reason, FileState::Missing);
    }

    #[test]
    fn explicit_replace_is_recorded() {
        let tmp = tempfile::tempdir().unwrap();
        let store = ContentStore::open(tmp.path().join("lib")).unwrap();
        let pack = make_pack(tmp.path(), &[("a.wav", b"aaa")]);
        store.install(&pack).unwrap();
        fs::remove_file(store.pack_dir("test-pack", "1.0.0").join("a.wav")).unwrap();
        store.rescan("test-pack", "1.0.0").unwrap();
        let st = store
            .relink("test-pack", "1.0.0", "a.wav", b"different-bytes", true)
            .unwrap();
        assert_eq!(st, FileState::Replaced);
        let rec = store.load_index().unwrap().packs["test-pack"]["1.0.0"].clone();
        assert!(rec.contains_user_replacements);
        assert_eq!(rec.status, PackStatus::Degraded);
    }
}
