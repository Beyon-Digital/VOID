//! Signed-checked model manifests (W12 / T51).
//!
//! A manifest is a JSON document that *describes* a bundled model —
//! `manifest.json` inside a worker bundle. Integrity: the document
//! carries `manifestSha256` = SHA-256 of the canonical serialization of
//! every other field. Flip one byte anywhere else and verification
//! fails — this is the tamper check, not a signature scheme (signed
//! distribution is a D06-gated topic; the honest record states exactly
//! what was verified).
//!
//! The manifest type `deny_unknown_fields`: a document that tries to
//! carry remote-code or execution fields (`url`, `download`, `script`,
//! `command`, ...) fails validation outright — T51's "disallowed
//! executable/remote code" rejection.

use crate::error::{ModelError, Result};
use crate::model::{
    ArtifactRef, BudgetDefaults, CapabilityFlags, ModelDescriptor, ModelKind, ModelStatus,
    RuntimeRef,
};
use serde::{Deserialize, Serialize};

/// Canonical JSON (sorted keys, no whitespace) — deterministic hashing
/// regardless of serde's preserve_order feature.
fn canonical_json(v: &serde_json::Value, out: &mut Vec<u8>) {
    match v {
        serde_json::Value::Object(m) => {
            out.push(b'{');
            let mut keys: Vec<&String> = m.keys().collect();
            keys.sort();
            let mut first = true;
            for k in keys {
                if !first {
                    out.push(b',');
                }
                first = false;
                serde_json::to_writer(&mut *out, k).expect("key");
                out.push(b':');
                canonical_json(&m[k], out);
            }
            out.push(b'}');
        }
        serde_json::Value::Array(a) => {
            out.push(b'[');
            for (i, e) in a.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                canonical_json(e, out);
            }
            out.push(b']');
        }
        _ => serde_json::to_writer(&mut *out, v).expect("scalar"),
    }
}

/// SHA-256 of a manifest document *without* its `manifestSha256` field.
pub fn manifest_content_sha256(doc: &serde_json::Value) -> String {
    let mut sans = doc.clone();
    if let Some(m) = sans.as_object_mut() {
        m.remove("manifestSha256");
    }
    let mut buf = Vec::with_capacity(1024);
    canonical_json(&sans, &mut buf);
    void_assets::hex_sha256(&buf)
}

fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn is_u64_str(s: &str) -> bool {
    !s.is_empty() && s.parse::<u64>().is_ok()
}

/// Executable basename allowlist shape: `[A-Za-z0-9._-]+`, no leading
/// dot, no separators — resolution stays inside the bundled workers dir.
pub fn is_allowed_executable(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('.')
        && !name.contains("..")
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

/// The manifest document itself. Field set is closed
/// (`deny_unknown_fields`): remote-code or extra-execution keys are
/// structurally impossible to express.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelManifest {
    pub format_version: u32,
    /// Integrity field — excluded from its own hash.
    pub manifest_sha256: String,
    pub model_id: String,
    pub version: String,
    pub name: String,
    pub kind: ModelKind,
    pub runtime: RuntimeRef,
    #[serde(default)]
    pub artifacts: Vec<ArtifactRef>,
    #[serde(default)]
    pub capabilities: CapabilityFlags,
    pub budgets: BudgetDefaults,
    pub license: String,
    pub source: String,
}

impl ModelManifest {
    /// Parse + verify a manifest document: well-formed JSON, integrity
    /// hash matches, declared hash fields are real sha256, executable is
    /// an allowlisted basename, budgets are parseable decimal u64s and
    /// carry a real cpu/memory cap.
    pub fn parse_and_verify(bytes: &[u8]) -> Result<Self> {
        let doc: serde_json::Value = serde_json::from_slice(bytes)
            .map_err(|e| ModelError::InvalidManifest(format!("json: {e}")))?;
        let declared = doc
            .get("manifestSha256")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ModelError::InvalidManifest("manifestSha256 missing".into()))?;
        let actual = manifest_content_sha256(&doc);
        if declared != actual {
            return Err(ModelError::ManifestTampered {
                declared: declared.to_string(),
                actual,
            });
        }
        let manifest: Self = serde_json::from_value(doc)
            .map_err(|e| ModelError::InvalidManifest(format!("fields: {e}")))?;
        manifest.validate_fields()?;
        Ok(manifest)
    }

    /// Field-level validation after structural parse.
    pub fn validate_fields(&self) -> Result<()> {
        let bad = |m: &str| ModelError::InvalidManifest(m.into());
        if self.format_version != 1 {
            return Err(bad("formatVersion must be 1"));
        }
        if self.model_id.is_empty() || self.model_id.len() > 200 {
            return Err(bad("modelId missing/oversized"));
        }
        if self.version.is_empty() || self.version.len() > 64 {
            return Err(bad("version missing/oversized"));
        }
        if self.name.is_empty() || self.name.len() > 200 {
            return Err(bad("name missing/oversized"));
        }
        if !is_allowed_executable(&self.runtime.executable) {
            return Err(ModelError::DisallowedCapability(format!(
                "executable {:?} is not an allowlisted basename",
                self.runtime.executable
            )));
        }
        if self.runtime.worker_protocol != 1 {
            return Err(bad("workerProtocol must be 1"));
        }
        if let Some(h) = &self.runtime.executable_sha256 {
            if !is_sha256_hex(h) {
                return Err(bad("executableSha256 not sha256 hex"));
            }
        }
        for f in [
            &self.budgets.cpu_seconds,
            &self.budgets.memory_bytes,
            &self.budgets.vram_bytes,
            &self.budgets.wall_ns,
            &self.capabilities.max_input_bytes,
        ] {
            if !is_u64_str(f) {
                return Err(bad("budget fields must be decimal u64 strings"));
            }
        }
        // T50: a qualified model must declare real cpu + memory caps —
        // "unbounded" is not a budget.
        if self.budgets.cpu_seconds == "0" {
            return Err(bad("cpuSeconds 0 is not a real cpu budget"));
        }
        if self.budgets.memory_bytes == "0" {
            return Err(bad("memoryBytes 0 is not a real memory budget"));
        }
        if self.capabilities.task_kinds.len() > 32 || self.capabilities.hardware.len() > 16 {
            return Err(bad("capability lists oversized"));
        }
        if self.artifacts.len() > 64 {
            return Err(bad("artifacts exceeds 64 entries"));
        }
        for a in &self.artifacts {
            if !is_sha256_hex(&a.sha256) {
                return Err(bad("artifacts[].sha256 not sha256 hex"));
            }
            if !is_u64_str(&a.bytes) {
                return Err(bad("artifacts[].bytes not a decimal u64"));
            }
        }
        Ok(())
    }

    /// Descriptor this manifest installs as (status resolved separately).
    pub fn to_descriptor(&self, status: ModelStatus) -> ModelDescriptor {
        ModelDescriptor {
            model_id: self.model_id.clone(),
            version: self.version.clone(),
            name: self.name.clone(),
            kind: self.kind,
            runtime: self.runtime.clone(),
            artifacts: self.artifacts.clone(),
            capabilities: self.capabilities.clone(),
            budgets: self.budgets.clone(),
            license: self.license.clone(),
            source: self.source.clone(),
            status,
            manifest_sha256: self.manifest_sha256.clone(),
            installed_at: String::new(),
        }
    }
}

/// Per-artifact verdict after checking the content store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactCheck {
    Verified,
    Missing,
    Corrupt,
}

/// Check every manifest artifact against the asset store. Returns the
/// status the model should carry (`available` when all required verify,
/// `degraded` when only optionals are missing, `missing` when a required
/// artifact is absent, `rejected` on any hash mismatch).
pub fn check_artifacts(
    manifest: &ModelManifest,
    store: &void_assets::AssetStore,
) -> (ModelStatus, Vec<(String, ArtifactCheck)>) {
    let mut checks = Vec::with_capacity(manifest.artifacts.len());
    let mut status = ModelStatus::Available;
    for a in &manifest.artifacts {
        let check = match store.find(&a.sha256) {
            None => ArtifactCheck::Missing,
            Some(_) => match store.verify(&a.sha256) {
                Ok(_) => ArtifactCheck::Verified,
                Err(_) => ArtifactCheck::Corrupt,
            },
        };
        match (check, a.required) {
            (ArtifactCheck::Corrupt, _) => status = ModelStatus::Rejected,
            (ArtifactCheck::Missing, true) if status != ModelStatus::Rejected => {
                status = ModelStatus::Missing
            }
            (ArtifactCheck::Missing, false) if status == ModelStatus::Available => {
                status = ModelStatus::Degraded
            }
            _ => {}
        }
        checks.push((a.sha256.clone(), check));
    }
    (status, checks)
}

/// Build a manifest document + its integrity field — the inverse of
/// parse_and_verify. Used by bundle tooling and tests to produce real
/// manifests rather than hand-edited fakes.
pub fn sign_manifest(mut doc: serde_json::Value) -> serde_json::Value {
    let sha = manifest_content_sha256(&doc);
    if let Some(m) = doc.as_object_mut() {
        m.insert("manifestSha256".into(), serde_json::Value::String(sha));
    }
    doc
}
