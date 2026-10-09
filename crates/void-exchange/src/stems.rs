//! Stems manifest interchange (W20).
//!
//! A stems bundle is a directory of audio files plus a JSON manifest —
//! `void-stems/1` — describing each stem's source track, file, hash,
//! length and the tempo map that rendered it. It is the honest subset of
//! "exporting a session to stems": payloads are referenced by relative
//! path + sha256, never embedded in the manifest, and the round trip is
//! manifest↔manifest plus byte-verified payloads.
//!
//! Determinism: `to_json` is pretty-serde on sorted entries; the manifest
//! contains no timestamps or absolute paths.

use crate::document::{ticks_serde, LoopRange, TempoPoint};
use crate::error::{ExchangeError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub const FORMAT: &str = "void-stems/1";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StemRef {
    /// Source track id in the exporting document.
    pub track_id: String,
    /// Source track display name (copied for readability).
    pub track_name: String,
    /// Relative path inside the bundle, e.g. `stems/drums.wav`.
    /// Validated safe (no absolute, no `..`, utf8-representable).
    pub rel_path: String,
    /// Container/codec, e.g. "audio/wav". Declared, not sniffed.
    pub media_type: String,
    /// lowercase-hex sha256 of the payload bytes.
    pub sha256: String,
    /// Byte count, decimal string (string-int64 DTO rule).
    #[serde(with = "crate::document::u64_serde")]
    pub bytes: u64,
    /// Rendered length in ticks at the session tempo map.
    #[serde(with = "ticks_serde")]
    pub length_ticks: i64,
    /// Rendered length in seconds — duplicated for non-VOID readers.
    pub length_seconds: f64,
    pub sample_rate: u32,
    pub channels: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StemsManifest {
    pub format: String,
    /// Name of the exported session.
    pub session_name: String,
    /// Tempo map at render time — consumers can derive bar positions.
    #[serde(default)]
    pub tempo_map: Vec<TempoPoint>,
    /// Loop range rendered, if the render was a loop bounce.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rendered_range: Option<LoopRange>,
    /// Sorted by track_id on emit.
    pub stems: Vec<StemRef>,
    /// Loss report canonical JSON of the render, embedded so the bundle is
    /// self-describing (stems are approximations by nature).
    #[serde(default)]
    pub render_loss_json: Option<String>,
}

impl StemsManifest {
    pub fn new(session_name: impl Into<String>) -> Self {
        Self {
            format: FORMAT.to_string(),
            session_name: session_name.into(),
            tempo_map: Vec::new(),
            rendered_range: None,
            stems: Vec::new(),
            render_loss_json: None,
        }
    }

    /// Canonical JSON: stems sorted by (trackId, relPath) first.
    pub fn to_json(&self) -> Result<Vec<u8>> {
        let mut m = self.clone();
        m.stems
            .sort_by(|a, b| (&a.track_id, &a.rel_path).cmp(&(&b.track_id, &b.rel_path)));
        Ok(serde_json::to_vec_pretty(&m)?)
    }

    pub fn parse(bytes: &[u8]) -> Result<Self> {
        let m: StemsManifest = serde_json::from_slice(bytes)
            .map_err(|e| ExchangeError::Malformed(format!("stems manifest json: {e}")))?;
        if m.format != FORMAT {
            return Err(ExchangeError::Unsupported(format!(
                "stems manifest format {:?} (expected {FORMAT:?})",
                m.format
            )));
        }
        m.validate()?;
        Ok(m)
    }

    pub fn validate(&self) -> Result<()> {
        let mut seen = std::collections::BTreeSet::new();
        for s in &self.stems {
            check_rel_path(&s.rel_path)?;
            if !s
                .sha256
                .chars()
                .all(|c| c.is_ascii_hexdigit() && (c.is_ascii_lowercase() || c.is_ascii_digit()))
                || s.sha256.len() != 64
            {
                return Err(ExchangeError::Invalid(format!(
                    "stem {:?} sha256 not lowercase-hex",
                    s.rel_path
                )));
            }
            if !s.length_seconds.is_finite() || s.length_seconds < 0.0 {
                return Err(ExchangeError::Invalid(format!(
                    "stem {:?} lengthSeconds",
                    s.rel_path
                )));
            }
            if !seen.insert((&s.track_id, &s.rel_path)) {
                return Err(ExchangeError::Invalid(format!(
                    "duplicate stem {}/{}",
                    s.track_id, s.rel_path
                )));
            }
        }
        Ok(())
    }
}

/// Reject absolute/parent-escaping paths — bundles unzip on real machines.
fn check_rel_path(p: &str) -> Result<()> {
    if p.is_empty() || p.starts_with('/') || p.starts_with('\\') || p.contains(':') {
        return Err(ExchangeError::UnsafePath(p.to_string()));
    }
    if p.split('/').any(|seg| seg == "..") || p.contains('\\') {
        return Err(ExchangeError::UnsafePath(p.to_string()));
    }
    Ok(())
}

/// Verify every stem payload under `dir` against the manifest.
/// Returns the list of relative paths verified.
pub fn verify_payloads(dir: &Path, manifest: &StemsManifest) -> Result<Vec<String>> {
    let mut ok = Vec::with_capacity(manifest.stems.len());
    for s in &manifest.stems {
        let path = dir.join(&s.rel_path);
        let data = std::fs::read(&path).map_err(ExchangeError::Io)?;
        if data.len() as u64 != s.bytes {
            return Err(ExchangeError::DigestMismatch(format!(
                "{}: {} bytes, manifest says {}",
                s.rel_path,
                data.len(),
                s.bytes
            )));
        }
        let hex = {
            let mut h = Sha256::new();
            h.update(&data);
            let d = h.finalize();
            crate::preserve::encode_hex(&d)
        };
        if hex != s.sha256 {
            return Err(ExchangeError::DigestMismatch(format!(
                "{}: sha256 mismatch",
                s.rel_path
            )));
        }
        ok.push(s.rel_path.clone());
    }
    Ok(ok)
}

/// Stage a manifest + payloads into `dir`: writes `manifest.json` and
/// verifies each staged payload hashes correctly. Caller owns payload
/// bytes — this function just verifies + places them deterministically.
pub fn stage_bundle(
    dir: &Path,
    manifest: &StemsManifest,
    payloads: &[(String, Vec<u8>)],
) -> Result<()> {
    manifest.validate()?;
    std::fs::create_dir_all(dir).map_err(ExchangeError::Io)?;
    for s in &manifest.stems {
        let Some((_, bytes)) = payloads.iter().find(|(p, _)| *p == s.rel_path) else {
            return Err(ExchangeError::NotFound(format!(
                "payload for {:?}",
                s.rel_path
            )));
        };
        let path = dir.join(&s.rel_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(ExchangeError::Io)?;
        }
        std::fs::write(&path, bytes).map_err(ExchangeError::Io)?;
    }
    let verified = verify_payloads(dir, manifest)?;
    if verified.len() != manifest.stems.len() {
        return Err(ExchangeError::Invalid("staged payloads incomplete".into()));
    }
    let mpath: PathBuf = dir.join("manifest.json");
    std::fs::write(&mpath, manifest.to_json()?).map_err(ExchangeError::Io)?;
    Ok(())
}

/// Load a bundle: parse manifest.json + verify every payload.
pub fn load_bundle(dir: &Path) -> Result<StemsManifest> {
    let bytes = std::fs::read(dir.join("manifest.json")).map_err(ExchangeError::Io)?;
    let m = StemsManifest::parse(&bytes)?;
    verify_payloads(dir, &m)?;
    Ok(m)
}
