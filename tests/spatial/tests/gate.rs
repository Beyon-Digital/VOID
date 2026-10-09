//! SpatialGate tests — every output path is Available-with-record or
//! Unavailable-with-typed-reason (T92/T93 core evidence).

use std::collections::BTreeMap;

use void_spatial::{
    FallbackMode, GateEnvironment, GateVerdict, MonitoringConfig, ObjectContainerSpec,
    ObjectDescriptor, ObjectExtent, ObjectPosition, Speaker, SpatialGate, SpatialLayout,
    SpatialOutputPath, SpatialUnavailable, ValidatorKind, ValidatorRecord,
};

fn cal(layout: SpatialLayout) -> BTreeMap<Speaker, f32> {
    layout
        .speaker_assignments()
        .iter()
        .map(|a| (a.speaker, 0.0))
        .collect()
}

fn stereo_monitor() -> MonitoringConfig {
    MonitoringConfig {
        speaker_set: SpatialLayout::Stereo,
        level_calibration_db: cal(SpatialLayout::Stereo),
        fallback: FallbackMode::None,
        distance_m: None,
    }
}

fn env() -> GateEnvironment {
    GateEnvironment {
        monitoring: Some(stereo_monitor()),
        validators: vec![SpatialGate::layout_selfcheck()],
        head_tracking_declared: false,
        platform: "linux-x86_64".into(),
    }
}

fn object() -> ObjectDescriptor {
    ObjectDescriptor {
        object_id: "o1".into(),
        name: None,
        position: ObjectPosition {
            azimuth_deg: 30.0,
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

fn container(bed: SpatialLayout) -> ObjectContainerSpec {
    ObjectContainerSpec {
        container_id: "c1".into(),
        bed: Some(bed),
        objects: vec![object()],
        sample_rate_hz: 48_000,
    }
}

fn reason(v: &GateVerdict) -> &SpatialUnavailable {
    match v {
        GateVerdict::Unavailable { reason } => reason,
        other => panic!("expected Unavailable, got {other:?}"),
    }
}

#[test]
fn channel_bus_available_only_with_validator_and_monitor() {
    let gate = SpatialGate::new(env());
    // Stereo bus: selfcheck covers channelBus:stereo, monitor drives it.
    match gate.check(&SpatialOutputPath::ChannelBus {
        layout: SpatialLayout::Stereo,
    }) {
        GateVerdict::Available {
            validator,
            monitor_route,
        } => {
            assert_eq!(validator.kind, ValidatorKind::SelfCheck);
            assert!(validator.scopes.contains(&"channelBus:stereo".to_string()));
            assert_eq!(monitor_route, "direct:stereo");
        }
        other => panic!("expected Available, got {other:?}"),
    }
}

#[test]
fn unavailable_paths_have_typed_reasons() {
    // No monitoring config at all.
    let mut e = env();
    e.monitoring = None;
    let gate = SpatialGate::new(e);
    assert!(matches!(
        reason(&gate.check(&SpatialOutputPath::ChannelBus {
            layout: SpatialLayout::Stereo
        })),
        SpatialUnavailable::NoMonitoringConfig
    ));

    // 5.1 bus on a stereo monitor with no fallback → MonitorUnreachable.
    let gate = SpatialGate::new(env());
    assert!(matches!(
        reason(&gate.check(&SpatialOutputPath::ChannelBus {
            layout: SpatialLayout::Surround51
        })),
        SpatialUnavailable::MonitorUnreachable { .. }
    ));

    // No validator covering a scope at all → ApprovedValidatorRequired.
    let mut e = env();
    e.monitoring = Some(MonitoringConfig {
        speaker_set: SpatialLayout::Surround222,
        level_calibration_db: cal(SpatialLayout::Surround222),
        fallback: FallbackMode::None,
        distance_m: None,
    });
    e.validators.clear();
    let gate = SpatialGate::new(e);
    assert!(matches!(
        reason(&gate.check(&SpatialOutputPath::ChannelBus {
            layout: SpatialLayout::Surround222
        })),
        SpatialUnavailable::ApprovedValidatorRequired { .. }
    ));
}

#[test]
fn invalid_monitoring_config_is_not_approximated() {
    let mut e = env();
    let mut bad_cal = cal(SpatialLayout::Surround51);
    bad_cal.remove(&Speaker::M110);
    e.monitoring = Some(MonitoringConfig {
        speaker_set: SpatialLayout::Surround51,
        level_calibration_db: bad_cal,
        fallback: FallbackMode::None,
        distance_m: None,
    });
    let gate = SpatialGate::new(e);
    match reason(&gate.check(&SpatialOutputPath::ChannelBus {
        layout: SpatialLayout::Surround51,
    })) {
        SpatialUnavailable::MonitoringInvalid { errors } => {
            assert!(errors.iter().any(|s| s.contains("M110")));
        }
        other => panic!("expected MonitoringInvalid, got {other:?}"),
    }
}

#[test]
fn head_tracked_binaural_needs_declared_hardware() {
    let gate = SpatialGate::new(env());
    // Plain binaural works — headphones + selfcheck scope.
    assert!(gate
        .check(&SpatialOutputPath::Binaural {
            head_tracked: false
        })
        .is_available());
    // Head-tracked binaural without declared hardware → typed reason.
    assert_eq!(
        reason(&gate.check(&SpatialOutputPath::Binaural {
            head_tracked: true
        })),
        &SpatialUnavailable::HeadTrackingUnavailable
    );

    // Declared → available.
    let mut e = env();
    e.head_tracking_declared = true;
    let gate = SpatialGate::new(e);
    assert!(gate
        .check(&SpatialOutputPath::Binaural {
            head_tracked: true
        })
        .is_available());
}

#[test]
fn atmos_paths_require_licensed_approved_validator() {
    let gate = SpatialGate::new(env());
    // Selfcheck alone is never enough for licensed deliverables.
    for path in [
        SpatialOutputPath::AtmosAdmBwf {
            container: container(SpatialLayout::Surround51),
        },
        SpatialOutputPath::AtmosMp4 {
            container: container(SpatialLayout::Surround51),
        },
    ] {
        assert!(matches!(
            reason(&gate.check(&path)),
            SpatialUnavailable::LicensedRendererRequired { .. }
        ));
    }

    // A registered Approved validator with evidence unlocks it.
    let mut e = env();
    e.validators.push(ValidatorRecord {
        id: "atmos-conformance-qc/1.0".into(),
        kind: ValidatorKind::Approved,
        scopes: vec!["atmos-adm-bwf".into(), "atmos-mp4".into()],
        evidence_ref: Some("sha256:abc".into()),
    });
    let gate = SpatialGate::new(e);
    assert!(gate
        .check(&SpatialOutputPath::AtmosAdmBwf {
            container: container(SpatialLayout::Surround51)
        })
        .is_available());
    assert!(gate
        .check(&SpatialOutputPath::AtmosMp4 {
            container: container(SpatialLayout::Surround51)
        })
        .is_available());

    // An Approved validator WITHOUT evidence is not honoured — a
    // record must carry proof.
    let mut e = env();
    e.validators.push(ValidatorRecord {
        id: "claim-only".into(),
        kind: ValidatorKind::Approved,
        scopes: vec!["atmos-adm-bwf".into()],
        evidence_ref: None,
    });
    let gate = SpatialGate::new(e);
    assert!(matches!(
        reason(&gate.check(&SpatialOutputPath::AtmosAdmBwf {
            container: container(SpatialLayout::Surround51)
        })),
        SpatialUnavailable::LicensedRendererRequired { .. }
    ));
}

#[test]
fn invalid_container_rejects_before_anything() {
    let gate = SpatialGate::new(env());
    let mut c = container(SpatialLayout::Stereo); // illegal bed
    c.objects.clear();
    let v = gate.check(&SpatialOutputPath::AtmosAdmBwf { container: c });
    match reason(&v) {
        SpatialUnavailable::InvalidContainer { errors } => {
            assert!(errors.iter().any(|s| s.contains("bed")));
        }
        other => panic!("expected InvalidContainer, got {other:?}"),
    }
}

#[test]
fn bed_plus_objects_resolves_monitoring() {
    // 5.1 bed+objects on a 5.1 rig with selfcheck scope → available.
    // (7.1.2 on a 22.2 rig is honestly NOT drivable — its U030/U330
    // top fronts don't exist in the 22.2 set.)
    let mut e = env();
    e.monitoring = Some(MonitoringConfig {
        speaker_set: SpatialLayout::Surround51,
        level_calibration_db: cal(SpatialLayout::Surround51),
        fallback: FallbackMode::None,
        distance_m: None,
    });
    let gate = SpatialGate::new(e);
    match gate.check(&SpatialOutputPath::BedPlusObjects {
        container: container(SpatialLayout::Surround51),
    }) {
        GateVerdict::Available { monitor_route, .. } => {
            assert!(monitor_route.contains("objects"));
        }
        other => panic!("expected Available, got {other:?}"),
    }

    // Stereo rig without a reachable fallback → unavailable.
    let gate = SpatialGate::new(env());
    assert!(matches!(
        reason(&gate.check(&SpatialOutputPath::BedPlusObjects {
            container: container(SpatialLayout::Surround51)
        })),
        SpatialUnavailable::MonitorUnreachable { .. }
    ));
}
