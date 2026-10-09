//! Multichannel layout model (OUT-06).
//!
//! [`SpatialLayout`] covers the mono → 22.2 ladder with the *actual*
//! per-bus speaker assignments each layout implies (channel order +
//! nominal speaker positions, ITU-R BS.2051-style azimuth/elevation).
//! [`legality`] is the conversion matrix: identical layouts and
//! mono→wider replication are `Direct`; wider→narrower fold-downs are
//! legal only with a declared downmix; non-mono→wider "upmixes" are
//! `Illegal` — there is no defined image to invent, and faking one is
//! exactly the stereo-as-proxy lie T92 forbids.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Nominal speaker assignment: one bus channel bound to a physical
/// loudspeaker position. Azimuth uses the ADM convention: positive
/// anticlockwise (left = +), 0 = front centre; elevation positive up.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpeakerAssignment {
    pub speaker: Speaker,
    /// Bus channel index this speaker occupies (0-based).
    pub channel: u8,
    pub azimuth_deg: f32,
    pub elevation_deg: f32,
}

/// Loudspeaker identifiers covering every layout VOID models.
/// `M` = ear level, `U` = upper layer, `B` = lower/bottom layer,
/// numeric suffix = |azimuth| (330 = −30°). `Lfe` = LFE channels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Speaker {
    M000, // front centre
    M030, // front left  +30°
    M330, // front right −30°
    M045, // front wide left  +45°
    M315, // front wide right −45°
    M060, // front left-edge ±60° (quad L / 22.2)
    M300, // front right-edge ∓60° (quad R / 22.2)
    M090, // left side surround ±90°
    M270, // right side surround ∓90°
    M110, // left surround ±110° (5.1 Ls)
    M250, // right surround ∓110°
    M135, // left rear surround ±135°
    M225, // right rear surround ∓135°
    M180, // rear centre (22.2)
    U000, // top front centre
    U030, // top front left
    U330, // top front right
    U090, // top side left
    U270, // top side right
    U110, // top surround left ±110° (7.1.4)
    U250, // top surround right ∓110° (7.1.4)
    U135, // top rear left ±135° (22.2)
    U225, // top rear right ∓135° (22.2)
    U045, // top front left +45° (22.2 TpFL)
    U315, // top front right −45° (22.2 TpFR)
    U180, // top back centre
    T000, // top centre, directly overhead +90° (22.2 TpC)
    B000, // bottom front centre (22.2)
    B045, // bottom front left +45° (22.2)
    B315, // bottom front right −45° (22.2)
    Lfe1,
    Lfe2,
}

impl fmt::Display for Speaker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Channel-based layouts VOID can route (W26). `22.2` is the NHK
/// Super Hi-Vision reference layout — included so the legality matrix
/// and speaker assignments are honest about the full ladder, not
/// because any output path can produce it today.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum SpatialLayout {
    Mono,
    Stereo,
    /// Quadraphonic — M060/M300/M110/M250.
    Quad,
    /// 5.1 surround — L R C LFE Ls Rs.
    Surround51,
    /// 7.1 surround — L R C LFE Lss Rss Lrs Rrs.
    Surround71,
    /// 7.1.2 — the Atmos channel bed (7.1 + top front pair).
    Surround712,
    /// 7.1.4 — 7.1 + four overhead channels.
    Surround714,
    /// 9.1.4 — 9.1 (7.1 + wide pair) + four overheads.
    Surround914,
    /// 22.2 — NHK SHV reference layout, 24 channels.
    Surround222,
}

const fn ch(speaker: Speaker, channel: u8, az: f32, el: f32) -> SpeakerAssignment {
    SpeakerAssignment {
        speaker,
        channel,
        azimuth_deg: az,
        elevation_deg: el,
    }
}

use Speaker::*;
const MONO: &[SpeakerAssignment] = &[ch(M000, 0, 0.0, 0.0)];
const STEREO: &[SpeakerAssignment] = &[ch(M030, 0, 30.0, 0.0), ch(M330, 1, -30.0, 0.0)];
const QUAD: &[SpeakerAssignment] = &[
    ch(M060, 0, 60.0, 0.0),
    ch(M300, 1, -60.0, 0.0),
    ch(M110, 2, 110.0, 0.0),
    ch(M250, 3, -110.0, 0.0),
];
const SURROUND_51: &[SpeakerAssignment] = &[
    ch(M030, 0, 30.0, 0.0),
    ch(M330, 1, -30.0, 0.0),
    ch(M000, 2, 0.0, 0.0),
    ch(Lfe1, 3, 0.0, 0.0),
    ch(M110, 4, 110.0, 0.0),
    ch(M250, 5, -110.0, 0.0),
];
const SURROUND_71: &[SpeakerAssignment] = &[
    ch(M030, 0, 30.0, 0.0),
    ch(M330, 1, -30.0, 0.0),
    ch(M000, 2, 0.0, 0.0),
    ch(Lfe1, 3, 0.0, 0.0),
    ch(M090, 4, 90.0, 0.0),
    ch(M270, 5, -90.0, 0.0),
    ch(M135, 6, 135.0, 0.0),
    ch(M225, 7, -135.0, 0.0),
];
const SURROUND_712: &[SpeakerAssignment] = &[
    ch(M030, 0, 30.0, 0.0),
    ch(M330, 1, -30.0, 0.0),
    ch(M000, 2, 0.0, 0.0),
    ch(Lfe1, 3, 0.0, 0.0),
    ch(M090, 4, 90.0, 0.0),
    ch(M270, 5, -90.0, 0.0),
    ch(M135, 6, 135.0, 0.0),
    ch(M225, 7, -135.0, 0.0),
    ch(U030, 8, 30.0, 45.0),
    ch(U330, 9, -30.0, 45.0),
];
const SURROUND_714: &[SpeakerAssignment] = &[
    ch(M030, 0, 30.0, 0.0),
    ch(M330, 1, -30.0, 0.0),
    ch(M000, 2, 0.0, 0.0),
    ch(Lfe1, 3, 0.0, 0.0),
    ch(M090, 4, 90.0, 0.0),
    ch(M270, 5, -90.0, 0.0),
    ch(M135, 6, 135.0, 0.0),
    ch(M225, 7, -135.0, 0.0),
    ch(U030, 8, 30.0, 45.0),
    ch(U330, 9, -30.0, 45.0),
    ch(U110, 10, 110.0, 45.0),
    ch(U250, 11, -110.0, 45.0),
];
const SURROUND_914: &[SpeakerAssignment] = &[
    ch(M030, 0, 30.0, 0.0),
    ch(M330, 1, -30.0, 0.0),
    ch(M000, 2, 0.0, 0.0),
    ch(Lfe1, 3, 0.0, 0.0),
    ch(M045, 4, 45.0, 0.0),
    ch(M315, 5, -45.0, 0.0),
    ch(M090, 6, 90.0, 0.0),
    ch(M270, 7, -90.0, 0.0),
    ch(M135, 8, 135.0, 0.0),
    ch(M225, 9, -135.0, 0.0),
    ch(U030, 10, 30.0, 45.0),
    ch(U330, 11, -30.0, 45.0),
    ch(U110, 12, 110.0, 45.0),
    ch(U250, 13, -110.0, 45.0),
];
// 22.2 — ITU-R BS.2051 system H (BS.2493 channel order):
// 12 middle (incl. LFE1/LFE2) + 9 upper + 3 lower = 24.
const SURROUND_222: &[SpeakerAssignment] = &[
    ch(M060, 0, 60.0, 0.0),    // FL
    ch(M300, 1, -60.0, 0.0),   // FR
    ch(M000, 2, 0.0, 0.0),     // FC
    ch(Lfe1, 3, 30.0, -20.0),  // LFE1
    ch(M135, 4, 135.0, 0.0),   // BL
    ch(M225, 5, -135.0, 0.0),  // BR
    ch(M030, 6, 30.0, 0.0),    // FLc
    ch(M330, 7, -30.0, 0.0),   // FRc
    ch(M180, 8, 180.0, 0.0),   // BC
    ch(Lfe2, 9, -30.0, -20.0), // LFE2
    ch(M090, 10, 90.0, 0.0),   // SiL
    ch(M270, 11, -90.0, 0.0),  // SiR
    ch(U045, 12, 45.0, 45.0),  // TpFL
    ch(U315, 13, -45.0, 45.0), // TpFR
    ch(U000, 14, 0.0, 45.0),   // TpFC
    ch(T000, 15, 0.0, 90.0),   // TpC
    ch(U135, 16, 135.0, 45.0), // TpBL
    ch(U225, 17, -135.0, 45.0),// TpBR
    ch(U090, 18, 90.0, 45.0),  // TpSiL
    ch(U270, 19, -90.0, 45.0), // TpSiR
    ch(U180, 20, 180.0, 45.0), // TpBC
    ch(B000, 21, 0.0, -30.0),  // BtFC
    ch(B045, 22, 45.0, -30.0), // BtFL
    ch(B315, 23, -45.0, -30.0),// BtFR
];

impl SpatialLayout {
    /// Ordered channel assignments — `channel` equals position in
    /// this slice. Pure metadata; no audio is implied by this table.
    pub fn speaker_assignments(self) -> &'static [SpeakerAssignment] {
        match self {
            SpatialLayout::Mono => MONO,
            SpatialLayout::Stereo => STEREO,
            SpatialLayout::Quad => QUAD,
            SpatialLayout::Surround51 => SURROUND_51,
            SpatialLayout::Surround71 => SURROUND_71,
            SpatialLayout::Surround712 => SURROUND_712,
            SpatialLayout::Surround714 => SURROUND_714,
            SpatialLayout::Surround914 => SURROUND_914,
            SpatialLayout::Surround222 => SURROUND_222,
        }
    }

    /// Bus channel count for this layout.
    pub fn channels(self) -> u8 {
        self.speaker_assignments().len() as u8
    }

    /// True when this layout is a legal channel *bed* for an object
    /// container (OUT-07: Atmos beds are 5.1/7.1/7.1.2 — the wider
    /// immersive layouts are deliverable targets, not beds).
    pub fn is_legal_bed(self) -> bool {
        matches!(
            self,
            SpatialLayout::Surround51 | SpatialLayout::Surround71 | SpatialLayout::Surround712
        )
    }

    /// Rank on the width/height ladder for fold-down comparisons.
    /// Only an ordering hint — legality is decided by the matrix, not
    /// by rank arithmetic alone.
    fn rank(self) -> u8 {
        match self {
            SpatialLayout::Mono => 0,
            SpatialLayout::Stereo => 1,
            SpatialLayout::Quad => 2,
            SpatialLayout::Surround51 => 3,
            SpatialLayout::Surround71 => 4,
            SpatialLayout::Surround712 => 5,
            SpatialLayout::Surround714 => 6,
            SpatialLayout::Surround914 => 7,
            SpatialLayout::Surround222 => 8,
        }
    }
}

/// How a `from -> to` layout conversion is allowed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Legality {
    /// Signal fits without conversion: identical layouts, or a mono
    /// source replicating into a wider one — no information lost.
    Direct,
    /// Legal only when the fold-down is explicitly declared (the
    /// delivery/monitoring path carries a `downmix` declaration).
    RequiresDeclaredDownmix,
    /// No defined conversion — rejected; never approximated.
    Illegal,
}

/// The mono→22.2 legality matrix.
///
/// - `X -> X` is `Direct`.
/// - `Mono -> wider` is `Direct` (dual-mono / centre replication).
/// - `wider -> narrower` is `RequiresDeclaredDownmix` — fold-downs
///   are defined for every ordered pair down the ladder *except*
///   landing on `Quad` from anything but `5.1` (quad has no defined
///   mapping for layouts carrying front-wide or height channels).
/// - `non-mono -> wider` is `Illegal` — upmixing invents spatial
///   information; a stereo export labelled "5.1" is exactly the
///   proxy-pass T92 forbids.
pub fn legality(from: SpatialLayout, to: SpatialLayout) -> Legality {
    use SpatialLayout::*;
    if from == to {
        return Legality::Direct;
    }
    if from == Mono {
        return Legality::Direct;
    }
    if from.rank() < to.rank() {
        return Legality::Illegal;
    }
    if to == Quad && from != Surround51 {
        return Legality::Illegal;
    }
    Legality::RequiresDeclaredDownmix
}

impl fmt::Display for SpatialLayout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            SpatialLayout::Mono => "mono",
            SpatialLayout::Stereo => "stereo",
            SpatialLayout::Quad => "quad",
            SpatialLayout::Surround51 => "5.1",
            SpatialLayout::Surround71 => "7.1",
            SpatialLayout::Surround712 => "7.1.2",
            SpatialLayout::Surround714 => "7.1.4",
            SpatialLayout::Surround914 => "9.1.4",
            SpatialLayout::Surround222 => "22.2",
        };
        write!(f, "{s}")
    }
}
