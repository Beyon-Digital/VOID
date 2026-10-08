//! Channel-layout legality matrix (ENG-06).
//!
//! Every edge in the routing graph is checked at connect time and again
//! at `validate()`: **no silent channel loss**. A connection that folds
//! channels down is legal only when the send carries an explicit
//! `downmix` declaration; a connection that widens a non-mono source has
//! no defined image and is rejected outright.

use serde::{Deserialize, Serialize};

/// Typed channel layouts for mixer nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ChannelLayout {
    Mono,
    Stereo,
    /// Quadraphonic (L R Ls Rs).
    Quad,
    /// 5.1 surround (L R C LFE Ls Rs).
    Surround51,
    /// 7.1 surround (L R C LFE Ls Rs Sl Sr).
    Surround71,
}

impl ChannelLayout {
    pub fn channels(self) -> u8 {
        match self {
            ChannelLayout::Mono => 1,
            ChannelLayout::Stereo => 2,
            ChannelLayout::Quad => 4,
            ChannelLayout::Surround51 => 6,
            ChannelLayout::Surround71 => 8,
        }
    }
}

/// How a `from -> to` layout connection is allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Legality {
    /// Signal fits without conversion: identical layouts, or a mono
    /// source replicating into a wider one (dual-mono / centre image —
    /// no information is lost).
    Direct,
    /// Legal only when the send declares the fold-down explicitly
    /// (`Send::downmix`). Collapsing channels discards spatial
    /// information and must never happen silently.
    RequiresDeclaredDownmix,
    /// No defined conversion — rejected at connect/validate time.
    Illegal,
}

/// The legality matrix. Conservative rules:
///
/// - `X -> X` is always direct.
/// - `Mono -> wider` is direct (replication loses nothing).
/// - `wider -> narrower` (including `surround -> smaller surround`)
///   requires a declared downmix.
/// - `wider -> even wider` for non-mono sources is illegal: there is no
///   defined upmix image, and faking one hides channel information.
pub fn legality(from: ChannelLayout, to: ChannelLayout) -> Legality {
    use ChannelLayout::*;
    if from == to {
        return Legality::Direct;
    }
    match (from, to) {
        (Mono, _) => Legality::Direct,
        (Stereo, Mono)
        | (Quad, Mono)
        | (Quad, Stereo)
        | (Surround51, Mono)
        | (Surround51, Stereo)
        | (Surround51, Quad)
        | (Surround71, Mono)
        | (Surround71, Stereo)
        | (Surround71, Quad)
        | (Surround71, Surround51) => Legality::RequiresDeclaredDownmix,
        _ => Legality::Illegal,
    }
}
