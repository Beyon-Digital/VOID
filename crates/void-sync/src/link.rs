//! Ableton Link — deliberately a marker, not an implementation.
//!
//! VOID does not ship a Link peer in this lane: Link requires a UDP
//! discovery/quantum-sync stack that cannot be honestly verified on
//! this box. The decision + rationale live in
//! `docs/show/ADR-LINK.md`; this module exists so `SyncSource::LinkPeer`
//! is typed and the arbiter can *reject* a Link master claim honestly
//! instead of pretending to support it.

/// Claims against Link are typed-unavailable until a peer stack is
/// implemented — see docs/show/ADR-LINK.md + NEEDS-LINK.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkAvailability {
    /// Link is not implemented in this build — honest unavailability.
    NotImplemented,
}
