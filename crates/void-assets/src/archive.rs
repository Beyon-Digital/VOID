//! Safe archive import (CONTRACTS.md §4, T19). Every entry is validated —
//! names, types, sizes, counts — BEFORE any byte is written. Anything
//! suspicious rejects the whole archive; partial extraction is never
//! published.

use crate::error::{AssetError, Result};
use crate::store::AssetStore;
use std::io::Read;
use std::path::Path;

/// Bounded archive-expansion limits (initial engineering policy).
pub const ARCHIVE_MAX_ENTRIES: usize = 1024;
pub const ARCHIVE_MAX_ENTRY_BYTES: u64 = 64 * 1024 * 1024; // 64 MiB per entry
pub const ARCHIVE_MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024; // 256 MiB payload

const UNIX_FILETYPE_MASK: u32 = 0o170000;
const UNIX_SYMLINK: u32 = 0o120000;

/// Validate and import every file entry of a zip archive into `store`.
/// Returns the imported asset refs. The archive is scanned twice: the whole
/// entry table must pass name/type/size checks before a single extract.
pub fn import_zip(
    store: &AssetStore,
    archive_path: impl AsRef<Path>,
) -> Result<Vec<crate::AssetRef>> {
    let file = std::fs::File::open(archive_path.as_ref())?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| AssetError::Archive(e.to_string()))?;

    let n = zip.len();
    if n > ARCHIVE_MAX_ENTRIES {
        return Err(AssetError::LimitExceeded(format!(
            "{n} entries exceeds {ARCHIVE_MAX_ENTRIES}"
        )));
    }

    // Pass 1: validate names, entry types and declared sizes.
    let mut accepted: Vec<(usize, String, u64)> = Vec::new();
    let mut total: u64 = 0;
    for i in 0..n {
        let entry = zip
            .by_index(i)
            .map_err(|e| AssetError::Archive(e.to_string()))?;
        if entry.is_dir() {
            continue;
        }
        let raw = entry.name().to_string();
        check_entry_name(&raw)?;
        if let Some(mode) = entry.unix_mode() {
            if mode & UNIX_FILETYPE_MASK == UNIX_SYMLINK {
                return Err(AssetError::UnsafeEntry(format!(
                    "symlink entry {raw:?} not allowed"
                )));
            }
        }
        let size = entry.size();
        if size > ARCHIVE_MAX_ENTRY_BYTES {
            return Err(AssetError::LimitExceeded(format!(
                "entry {raw:?} declares {size} bytes (max {ARCHIVE_MAX_ENTRY_BYTES})"
            )));
        }
        total = total.saturating_add(size);
        if total > ARCHIVE_MAX_TOTAL_BYTES {
            return Err(AssetError::LimitExceeded(format!(
                "archive expands past {ARCHIVE_MAX_TOTAL_BYTES} bytes"
            )));
        }
        accepted.push((i, raw, size));
    }

    // Pass 2: bounded extraction straight into the content-addressed store.
    let mut out = Vec::new();
    for (index, name, size) in accepted {
        let entry = zip
            .by_index(index)
            .map_err(|e| AssetError::Archive(e.to_string()))?;
        let capped = entry.take(size.min(ARCHIVE_MAX_ENTRY_BYTES) + 1);
        let ext = Path::new(&name)
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_else(|| "bin".into());
        let r = store.import_reader(capped, &ext).map_err(|e| match e {
            AssetError::Oversized(b) => {
                AssetError::LimitExceeded(format!("entry {name:?} expanded past limit ({b} bytes)"))
            }
            other => other,
        })?;
        out.push(r);
    }
    Ok(out)
}

/// Reject archive names that could escape the container or alias existing
/// files: absolute paths, `..` components, drive prefixes, backslashes,
/// control characters and names that normalize to nothing.
fn check_entry_name(raw: &str) -> Result<()> {
    if raw.is_empty() {
        return Err(AssetError::UnsafeEntry("empty entry name".into()));
    }
    if raw.starts_with('/') || raw.starts_with('\\') {
        return Err(AssetError::UnsafeEntry(format!("absolute path {raw:?}")));
    }
    if raw.contains('\\') {
        return Err(AssetError::UnsafeEntry(format!(
            "backslash separator in {raw:?}"
        )));
    }
    if raw.contains(':') {
        // Covers drive letters (`C:`) and ADS-style suffixes.
        return Err(AssetError::UnsafeEntry(format!("colon in {raw:?}")));
    }
    if raw.chars().any(|c| c.is_control()) {
        return Err(AssetError::UnsafeEntry(format!(
            "control character in {raw:?}"
        )));
    }
    for comp in raw.split('/') {
        if comp == ".." {
            return Err(AssetError::UnsafeEntry(format!(
                "parent traversal in {raw:?}"
            )));
        }
    }
    if raw.split('/').all(|c| c.is_empty() || c == ".") {
        return Err(AssetError::UnsafeEntry(format!(
            "entry {raw:?} names no file"
        )));
    }
    Ok(())
}
