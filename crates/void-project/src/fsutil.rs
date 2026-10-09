//! Filesystem helpers shared across the container.

use crate::error::{ProjectError, Result};
use std::path::{Component, Path, PathBuf};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// Validate a checkpoint-internal relative path (checkpoint.schema.json
/// path pattern): relative, `/` separated, no `..`, no backslashes, no
/// drive letters, ≤500 chars, no NUL/control.
pub fn safe_rel_path(p: &str) -> Result<PathBuf> {
    if p.is_empty() || p.len() > 500 {
        return Err(ProjectError::UnsafePath(p.into()));
    }
    if p.starts_with('/') || p.contains('\\') || p.contains(':') {
        return Err(ProjectError::UnsafePath(p.into()));
    }
    if p.chars().any(|c| c.is_control()) {
        return Err(ProjectError::UnsafePath(p.into()));
    }
    let rel = Path::new(p);
    for comp in rel.components() {
        match comp {
            Component::Normal(_) | Component::CurDir => {}
            _ => return Err(ProjectError::UnsafePath(p.into())),
        }
    }
    Ok(rel.to_path_buf())
}

/// RFC 3339 UTC timestamp for manifest/meta records.
pub fn utc_now() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".into())
}
