//! Layout model tests — channel counts, the canonical 22.2 order,
//! and the legality matrix (T92 model side).

use void_spatial::{legality, Legality, Speaker, SpatialLayout};

#[test]
fn channel_counts_match_the_ladder() {
    let cases = [
        (SpatialLayout::Mono, 1),
        (SpatialLayout::Stereo, 2),
        (SpatialLayout::Quad, 4),
        (SpatialLayout::Surround51, 6),
        (SpatialLayout::Surround71, 8),
        (SpatialLayout::Surround712, 10),
        (SpatialLayout::Surround714, 12),
        (SpatialLayout::Surround914, 14),
        (SpatialLayout::Surround222, 24),
    ];
    for (layout, n) in cases {
        assert_eq!(layout.channels(), n, "{layout}");
        let asg = layout.speaker_assignments();
        assert_eq!(asg.len(), n as usize);
        // Channel indices are dense and ordered.
        for (i, a) in asg.iter().enumerate() {
            assert_eq!(a.channel as usize, i, "{layout} channel {i}");
        }
    }
}

#[test]
fn surround_222_is_the_canonical_bs2493_order() {
    let asg = SpatialLayout::Surround222.speaker_assignments();
    let speakers: Vec<Speaker> = asg.iter().map(|a| a.speaker).collect();
    // ITU BS.2493 Table 1: FL FR FC LFE1 BL BR FLc FRc BC LFE2 SiL
    // SiR TpFL TpFR TpFC TpC TpBL TpBR TpSiL TpSiR TpBC BtFC BtFL BtFR.
    let expected = [
        Speaker::M060,
        Speaker::M300,
        Speaker::M000,
        Speaker::Lfe1,
        Speaker::M135,
        Speaker::M225,
        Speaker::M030,
        Speaker::M330,
        Speaker::M180,
        Speaker::Lfe2,
        Speaker::M090,
        Speaker::M270,
        Speaker::U045,
        Speaker::U315,
        Speaker::U000,
        Speaker::T000,
        Speaker::U135,
        Speaker::U225,
        Speaker::U090,
        Speaker::U270,
        Speaker::U180,
        Speaker::B000,
        Speaker::B045,
        Speaker::B315,
    ];
    assert_eq!(speakers.as_slice(), &expected);
    // Positions spot-checked: TpC overhead, LFE1 at +30/-20°.
    assert_eq!(asg[15].elevation_deg, 90.0);
    assert_eq!(asg[3].speaker, Speaker::Lfe1);
}

#[test]
fn legality_matrix() {
    use SpatialLayout::*;
    // Identity and mono replication are direct.
    for l in [
        Mono,
        Stereo,
        Quad,
        Surround51,
        Surround71,
        Surround712,
        Surround714,
        Surround914,
        Surround222,
    ] {
        assert_eq!(legality(l, l), Legality::Direct);
        assert_eq!(legality(Mono, l), Legality::Direct);
    }
    // Upmixes are illegal — never approximated.
    assert_eq!(legality(Stereo, Surround51), Legality::Illegal);
    assert_eq!(legality(Stereo, Quad), Legality::Illegal);
    assert_eq!(legality(Quad, Surround51), Legality::Illegal);
    assert_eq!(legality(Surround51, Surround71), Legality::Illegal);
    assert_eq!(legality(Surround712, Surround222), Legality::Illegal);
    // Fold-downs require a declared downmix.
    assert_eq!(legality(Surround222, Surround51), Legality::RequiresDeclaredDownmix);
    assert_eq!(legality(Surround714, Stereo), Legality::RequiresDeclaredDownmix);
    assert_eq!(legality(Surround51, Stereo), Legality::RequiresDeclaredDownmix);
    // Quad is special-cased: reachable only from 5.1.
    assert_eq!(legality(Surround51, Quad), Legality::RequiresDeclaredDownmix);
    assert_eq!(legality(Surround71, Quad), Legality::Illegal);
    assert_eq!(legality(Stereo, Mono), Legality::RequiresDeclaredDownmix);
}

#[test]
fn legal_beds_are_atmos_beds() {
    assert!(SpatialLayout::Surround51.is_legal_bed());
    assert!(SpatialLayout::Surround71.is_legal_bed());
    assert!(SpatialLayout::Surround712.is_legal_bed());
    for l in [
        SpatialLayout::Mono,
        SpatialLayout::Stereo,
        SpatialLayout::Quad,
        SpatialLayout::Surround714,
        SpatialLayout::Surround914,
        SpatialLayout::Surround222,
    ] {
        assert!(!l.is_legal_bed(), "{l}");
    }
}
