//! Immutable content-addressed media store (CONTRACTS.md §4).
//!
//! Blobs live at `assets/sha256/<hash>.<ext>` inside the project container.
//! Writes go through a `.incoming/` temp file that is fsynced and verified
//! before an atomic rename publishes the blob. Identical content
//! deduplicates; distinct clip identities remain distinct upstream.

use crate::error::{AssetError, Result};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{BufReader, Read, Write};
use std::path::{Path, PathBuf};

/// Default ceiling for a single imported blob (bounded resources, §2).
pub const ASSET_IMPORT_MAX_BYTES: u64 = 8 * 1024 * 1024 * 1024; // 8 GiB

const INCOMING_DIR: &str = ".incoming";

/// A verified immutable asset reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetRef {
    pub sha256: String,
    pub ext: String,
    pub bytes: u64,
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct AssetStore {
    root: PathBuf,
    max_bytes: u64,
}

impl AssetStore {
    /// Open (and create if needed) the store rooted at `<project>/assets`.
    pub fn new(root: impl Into<PathBuf>, max_bytes: u64) -> Result<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("sha256"))?;
        fs::create_dir_all(root.join(INCOMING_DIR))?;
        Ok(Self { root, max_bytes })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Import raw bytes; returns the verified reference. `ext` is a short
    /// type tag (`wav`, `aif`, `png`, ...) — sanitized to `[a-z0-9]`.
    pub fn import_bytes(&self, data: &[u8], ext: &str) -> Result<AssetRef> {
        let ext = sanitize_ext(ext)?;
        if data.len() as u64 > self.max_bytes {
            return Err(AssetError::Oversized(data.len() as u64));
        }
        let sha = hex_sha256(data);
        let incoming = self.incoming_path();
        {
            let mut f = File::create(&incoming)?;
            if let Err(e) = f.write_all(data).and_then(|_| f.sync_all()) {
                drop(f);
                let _ = fs::remove_file(&incoming);
                return Err(e.into());
            }
        }
        let r = self.publish_prepared(&incoming, &sha, &ext, data.len() as u64);
        if r.is_err() {
            let _ = fs::remove_file(&incoming);
        }
        r
    }

    /// Streaming import from a reader — large media never lands whole in
    /// memory. Reads at most `max_bytes + 1` before rejecting Oversized.
    pub fn import_reader(&self, mut r: impl Read, ext: &str) -> Result<AssetRef> {
        let ext = sanitize_ext(ext)?;
        let incoming = self.incoming_path();
        let mut hasher = Sha256::new();
        let mut total: u64 = 0;
        let mut file = File::create(&incoming)?;
        let mut buf = [0u8; 64 * 1024];
        loop {
            match r.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    total = total.saturating_add(n as u64);
                    if total > self.max_bytes {
                        drop(file);
                        let _ = fs::remove_file(&incoming);
                        return Err(AssetError::Oversized(total));
                    }
                    hasher.update(&buf[..n]);
                    if let Err(e) = file.write_all(&buf[..n]) {
                        drop(file);
                        let _ = fs::remove_file(&incoming);
                        return Err(e.into());
                    }
                }
                Err(e) => {
                    drop(file);
                    let _ = fs::remove_file(&incoming);
                    return Err(e.into());
                }
            }
        }
        if let Err(e) = file.sync_all() {
            drop(file);
            let _ = fs::remove_file(&incoming);
            return Err(e.into());
        }
        drop(file);
        let sha = format!("{:x}", hasher.finalize());
        let r = self.publish_prepared(&incoming, &sha, &ext, total);
        if r.is_err() {
            let _ = fs::remove_file(&incoming);
        }
        r
    }

    /// Import a file from disk.
    pub fn import_path(&self, path: impl AsRef<Path>, ext: &str) -> Result<AssetRef> {
        let f = File::open(path.as_ref())?;
        self.import_reader(BufReader::new(f), ext)
    }

    /// Locate a stored blob by hash (extension-agnostic).
    pub fn find(&self, sha256: &str) -> Option<PathBuf> {
        let dir = self.root.join("sha256");
        let prefix = format!("{sha256}.");
        let rd = fs::read_dir(dir).ok()?;
        for e in rd.flatten() {
            if e.file_name().to_string_lossy().starts_with(&prefix) {
                return Some(e.path());
            }
        }
        None
    }

    /// Recompute a blob's hash and confirm it matches its address.
    pub fn verify(&self, sha256: &str) -> Result<u64> {
        let path = self
            .find(sha256)
            .ok_or_else(|| AssetError::Missing(sha256.to_string()))?;
        let actual = file_sha256(&path)?;
        if actual != sha256 {
            return Err(AssetError::HashMismatch {
                expected: sha256.to_string(),
                actual,
            });
        }
        Ok(path.metadata()?.len())
    }

    /// List all stored asset hashes.
    pub fn list(&self) -> Result<Vec<String>> {
        let mut out = Vec::new();
        for e in fs::read_dir(self.root.join("sha256"))?.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if let Some(hash) = name.split('.').next() {
                out.push(hash.to_string());
            }
        }
        out.sort();
        out.dedup();
        Ok(out)
    }

    fn incoming_path(&self) -> PathBuf {
        self.root
            .join(INCOMING_DIR)
            .join(format!("{}.part", uuid::Uuid::new_v4()))
    }

    /// Move an already-flushed `.incoming` file to its content address after
    /// re-verifying the staged bytes on disk (never trust the writer).
    fn publish_prepared(
        &self,
        incoming: &Path,
        sha256: &str,
        ext: &str,
        declared_bytes: u64,
    ) -> Result<AssetRef> {
        let dest = self.root.join("sha256").join(format!("{sha256}.{ext}"));
        if dest.exists() {
            // Dedup: verify the stored copy instead of rewriting.
            self.verify(sha256)?;
            let _ = fs::remove_file(incoming);
            return Ok(AssetRef {
                sha256: sha256.to_string(),
                ext: ext.to_string(),
                bytes: declared_bytes,
                path: dest,
            });
        }
        let actual = file_sha256(incoming)?;
        if actual != sha256 {
            let _ = fs::remove_file(incoming);
            return Err(AssetError::HashMismatch {
                expected: sha256.to_string(),
                actual,
            });
        }
        let actual_len = incoming.metadata()?.len();
        if actual_len != declared_bytes {
            let _ = fs::remove_file(incoming);
            return Err(AssetError::InvalidRef(format!(
                "declared {declared_bytes} bytes but staged {actual_len}"
            )));
        }
        fs::rename(incoming, &dest)?;
        sync_dir(&self.root.join("sha256"))?;
        Ok(AssetRef {
            sha256: sha256.to_string(),
            ext: ext.to_string(),
            bytes: declared_bytes,
            path: dest,
        })
    }
}

pub fn hex_sha256(data: &[u8]) -> String {
    let mut h = Sha256::new();
    h.update(data);
    format!("{:x}", h.finalize())
}

pub fn file_sha256(path: &Path) -> Result<String> {
    let mut h = Sha256::new();
    let mut f = BufReader::new(File::open(path)?);
    let mut buf = [0u8; 64 * 1024];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(format!("{:x}", h.finalize()))
}

/// fsync a directory so the rename inside it is durable.
pub fn sync_dir(path: &Path) -> Result<()> {
    File::open(path)?.sync_all()?;
    Ok(())
}

/// Atomically write `dest` via a sibling temp file + rename + dir fsync.
pub fn write_atomic(dest: &Path, data: &[u8]) -> Result<()> {
    let tmp = dest.with_file_name(format!(
        ".{}.tmp-{}",
        dest.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".into()),
        uuid::Uuid::new_v4().simple()
    ));
    {
        let mut f = OpenOptions::new().create_new(true).write(true).open(&tmp)?;
        if let Err(e) = f.write_all(data).and_then(|_| f.sync_all()) {
            drop(f);
            let _ = fs::remove_file(&tmp);
            return Err(e.into());
        }
    }
    fs::rename(&tmp, dest)?;
    if let Some(parent) = dest.parent() {
        sync_dir(parent)?;
    }
    Ok(())
}

fn sanitize_ext(ext: &str) -> Result<String> {
    let ext = ext.trim().trim_start_matches('.').to_ascii_lowercase();
    if ext.is_empty() || ext.len() > 16 || !ext.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err(AssetError::InvalidRef(format!("bad extension {ext:?}")));
    }
    Ok(ext)
}
