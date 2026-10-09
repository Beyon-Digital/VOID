//! Opaque object identity (CONTRACTS.md §1): UUID strings, validated
//! before any engine addressing. IDs never encode relationships.

use uuid::Uuid;

/// Generate a fresh opaque ID.
pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

/// Validate that a string is a well-formed opaque ID (UUID text form).
/// Rejects empty/oversized/non-UUID strings before they reach engine calls.
pub fn is_valid_id(s: &str) -> bool {
    !s.is_empty() && s.len() <= 64 && Uuid::parse_str(s).is_ok()
}
