//! Export provenance record (CONTRACTS.md §6/§7).
//!
//! Written inside the staged output *before* publish so it travels with
//! the artifact atomically: params, immutable checkpoint input, artifact
//! SHA-256 and tool versions. Large ints are decimal strings per §1.

use crate::spec::ExportSpec;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactRecord {
    /// Scoped file name relative to the export dir.
    pub file: String,
    pub sha256: String,
    /// Decimal string per §1.
    pub bytes: String,
    /// Verified PCM frames for wav; absent for midi.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frames: Option<String>,
    /// Declared plan the verifier required (range/tail/total).
    pub declared_total_frames: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRecord {
    /// Renderer identity (e.g. "argv", "engine-offline").
    pub renderer: String,
    /// Executable base name only — never an absolute path (safe-message rule).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable: Option<String>,
    /// SHA-256 of the exact argv vector (join of argv entries with \x1f).
    /// Proves which invocation ran without embedding filesystem paths.
    pub argv_sha256: String,
    /// ("name","version") pairs the renderer reported.
    #[serde(default)]
    pub versions: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportProvenance {
    pub format_version: u32,
    pub job_id: String,
    pub project_id: String,
    /// Immutable inputs — this is the "fixed snapshot" evidence of T42.
    pub checkpoint_id: String,
    pub source_revision: String,
    #[serde(default)]
    pub asset_hashes: Vec<String>,
    /// The full spec echo — every render parameter is recorded verbatim.
    pub spec: ExportSpec,
    /// Explicit frame plan the verifier enforced.
    pub frame_plan: crate::spec::FramePlan,
    pub artifact: ArtifactRecord,
    pub tool: ToolRecord,
    /// RFC 3339 UTC.
    pub created_at: String,
    pub result: String,
}
