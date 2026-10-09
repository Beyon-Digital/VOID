//! Media link state (CONTRACTS.md §4, T19): a missing asset resolves to a
//! visible relink placeholder — never to silently substituted audio.

use crate::error::{AssetError, Result};
use crate::store::{hex_sha256, AssetStore};
use serde::{Deserialize, Serialize};

/// How a project references a media blob.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum MediaLink {
    /// Verified content address.
    Present { sha256: String },
    /// Asset expected but absent — rendered as a relink placeholder.
    Missing {
        expected_sha256: String,
        display_name: String,
        detail: String,
    },
}

/// Result of resolving a link against the store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkResolution {
    Found(std::path::PathBuf),
    /// Placeholder metadata the UI must surface (no replacement audio).
    Placeholder {
        expected_sha256: String,
        display_name: String,
        detail: String,
    },
}

pub fn resolve(link: &MediaLink, store: &AssetStore) -> LinkResolution {
    match link {
        MediaLink::Present { sha256 } => match store.find(sha256) {
            Some(p) => LinkResolution::Found(p),
            None => LinkResolution::Placeholder {
                expected_sha256: sha256.clone(),
                display_name: format!("{sha256} (content address)"),
                detail: "stored blob missing from assets/sha256".into(),
            },
        },
        MediaLink::Missing {
            expected_sha256,
            display_name,
            detail,
        } => LinkResolution::Placeholder {
            expected_sha256: expected_sha256.clone(),
            display_name: display_name.clone(),
            detail: detail.clone(),
        },
    }
}

/// Outcome of a relink attempt — recorded, never silent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RelinkOutcome {
    /// Candidate bytes hashed to exactly the expected address.
    Relinked { sha256: String },
    /// User explicitly replaced the media; the previous expectation is
    /// carried for provenance.
    Replaced {
        was_expected: String,
        new_sha256: String,
    },
}

/// Attempt to relink a missing asset with candidate bytes.
/// `explicit_replace` must be true to accept content whose hash differs
/// from the expected address (the user's explicit replacement choice).
pub fn relink(
    link: &MediaLink,
    store: &AssetStore,
    candidate: &[u8],
    ext: &str,
    explicit_replace: bool,
) -> Result<(MediaLink, RelinkOutcome)> {
    let MediaLink::Missing {
        expected_sha256, ..
    } = link
    else {
        return Err(AssetError::InvalidRef("asset is not missing".into()));
    };
    let actual = hex_sha256(candidate);
    if &actual == expected_sha256 {
        let r = store.import_bytes(candidate, ext)?;
        return Ok((
            MediaLink::Present {
                sha256: r.sha256.clone(),
            },
            RelinkOutcome::Relinked { sha256: r.sha256 },
        ));
    }
    if !explicit_replace {
        return Err(AssetError::HashMismatch {
            expected: expected_sha256.clone(),
            actual,
        });
    }
    let r = store.import_bytes(candidate, ext)?;
    Ok((
        MediaLink::Present {
            sha256: r.sha256.clone(),
        },
        RelinkOutcome::Replaced {
            was_expected: expected_sha256.clone(),
            new_sha256: r.sha256,
        },
    ))
}
