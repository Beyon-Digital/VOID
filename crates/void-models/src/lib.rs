//! void-models — model registry (W12, CONTRACTS.md §6).
//!
//! - `model`: descriptor records — id/version/kind/runtime, artifact
//!   SHA-256 refs into the content-addressed asset store, capability
//!   flags and per-model budget ceilings.
//! - `manifest`: integrity-checked manifest documents. `manifestSha256`
//!   covers the canonical document; tampering anywhere else breaks
//!   verification (T51). The closed field set makes remote-code or
//!   extra-execution keys structurally impossible.
//! - `registry`: local persistence inside the app SQLite index —
//!   reconstructable, never the authority over files. `resolve`
//!   re-verifies artifacts on every launch decision so removed model
//!   files surface as `missing`/`degraded`, not a stale "available".
//!
//! No network installs exist here by contract: manifests and artifacts
//! arrive via the bundled content pipeline; paid-cloud/download paths
//! are not first-song scope (D06/D07).

mod error;
mod manifest;
mod model;
mod registry;

#[cfg(test)]
mod tests;

pub use error::{ModelError, Result};
pub use manifest::{
    check_artifacts, is_allowed_executable, manifest_content_sha256, sign_manifest, ArtifactCheck,
    ModelManifest,
};
pub use model::{
    ArtifactRef, BudgetDefaults, CapabilityFlags, ModelDescriptor, ModelKind, ModelStatus,
    RuntimeKind, RuntimeRef,
};
pub use registry::ModelRegistry;
