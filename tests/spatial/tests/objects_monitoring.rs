//! Object container + monitoring tests — bounded descriptors, bed
//! legality, calibration coverage, declared-fallback resolution
//! (T92/T93 model side).

use std::collections::BTreeMap;

use void_spatial::{
    can_drive, FallbackMode, MonitorUnavailable, MonitorVerdict, MonitoringConfig,
    ObjectContainerSpec, ObjectDescriptor, ObjectExtent, ObjectPosition, Speaker, SpatialLayout,
    MAX_OBJECTS, MAX_TRIM_DB,
};

fn obj(id: &str) -> ObjectDescriptor {
    ObjectDescriptor {
        object_id: id.into(),
        name: None,
        position: ObjectPosition {
            azimuth_deg: 0.0,
            elevation_deg: 0.0,
            distance: 1.0,
        },
        extent: ObjectExtent {
            width: 0.0,
            height: 0.0,
            depth: 0.0,
        },
        gain_db: 0.0,
        divergence: 0.0,
        channel_count: 1,
    }
}

fn container(bed: Option<SpatialLayout>, objects: Vec<ObjectDescriptor>) -> ObjectContainerSpec {
    ObjectContainerSpec {
        container_id: "c1".into(),
        bed,
        objects,
        sample_rate_hz: 48_000,
    }
}

#[test]
fn container_bounds_are_enforced() {
    // Legal: 7.1.2 bed + a few objects.
    let c = container(
        Some(SpatialLayout::Surround712),
        vec![obj("o1"), obj("o2")],
    );
    assert!(c.validate().is_ok());
    assert_eq!(c.implied_channels(), 10 + 2);

    // Illegal bed: stereo is not a bed.
    let c = container(Some(SpatialLayout::Stereo), vec![]);
    assert!(c.validate().is_err());

    // Over the ADM object cap.
    let many: Vec<ObjectDescriptor> = (0..=MAX_OBJECTS)
        .map(|i| obj(&format!("o{i}")))
        .collect();
    let c = container(Some(SpatialLayout::Surround51), many);
    assert!(c.validate().is_err());

    // Duplicate object ids.
    let c = container(None, vec![obj("o1"), obj("o1")]);
    assert!(c.validate().is_err());

    // Bad gain/divergence/position bounds.
    let mut bad = obj("bad");
    bad.gain_db = f32::NAN;
    let c = container(None, vec![bad]);
    assert!(c.validate().is_err());
    let mut bad = obj("bad");
    bad.gain_db = 20.0; // > +12
    assert!(container(None, vec![bad]).validate().is_err());
    let mut bad = obj("bad");
    bad.position.azimuth_deg = 200.0;
    assert!(container(None, vec![bad]).validate().is_err());
    let mut bad = obj("bad");
    bad.channel_count = 0;
    assert!(container(None, vec![bad]).validate().is_err());

    // Zero sample rate.
    let mut c = container(Some(SpatialLayout::Surround51), vec![]);
    c.sample_rate_hz = 0;
    assert!(c.validate().is_err());
}

fn full_calibration(layout: SpatialLayout) -> BTreeMap<Speaker, f32> {
    layout
        .speaker_assignments()
        .iter()
        .map(|a| (a.speaker, 0.0))
        .collect()
}

#[test]
fn monitoring_validation_is_strict() {
    // Complete 5.1 set with trims + stereo downmix fallback: valid.
    let ok = MonitoringConfig {
        speaker_set: SpatialLayout::Surround51,
        level_calibration_db: full_calibration(SpatialLayout::Surround51),
        fallback: FallbackMode::DeclaredDownmix {
            to: SpatialLayout::Stereo,
        },
        distance_m: Some(2.0),
    };
    assert!(ok.validate().is_ok());

    // Missing trim → invalid.
    let mut missing = full_calibration(SpatialLayout::Surround51);
    missing.remove(&Speaker::M110);
    let cfg = MonitoringConfig {
        speaker_set: SpatialLayout::Surround51,
        level_calibration_db: missing,
        fallback: FallbackMode::None,
        distance_m: None,
    };
    let errs = cfg.validate().unwrap_err();
    assert!(errs.iter().any(|e| e.to_string().contains("M110")));

    // Out-of-bounds trim → invalid.
    let mut hot = full_calibration(SpatialLayout::Stereo);
    hot.insert(Speaker::M030, MAX_TRIM_DB + 1.0);
    let cfg = MonitoringConfig {
        speaker_set: SpatialLayout::Stereo,
        level_calibration_db: hot,
        fallback: FallbackMode::None,
        distance_m: None,
    };
    assert!(cfg.validate().is_err());

    // Stray trim (speaker not in the set) → flagged.
    let mut stray = full_calibration(SpatialLayout::Stereo);
    stray.insert(Speaker::U000, 0.0);
    let cfg = MonitoringConfig {
        speaker_set: SpatialLayout::Stereo,
        level_calibration_db: stray,
        fallback: FallbackMode::None,
        distance_m: None,
    };
    let errs = cfg.validate().unwrap_err();
    assert!(errs.iter().any(|e| e.to_string().contains("not in the speaker set")));

    // Fallback to the same layout declares nothing → invalid.
    let cfg = MonitoringConfig {
        speaker_set: SpatialLayout::Surround51,
        level_calibration_db: full_calibration(SpatialLayout::Surround51),
        fallback: FallbackMode::DeclaredDownmix {
            to: SpatialLayout::Surround51,
        },
        distance_m: None,
    };
    assert!(cfg.validate().is_err());

    // Fallback to a WIDER layout can never be a downmix → invalid.
    let cfg = MonitoringConfig {
        speaker_set: SpatialLayout::Surround51,
        level_calibration_db: full_calibration(SpatialLayout::Surround51),
        fallback: FallbackMode::DeclaredDownmix {
            to: SpatialLayout::Surround71,
        },
        distance_m: None,
    };
    assert!(cfg.validate().is_err());
}

#[test]
fn monitor_for_resolves_routes_honestly() {
    let cfg = MonitoringConfig {
        speaker_set: SpatialLayout::Surround51,
        level_calibration_db: full_calibration(SpatialLayout::Surround51),
        fallback: FallbackMode::DeclaredDownmix {
            to: SpatialLayout::Stereo,
        },
        distance_m: None,
    };

    // Exact match is direct.
    assert_eq!(
        cfg.monitor_for(SpatialLayout::Surround51),
        MonitorVerdict::Direct
    );
    // Narrower content whose speakers exist in the set: placement,
    // not upmix — stereo L/R sit on the 5.1 fronts.
    assert_eq!(cfg.monitor_for(SpatialLayout::Stereo), MonitorVerdict::Direct);
    assert_eq!(cfg.monitor_for(SpatialLayout::Mono), MonitorVerdict::Direct);
    // Quad cannot be placed on a 5.1 set (no ±60°/±110° quad corners
    // all present? Quad = M060/M300/M110/M250; 5.1 has M110/M250 but
    // lacks M060/M300) → falls to the declared stereo fold? No —
    // quad→stereo folds via the fallback only if quad folds to stereo
    // legally (RequiresDeclaredDownmix) AND stereo drives the set.
    // quad→stereo IS a declared fold, stereo drives 5.1 → ViaFallback.
    assert_eq!(
        cfg.monitor_for(SpatialLayout::Quad),
        MonitorVerdict::ViaFallback {
            to: Some(SpatialLayout::Stereo)
        }
    );
    // 7.1.4 wider than the set → folds to declared stereo.
    assert_eq!(
        cfg.monitor_for(SpatialLayout::Surround714),
        MonitorVerdict::ViaFallback {
            to: Some(SpatialLayout::Stereo)
        }
    );
}

#[test]
fn no_fallback_means_wider_content_is_unavailable() {
    let cfg = MonitoringConfig {
        speaker_set: SpatialLayout::Stereo,
        level_calibration_db: full_calibration(SpatialLayout::Stereo),
        fallback: FallbackMode::None,
        distance_m: None,
    };
    match cfg.monitor_for(SpatialLayout::Surround51) {
        MonitorVerdict::Unavailable {
            reason: MonitorUnavailable::NoDeclaredFallback { content },
        } => assert_eq!(content, SpatialLayout::Surround51),
        other => panic!("expected NoDeclaredFallback, got {other:?}"),
    }
    // Stereo itself stays direct.
    assert_eq!(cfg.monitor_for(SpatialLayout::Stereo), MonitorVerdict::Direct);
}

#[test]
fn binaural_fallback_covers_any_content() {
    let cfg = MonitoringConfig {
        speaker_set: SpatialLayout::Stereo,
        level_calibration_db: full_calibration(SpatialLayout::Stereo),
        fallback: FallbackMode::Binaural,
        distance_m: None,
    };
    assert_eq!(
        cfg.monitor_for(SpatialLayout::Surround222),
        MonitorVerdict::ViaFallback { to: None }
    );
}

#[test]
fn can_drive_is_placement_not_upmix() {
    assert!(can_drive(SpatialLayout::Stereo, SpatialLayout::Surround51));
    assert!(can_drive(SpatialLayout::Mono, SpatialLayout::Surround222));
    assert!(!can_drive(SpatialLayout::Surround51, SpatialLayout::Stereo));
    // Quad's corner pair isn't a subset of 7.1's (7.1 uses ±90°/±135°).
    assert!(!can_drive(SpatialLayout::Quad, SpatialLayout::Surround71));
}
