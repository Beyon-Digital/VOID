//! `project.json` — stable project identity and container format version.

use serde::{Deserialize, Serialize};

/// Container format written by this build.
pub const FORMAT_VERSION: u32 = 1;
/// Oldest format this build can read (anything older is migrated CoW).
pub const FORMAT_READ_MIN: u32 = 0;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMeta {
    /// Present from v1; a missing field is treated as format 0.
    #[serde(default)]
    pub format_version: u32,
    pub project_id: String,
    pub name: String,
    pub created_at: String,
    #[serde(default)]
    pub generator: String,
}

impl ProjectMeta {
    pub fn new(project_id: String, name: String) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            project_id,
            name,
            created_at: crate::fsutil::utc_now(),
            generator: format!("void-project {}", env!("CARGO_PKG_VERSION")),
        }
    }
}
