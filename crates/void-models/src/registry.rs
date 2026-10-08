//! Local model registry — persisted in the app SQLite index.
//!
//! CONTRACTS §4/DATA-01: the index is *reconstructable* — it records
//! which model descriptors were installed, but the manifest + artifact
//! bytes are the authority. Rebuild the registry by re-verifying
//! manifests; never treat a row as proof a model file exists (T51's
//! "remove optional model files" case is exactly why `check_artifacts`
//! re-verifies on every resolve).
//!
//! The table lives in the SAME app index file the job store uses
//! (`JobDb::open(path)`); connections are per-crate but the file is one
//! index. `user_version` is left to the jobs schema — this crate only
//! creates its own table.

use crate::error::{ModelError, Result};
use crate::manifest::{check_artifacts, ArtifactCheck, ModelManifest};
use crate::model::{ModelDescriptor, ModelStatus};
use rusqlite::{params, Connection};
use std::path::Path;

const MODELS_TABLE_DDL: &str = "
    CREATE TABLE IF NOT EXISTS models (
        model_id TEXT NOT NULL,
        version TEXT NOT NULL,
        kind TEXT NOT NULL,
        status TEXT NOT NULL,
        name TEXT NOT NULL,
        record_json TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        PRIMARY KEY (model_id, version)
    );
";

pub struct ModelRegistry {
    conn: Connection,
}

impl ModelRegistry {
    /// Open (creating) the registry inside the app index file.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(p) = path.as_ref().parent() {
            std::fs::create_dir_all(p).map_err(ModelError::Io)?;
        }
        Self::init(Connection::open(path)?)
    }

    /// In-memory registry for tests.
    pub fn open_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.execute_batch(MODELS_TABLE_DDL)?;
        Ok(Self { conn })
    }

    fn utc_now(&self) -> String {
        self.conn
            .query_row("SELECT strftime('%Y-%m-%dT%H:%M:%fZ','now')", [], |r| {
                r.get(0)
            })
            .unwrap_or_else(|_| "1970-01-01T00:00:00.000Z".into())
    }

    /// Install a manifest: integrity + field validation, artifact
    /// verification against `store`, then persist the descriptor.
    /// Tampered/invalid manifests are rejected BEFORE any row exists —
    /// a rejected manifest never masquerades as an installed model (T51).
    pub fn install(
        &self,
        manifest_bytes: &[u8],
        store: &void_assets::AssetStore,
    ) -> Result<ModelDescriptor> {
        let manifest = ModelManifest::parse_and_verify(manifest_bytes)?;
        let (status, _checks) = check_artifacts(&manifest, store);
        if status == ModelStatus::Rejected {
            return Err(ModelError::MissingArtifact(
                "artifact hash mismatch on install".into(),
            ));
        }
        let mut desc = manifest.to_descriptor(status);
        desc.installed_at = self.utc_now();
        self.upsert(&desc)?;
        Ok(desc)
    }

    /// Persist/replace a descriptor row.
    pub fn upsert(&self, desc: &ModelDescriptor) -> Result<()> {
        self.conn.execute(
            "INSERT INTO models(model_id, version, kind, status, name, record_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(model_id, version)
             DO UPDATE SET kind=excluded.kind, status=excluded.status, name=excluded.name,
                           record_json=excluded.record_json, updated_at=excluded.updated_at",
            params![
                desc.model_id,
                desc.version,
                kind_str(desc.kind),
                desc.status.as_str(),
                desc.name,
                serde_json::to_string(desc)?,
                self.utc_now(),
            ],
        )?;
        Ok(())
    }

    /// Fetch a descriptor without re-verifying artifacts — the *indexed*
    /// state. Use `resolve` for a launch decision.
    pub fn get(&self, model_id: &str, version: &str) -> Result<ModelDescriptor> {
        let rec = self.row(model_id, version)?;
        rec.ok_or_else(|| ModelError::NotFound(format!("{model_id}@{version}")))
    }

    /// Resolve a model for launch: re-verify artifacts against the store
    /// and return the *current* status (the stored row is updated to
    /// match — files can disappear after install, T51). A runnable model
    /// returns its descriptor; anything else returns NotFound/Unavailable
    /// semantics via the status field.
    pub fn resolve(
        &self,
        model_id: &str,
        version: &str,
        store: &void_assets::AssetStore,
    ) -> Result<ModelDescriptor> {
        let mut desc = self.get(model_id, version)?;
        let (status, _checks) = self.verify_descriptor(&desc, store);
        if status != desc.status {
            desc.status = status;
            self.upsert(&desc)?;
        }
        Ok(desc)
    }

    /// Re-check one descriptor's artifacts (builds a manifest-shaped view
    /// so the same checker serves both install and re-verify).
    pub fn verify_descriptor(
        &self,
        desc: &ModelDescriptor,
        store: &void_assets::AssetStore,
    ) -> (ModelStatus, Vec<(String, ArtifactCheck)>) {
        let manifest = ModelManifest {
            format_version: 1,
            manifest_sha256: desc.manifest_sha256.clone(),
            model_id: desc.model_id.clone(),
            version: desc.version.clone(),
            name: desc.name.clone(),
            kind: desc.kind,
            runtime: desc.runtime.clone(),
            artifacts: desc.artifacts.clone(),
            capabilities: desc.capabilities.clone(),
            budgets: desc.budgets.clone(),
            license: desc.license.clone(),
            source: desc.source.clone(),
        };
        check_artifacts(&manifest, store)
    }

    /// All registered descriptors, ordered by id then version.
    pub fn list(&self) -> Result<Vec<ModelDescriptor>> {
        let mut st = self
            .conn
            .prepare("SELECT model_id, version FROM models ORDER BY model_id, version")?;
        let keys = st
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        keys.into_iter().map(|(id, v)| self.get(&id, &v)).collect()
    }

    /// Remove a model record (user deletion / retention policy, T52).
    /// Only the index row goes — artifact blobs are immutable and are
    /// pruned separately by reachability.
    pub fn remove(&self, model_id: &str, version: &str) -> Result<bool> {
        let n = self.conn.execute(
            "DELETE FROM models WHERE model_id=?1 AND version=?2",
            params![model_id, version],
        )?;
        Ok(n > 0)
    }

    /// Budget defaults for a runnable model — the runner clamps a job's
    /// request to these ceilings.
    pub fn budget_for(
        &self,
        model_id: &str,
        version: &str,
    ) -> Result<crate::model::BudgetDefaults> {
        self.get(model_id, version).map(|d| d.budgets)
    }

    fn row(&self, model_id: &str, version: &str) -> Result<Option<ModelDescriptor>> {
        let mut st = self
            .conn
            .prepare("SELECT record_json FROM models WHERE model_id=?1 AND version=?2")?;
        let mut rows = st.query(params![model_id, version])?;
        match rows.next()? {
            None => Ok(None),
            Some(r) => {
                let json: String = r.get(0)?;
                Ok(Some(serde_json::from_str(&json)?))
            }
        }
    }
}

fn kind_str(k: crate::model::ModelKind) -> &'static str {
    match k {
        crate::model::ModelKind::Symbolic => "symbolic",
        crate::model::ModelKind::Audio => "audio",
        crate::model::ModelKind::Visual => "visual",
    }
}
