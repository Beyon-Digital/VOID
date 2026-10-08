//! Content-pack manifest schema + verification (W19 / SND-06, T73).
//!
//! A content pack is a directory containing `manifest.json` plus payload
//! files (samples, IRs, preset descriptors). The manifest is the trust
//! boundary: it carries `manifestSha256` — SHA-256 of the canonical
//! serialization of every other field — and a per-file SHA-256 inventory.
//! Tampering with either is detected before install publishes anything.
//!
//! This is a *checksum* scheme, not a signature: it proves the bytes on
//! disk still match the manifest the pack was built with. Signed
//! distribution is a D06-gated topic and stays out of scope.
//!
//! The manifest `deny_unknown_fields`: remote-code or execution fields
//! (`url`, `download`, `script`, `command`, ...) cannot be expressed —
//! packs are data, never instructions.

use crate::error::{ContentError, Result};
use serde::{Deserialize, Serialize};

/// Canonical JSON (sorted keys, no whitespace) — same convention as
/// `void_models::manifest` so pack manifests and model manifests hash
/// identically for shared tooling.
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

pub fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn is_u64_str(s: &str) -> bool {
    !s.is_empty() && s.parse::<u64>().is_ok()
}

/// Pack-relative payload path: no absolute, no `..`, no NUL, no backslash
/// tricks — resolution must stay inside the pack directory (T19 rules).
pub fn is_safe_pack_path(p: &str) -> bool {
    !p.is_empty()
        && p.len() <= 512
        && !p.starts_with('/')
        && !p.starts_with('\\')
        && !p.contains('\\')
        && !p.contains('\0')
        && !p
            .split('/')
            .any(|seg| seg.is_empty() || seg == "." || seg == "..")
}

/// Pack id slug: `[a-z0-9._-]+`, no leading dot — used as a directory
/// name inside the content root.
pub fn is_pack_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 120
        && !s.starts_with('.')
        && !s.contains("..")
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'))
}

/// What kind of payload a file is. Descriptors only — no executable
/// content exists in a content pack.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    /// PCM audio used by zones/one-shots.
    Sample,
    /// Impulse response for convolution.
    ImpulseResponse,
    /// Instrument/preset parameter descriptor (JSON).
    Preset,
    /// Zone/multisample instrument definition (JSON).
    Instrument,
    /// Licence/attribution text carried inside the pack.
    Rights,
    /// Loop/one-shot metadata (tempo/key tags).
    LoopMeta,
}

/// One payload file declared by the manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PackFile {
    /// Pack-relative path (`samples/kick/C1_rr1.wav`).
    pub path: String,
    /// Expected SHA-256 of the file bytes (lowercase hex).
    pub sha256: String,
    /// Declared byte size — decimal u64 string.
    pub bytes: String,
    pub kind: FileKind,
    /// Required files quarantine the pack when corrupt/missing;
    /// optional ones mark the pack `degraded` instead.
    #[serde(default = "default_required")]
    pub required: bool,
}

fn default_required() -> bool {
    true
}

/// The pack manifest document. Closed field set (`deny_unknown_fields`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentPackManifest {
    pub format_version: u32,
    /// Integrity field — excluded from its own hash.
    pub manifest_sha256: String,
    /// Slug id (`void-drums-core`).
    pub pack_id: String,
    /// Real semver (`1.2.0`, `2.0.0-rc.1`) — parsed with `semver`.
    pub version: String,
    pub name: String,
    /// Stock family this pack serves (`STOCK-03`) or `other`.
    pub family: String,
    /// License ledger entry id (docs/content-rights/rights-ledger.json).
    pub license_id: String,
    /// Max simultaneous voices this pack may hold — decimal u64 string,
    /// must be > 0 ("unbounded" is not a budget).
    pub voice_budget: String,
    /// Max bytes this pack may hold in the streaming/preload cache —
    /// decimal u64 string, must be > 0.
    pub cache_budget_bytes: String,
    pub files: Vec<PackFile>,
}

impl ContentPackManifest {
    /// Parse + verify: well-formed JSON, integrity hash matches, then
    /// field-level validation (semver, budgets, sha256, safe paths).
    pub fn parse_and_verify(bytes: &[u8]) -> Result<Self> {
        let doc: serde_json::Value = serde_json::from_slice(bytes)
            .map_err(|e| ContentError::InvalidManifest(format!("json: {e}")))?;
        let declared = doc
            .get("manifestSha256")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ContentError::InvalidManifest("manifestSha256 missing".into()))?;
        let actual = manifest_content_sha256(&doc);
        if declared != actual {
            return Err(ContentError::ManifestTampered {
                declared: declared.to_string(),
                actual,
            });
        }
        let manifest: Self = serde_json::from_value(doc)
            .map_err(|e| ContentError::InvalidManifest(format!("fields: {e}")))?;
        manifest.validate_fields()?;
        Ok(manifest)
    }

    /// Field-level validation after structural parse.
    pub fn validate_fields(&self) -> Result<()> {
        let bad = |m: &str| ContentError::InvalidManifest(m.into());
        if self.format_version != 1 {
            return Err(bad("formatVersion must be 1"));
        }
        if !is_pack_id(&self.pack_id) {
            return Err(bad("packId must be a safe slug"));
        }
        // Real semver — no free-form version strings.
        semver::Version::parse(&self.version)
            .map_err(|e| bad(&format!("version not semver: {e}")))?;
        if self.name.is_empty() || self.name.len() > 200 {
            return Err(bad("name missing/oversized"));
        }
        if self.family.is_empty() || self.family.len() > 40 {
            return Err(bad("family missing/oversized"));
        }
        if self.license_id.is_empty() || self.license_id.len() > 120 {
            return Err(bad("licenseId missing/oversized"));
        }
        if !is_u64_str(&self.voice_budget) || self.voice_budget == "0" {
            return Err(bad("voiceBudget must be a nonzero decimal u64"));
        }
        if !is_u64_str(&self.cache_budget_bytes) || self.cache_budget_bytes == "0" {
            return Err(bad("cacheBudgetBytes must be a nonzero decimal u64"));
        }
        if self.files.is_empty() {
            return Err(bad("files must not be empty"));
        }
        if self.files.len() > 4096 {
            return Err(bad("files exceeds 4096 entries"));
        }
        let mut seen = std::collections::HashSet::with_capacity(self.files.len());
        for f in &self.files {
            if !is_safe_pack_path(&f.path) {
                return Err(ContentError::UnsafePath(f.path.clone()));
            }
            if !seen.insert(f.path.clone()) {
                return Err(bad("duplicate file path in manifest"));
            }
            if !is_sha256_hex(&f.sha256) {
                return Err(bad("files[].sha256 not sha256 hex"));
            }
            if !is_u64_str(&f.bytes) {
                return Err(bad("files[].bytes not a decimal u64"));
            }
        }
        Ok(())
    }

    /// Parsed semver version (valid post-verify).
    pub fn semver(&self) -> semver::Version {
        semver::Version::parse(&self.version).expect("validated manifest version")
    }

    pub fn voice_budget(&self) -> u64 {
        self.voice_budget.parse().expect("validated budget")
    }

    pub fn cache_budget_bytes(&self) -> u64 {
        self.cache_budget_bytes.parse().expect("validated budget")
    }
}

/// Build a manifest document + its integrity field — the inverse of
/// `parse_and_verify`. Used by pack tooling and tests to produce real
/// manifests rather than hand-edited fakes.
pub fn sign_manifest(mut doc: serde_json::Value) -> serde_json::Value {
    let sha = manifest_content_sha256(&doc);
    if let Some(m) = doc.as_object_mut() {
        m.insert("manifestSha256".into(), serde_json::Value::String(sha));
    }
    doc
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc() -> serde_json::Value {
        serde_json::json!({
            "formatVersion": 1,
            "packId": "void-drums-core",
            "version": "1.0.0",
            "name": "VOID Drums Core",
            "family": "STOCK-03",
            "licenseId": "void-original",
            "voiceBudget": "32",
            "cacheBudgetBytes": "67108864",
            "files": [
                {"path": "samples/kick.wav", "sha256": void_assets::hex_sha256(b"kick"),
                 "bytes": "4", "kind": "sample", "required": true}
            ]
        })
    }

    #[test]
    fn signed_manifest_verifies() {
        let signed = sign_manifest(doc());
        let m = ContentPackManifest::parse_and_verify(
            serde_json::to_string(&signed).unwrap().as_bytes(),
        )
        .unwrap();
        assert_eq!(m.pack_id, "void-drums-core");
        assert_eq!(m.semver().major, 1);
    }

    #[test]
    fn tampered_field_rejected() {
        let mut signed = sign_manifest(doc());
        signed["name"] = serde_json::json!("renamed-after-signing");
        let err = ContentPackManifest::parse_and_verify(
            serde_json::to_string(&signed).unwrap().as_bytes(),
        )
        .unwrap_err();
        assert!(matches!(err, ContentError::ManifestTampered { .. }));
    }

    #[test]
    fn bad_semver_rejected() {
        let mut d = doc();
        d["version"] = serde_json::json!("version-one");
        let signed = sign_manifest(d);
        let err = ContentPackManifest::parse_and_verify(
            serde_json::to_string(&signed).unwrap().as_bytes(),
        )
        .unwrap_err();
        assert!(matches!(err, ContentError::InvalidManifest(_)));
    }

    #[test]
    fn traversal_path_rejected() {
        let mut d = doc();
        d["files"][0]["path"] = serde_json::json!("../../etc/passwd");
        let signed = sign_manifest(d);
        let err = ContentPackManifest::parse_and_verify(
            serde_json::to_string(&signed).unwrap().as_bytes(),
        )
        .unwrap_err();
        assert!(matches!(err, ContentError::UnsafePath(_)));
    }

    #[test]
    fn unknown_field_rejected() {
        let mut d = doc();
        d["download"] = serde_json::json!("https://evil.example/payload");
        let signed = sign_manifest(d);
        let err = ContentPackManifest::parse_and_verify(
            serde_json::to_string(&signed).unwrap().as_bytes(),
        )
        .unwrap_err();
        assert!(matches!(err, ContentError::InvalidManifest(_)));
    }

    #[test]
    fn zero_budget_rejected() {
        let mut d = doc();
        d["voiceBudget"] = serde_json::json!("0");
        let signed = sign_manifest(d);
        let err = ContentPackManifest::parse_and_verify(
            serde_json::to_string(&signed).unwrap().as_bytes(),
        )
        .unwrap_err();
        assert!(matches!(err, ContentError::InvalidManifest(_)));
    }
}
