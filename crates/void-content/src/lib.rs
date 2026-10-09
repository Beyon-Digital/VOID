//! void-content — stock content library model + lifecycle (W19).
//!
//! - `manifest`: content-pack manifest schema + sha256/semver verification
//! - `zones`: key/velocity zone model, layers, deterministic round-robin
//! - `streaming`: preload/stream/offline descriptors, voice + cache budgets
//! - `lifecycle`: install → verify → quarantine → relink → remove, plus
//!   the explicit missing-content report dependent projects get (T73)
//!
//! This is the *policy/inventory* layer — sample playback, streaming IO
//! and DSP renderers are engine-resident (docs/content-rights/NEEDS.md).

mod error;
mod inventory;
mod lifecycle;
mod manifest;
mod streaming;
mod zones;

pub use error::{ContentError, Result};
pub use inventory::{
    Distribution, EntryKind, EntryStatus, LicenseLedger, LicenseRow, LicenseStatus, Rights, Source,
    StockEntry, StockInventory, INVENTORY_SCHEMA, KNOWN_STOCK_IDS,
};
pub use lifecycle::{
    ContentStore, FileRecord, FileState, InstalledPack, LibraryIndex, MissingContent,
    MissingContentReport, PackDependency, PackStatus,
};
pub use manifest::{
    is_pack_id, is_safe_pack_path, is_sha256_hex, manifest_content_sha256, sign_manifest,
    ContentPackManifest, FileKind, PackFile,
};
pub use streaming::{
    StreamingDescriptor, StreamingMode, StreamingPlan, Voice, VoiceAllocator, VoiceOutcome,
    VoiceSteal,
};
pub use zones::{LayerSelector, LoopMode, Multisample, RoundRobin, Zone};
