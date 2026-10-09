//! Module manifest schema v1 — the declarative half of a module
//! package. A module is `manifest.json` + `module.wasm` and nothing
//! else; there is no native-code path. The manifest declares
//! identity, the wasm's sha256, capabilities, ambient requests (all
//! denied), limits and runtime placement.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::capability::{reject_ambient, Capability, Placement};
use crate::error::{Result, WasmError};
use crate::policy::{DeclaredLimits, HostPolicy};

pub const MANIFEST_VERSION: u32 = 1;
pub const MANIFEST_FILE: &str = "manifest.json";
pub const MODULE_FILE: &str = "module.wasm";
pub const WASM_MAGIC: &[u8; 4] = b"\0asm";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleManifest {
    /// Schema version; unknown newer versions are refused (write-open
    /// rule — CONTRACTS.md §4 migration stance).
    pub manifest_version: u32,
    /// Stable reverse-dns-style module id (`void.demo.saw`).
    pub module_id: String,
    /// Human display name.
    pub name: String,
    /// Semver-lite version `major.minor.patch(-label)`.
    pub version: String,
    /// sha256 (hex, lowercase) of `module.wasm`. Binds the manifest to
    /// exactly one binary — registry installs verify it.
    pub wasm_sha256: String,
    /// Declared grantable capabilities.
    pub capabilities: Vec<Capability>,
    /// Ambient powers the package *asks* for. Any entry rejects the
    /// manifest — recorded for audit, never granted.
    #[serde(default)]
    pub ambient_requests: Vec<String>,
    /// Declared budgets inside the host policy envelope.
    pub limits: DeclaredLimits,
    /// Where this module's entry points may execute. `audio_path` is
    /// not expressible — the critical path is unrepresentable.
    pub placement: Placement,
    /// ABI binding — must equal `abi::ABI_NAME`/`ABI_VERSION`.
    pub abi: String,
}

impl ModuleManifest {
    /// Parse + validate against policy. Pure function — no fs access.
    pub fn parse(json: &str, policy: &HostPolicy) -> Result<ModuleManifest> {
        let m: ModuleManifest =
            serde_json::from_str(json).map_err(|e| WasmError::InvalidManifest(e.to_string()))?;
        m.validate(policy)?;
        Ok(m)
    }

    pub fn validate(&self, policy: &HostPolicy) -> Result<()> {
        if self.manifest_version != MANIFEST_VERSION {
            return Err(WasmError::InvalidManifest(format!(
                "manifest_version {} unsupported (schema is {}, newer versions refused)",
                self.manifest_version, MANIFEST_VERSION
            )));
        }
        if self.abi != crate::abi::ABI_NAME {
            return Err(WasmError::InvalidManifest(format!(
                "abi `{}` unsupported (host binds `{}`)",
                self.abi,
                crate::abi::ABI_NAME
            )));
        }
        let id_ok = !self.module_id.is_empty()
            && self.module_id.len() <= 128
            && self
                .module_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_');
        if !id_ok {
            return Err(WasmError::InvalidManifest(format!(
                "module_id `{}` invalid (allowed [A-Za-z0-9._-], ≤128 chars)",
                self.module_id
            )));
        }
        if self.name.len() > 256 {
            return Err(WasmError::InvalidManifest("name exceeds 256 chars".into()));
        }
        if !is_valid_version(&self.version) {
            return Err(WasmError::InvalidManifest(format!(
                "version `{}` invalid (expected major.minor.patch[-label])",
                self.version
            )));
        }
        if self.wasm_sha256.len() != 64
            || !self
                .wasm_sha256
                .chars()
                .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
        {
            return Err(WasmError::InvalidManifest(
                "wasm_sha256 must be 64 lowercase hex chars".into(),
            ));
        }
        if self.capabilities.is_empty() {
            return Err(WasmError::InvalidManifest(
                "capabilities must declare at least one entry".into(),
            ));
        }
        reject_ambient(&self.ambient_requests)?;
        self.limits.validate_against(policy)?;
        Ok(())
    }

    /// Verify this manifest binds `wasm_bytes` via sha256.
    pub fn verify_wasm(&self, wasm_bytes: &[u8]) -> Result<()> {
        let actual = sha256_hex(wasm_bytes);
        if actual != self.wasm_sha256 {
            return Err(WasmError::Tampered {
                what: "module.wasm".into(),
                expected: self.wasm_sha256.clone(),
                actual,
            });
        }
        Ok(())
    }

    pub fn has_capability(&self, cap: Capability) -> bool {
        self.capabilities.contains(&cap)
    }

    /// Canonical key used in the registry index and on-disk layout.
    pub fn key(&self) -> String {
        format!("{}@{}", self.module_id, self.version)
    }
}

/// Semver-lite: `N.N.N` with optional `-label` of alphanumerics/dots/dashes.
pub fn is_valid_version(v: &str) -> bool {
    let (core, _label) = match v.split_once('-') {
        Some((c, l)) => (c, Some(l)),
        None => (v, None),
    };
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() != 3
        || parts
            .iter()
            .any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()))
    {
        return false;
    }
    if let Some(l) = _label {
        if l.is_empty()
            || !l
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
        {
            return false;
        }
    }
    true
}

/// Compare semver-lite versions for registry resolution.
/// Returns Ordering of (major,minor,patch) tuples; labels compare after.
pub fn version_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    // Semver precedence: numeric core first, then release > prerelease
    // (an absent label outranks any label), labels lexically.
    fn parts(v: &str) -> (u64, u64, u64, u8, String) {
        let (core, label) = match v.split_once('-') {
            Some((c, l)) => (c, l.to_string()),
            None => (v, String::new()),
        };
        let mut it = core.split('.').map(|p| p.parse::<u64>().unwrap_or(0));
        (
            it.next().unwrap_or(0),
            it.next().unwrap_or(0),
            it.next().unwrap_or(0),
            if label.is_empty() { 1 } else { 0 },
            label,
        )
    }
    parts(a).cmp(&parts(b))
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(bytes);
    let out = h.finalize();
    let mut s = String::with_capacity(64);
    for b in out {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_ordering() {
        assert!(version_cmp("1.0.0", "1.0.1") == std::cmp::Ordering::Less);
        assert!(version_cmp("2.0.0", "1.9.9") == std::cmp::Ordering::Greater);
        assert!(version_cmp("1.0.0-rc1", "1.0.0") == std::cmp::Ordering::Less);
        assert!(is_valid_version("0.1.0"));
        assert!(!is_valid_version("1.0"));
        assert!(!is_valid_version("1.0.0.0"));
        assert!(!is_valid_version("../etc"));
    }
}
