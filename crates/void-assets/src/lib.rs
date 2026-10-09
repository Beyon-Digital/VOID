//! void-assets — immutable content-addressed media (W05, CONTRACTS.md §4).
//!
//! - `store`: SHA-256-addressed blob store with verify-before-publish writes
//! - `media`: format probing that rejects corrupt/truncated media
//! - `archive`: zip import that validates every entry before extracting
//! - `link`: missing-media placeholders + explicit (never silent) relink

mod archive;
mod error;
mod link;
mod media;
mod store;

pub use archive::{
    import_zip, ARCHIVE_MAX_ENTRIES, ARCHIVE_MAX_ENTRY_BYTES, ARCHIVE_MAX_TOTAL_BYTES,
};
pub use error::{AssetError, Result};
pub use link::{relink, resolve, LinkResolution, MediaLink, RelinkOutcome};
pub use media::{probe_wav, WavInfo};
pub use store::{
    file_sha256, hex_sha256, sync_dir, write_atomic, AssetRef, AssetStore, ASSET_IMPORT_MAX_BYTES,
};
