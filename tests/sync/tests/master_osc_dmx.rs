//! Master arbiter + drift + OSC bounds + DMX descriptor tests
//! (T94/T95 evidence).

use std::collections::BTreeMap;

use void_sync::{
    sync_health, validate_against, ArbiterEvent, ConflictResolution, DmxChannelDescriptor,
    DmxFixture, DmxRig, DmxUniverse, DriftModel, MasterArbiter, OscAddress, OscArg, OscArgSpec,
    OscControl, OscMessage, SyncHealth, SyncSource, MAX_OSC_PAYLOAD_ARGS, MAX_STROBE_HZ,
};

// ---------- tempo-master arbiter ----------

#[test]
fn first_claim_wins_competing_claim_is_typed_conflict() {
    let mut a = MasterArbiter::new();
    a.claim(SyncSource::Internal).unwrap();
    // Idempotent re-claim is fine.
    a.claim(SyncSource::Internal).unwrap();
    let c = a
        .claim(SyncSource::MtcIn {
            endpoint: "hw-1".into(),
        })
        .unwrap_err();
    assert_eq!(c.held, SyncSource::Internal);
    assert!(matches!(c.attempted, SyncSource::MtcIn { .. }));
    // Master unchanged.
    assert_eq!(a.master(), Some(&SyncSource::Internal));
}

#[test]
fn keep_held_resolution_is_recorded() {
    let mut a = MasterArbiter::new();
    a.claim(SyncSource::Internal).unwrap();
    let c = a
        .claim(SyncSource::MidiClockIn {
            endpoint: "hw-2".into(),
        })
        .unwrap_err();
    a.resolve(&c, ConflictResolution::KeepHeld);
    assert_eq!(a.master(), Some(&SyncSource::Internal));
    assert!(a
        .log()
        .iter()
        .any(|e| matches!(e, ArbiterEvent::ConflictRejected { .. })));
}

#[test]
fn switch_at_next_stop_applies_on_stop() {
    let mut a = MasterArbiter::new();
    a.claim(SyncSource::Internal).unwrap();
    let ext = SyncSource::OscExternal {
        prefix: "/void".into(),
    };
    let c = a.claim(ext.clone()).unwrap_err();
    a.resolve(&c, ConflictResolution::SwitchAtNextStop);
    // Still internal until the transport actually stops.
    assert_eq!(a.master(), Some(&SyncSource::Internal));
    a.on_transport_stop();
    assert_eq!(a.master(), Some(&ext));
}

#[test]
fn switch_immediate_and_release() {
    let mut a = MasterArbiter::new();
    a.claim(SyncSource::Internal).unwrap();
    let ext = SyncSource::MtcIn {
        endpoint: "hw".into(),
    };
    let c = a.claim(ext.clone()).unwrap_err();
    a.resolve(&c, ConflictResolution::SwitchImmediate);
    assert_eq!(a.master(), Some(&ext));
    // Release requires the holder.
    assert!(!a.release(&SyncSource::Internal));
    assert!(a.release(&ext));
    assert_eq!(a.master(), None);
}

// ---------- drift measurement ----------

#[test]
fn drift_is_measured_not_asserted() {
    let m = DriftModel {
        tolerance_ppm: 100.0,
        jump_threshold_pulses: 6,
    };
    // Perfect: within tolerance.
    let r = m.measure(100_000, 100_000, 10.0, 0);
    assert!(r.within_tolerance);
    assert_eq!(r.error_ppm, 0.0);
    // +50 ppm: within 100 ppm tolerance.
    let r = m.measure(100_000, 100_005, 10.0, 0);
    assert!(r.within_tolerance);
    // +500 ppm: flagged.
    let r = m.measure(100_000, 100_050, 10.0, 0);
    assert!(!r.within_tolerance);
    assert!(r.error_ppm > 0.0);
    // Zero expected but traffic arrived → hard-flagged, not divided.
    let r = m.measure(0, 5, 10.0, 0);
    assert!(!r.within_tolerance);
    // Position discontinuity above the jump threshold → resync.
    let r = m.measure(100_000, 100_000, 10.0, 7);
    assert!(r.jump_detected);
    let r = m.measure(100_000, 100_000, 10.0, 6);
    assert!(!r.jump_detected);
}

#[test]
fn health_rolls_up_liveness_and_drift() {
    let m = DriftModel::default();
    assert_eq!(
        sync_health(false, None, 0, 2000),
        SyncHealth::NoMaster
    );
    // Silence past the lost threshold.
    assert_eq!(
        sync_health(true, None, 2500, 2000),
        SyncHealth::Lost { since_ms: 2500 }
    );
    let good = m.measure(1000, 1000, 1.0, 0);
    assert_eq!(
        sync_health(true, Some(&good), 0, 2000),
        SyncHealth::Locked
    );
    let bad = m.measure(1000, 2000, 1.0, 0);
    assert!(matches!(
        sync_health(true, Some(&bad), 0, 2000),
        SyncHealth::Drifting { .. }
    ));
    let jumpy = m.measure(1000, 1000, 1.0, 10);
    assert!(matches!(
        sync_health(true, Some(&jumpy), 0, 2000),
        SyncHealth::ResyncRequired { .. }
    ));
}

// ---------- OSC ----------

#[test]
fn osc_addresses_validate_literal_form() {
    assert!(OscAddress::parse("/void/transport/bpm").is_ok());
    for bad in [
        "",
        "no/leading/slash",
        "/",
        "//double",
        "/trailing/",
        "/has space/x",
        "/wild*/card",
        "/quest?ion",
        "/br[ack]et",
        "/brace{ch}an",
        "/hash#tag",
        "/com,ma",
    ] {
        assert!(OscAddress::parse(bad).is_err(), "accepted {bad:?}");
    }
}

#[test]
fn osc_pattern_matching_is_spec_shaped() {
    let a = OscAddress::parse("/void/track/1/fader").unwrap();
    assert!(a.matches("/void/track/*/fader"));
    assert!(a.matches("/void/*/1/fader"));
    assert!(a.matches("/void/track/?/fader"));
    assert!(a.matches("/void/track/{1,2,3}/fader"));
    assert!(a.matches("/void/track/[0-9]/fader"));
    assert!(a.matches("/void/track/[!4-9]/fader"));
    assert!(!a.matches("/void/track/1"));
    assert!(!a.matches("/other/track/1/fader"));
    assert!(!a.matches("/void/track/2/3/fader"));
    // * does not cross segment boundaries.
    assert!(!a.matches("/void/*/fader"));
}

#[test]
fn osc_bounded_message_validation() {
    let control = OscControl {
        address: "/void/track/1/fader".into(),
        args: vec![OscArgSpec::float_range(0.0, 1.0)],
        label: "fader".into(),
    };
    // Well-formed.
    let ok = OscMessage {
        address: "/void/track/1/fader".into(),
        args: vec![OscArg::Float32(0.5)],
    };
    assert!(validate_against(&control, &ok).is_ok());

    // Wrong address.
    let bad = OscMessage {
        address: "/void/track/2/fader".into(),
        args: vec![OscArg::Float32(0.5)],
    };
    assert!(validate_against(&control, &bad).is_err());
    // Wrong arg count.
    let bad = OscMessage {
        address: "/void/track/1/fader".into(),
        args: vec![],
    };
    assert!(validate_against(&control, &bad).is_err());
    // Wrong type.
    let bad = OscMessage {
        address: "/void/track/1/fader".into(),
        args: vec![OscArg::Str("hi".into())],
    };
    assert!(validate_against(&control, &bad).is_err());
    // Out of declared range — a bound is a bound.
    let bad = OscMessage {
        address: "/void/track/1/fader".into(),
        args: vec![OscArg::Float32(1.01)],
    };
    assert!(validate_against(&control, &bad).is_err());
    // Non-finite.
    let bad = OscMessage {
        address: "/void/track/1/fader".into(),
        args: vec![OscArg::Float32(f32::NAN)],
    };
    assert!(validate_against(&control, &bad).is_err());
    // Payload bound.
    let big = OscMessage {
        address: "/void/track/1/fader".into(),
        args: vec![OscArg::True; MAX_OSC_PAYLOAD_ARGS + 1],
    };
    assert!(validate_against(&control, &big).is_err());

    // String length bound.
    let s_control = OscControl {
        address: "/void/name".into(),
        args: vec![OscArgSpec::string(8)],
        label: "name".into(),
    };
    let bad = OscMessage {
        address: "/void/name".into(),
        args: vec![OscArg::Str("ninechars".into())],
    };
    assert!(validate_against(&s_control, &bad).is_err());
}

// ---------- DMX ----------

fn ch(offset: u16, function: &str) -> DmxChannelDescriptor {
    DmxChannelDescriptor {
        offset,
        function: function.into(),
        strobe_max_hz: None,
    }
}

#[test]
fn dmx_universe_bounds_are_real() {
    let mut u = DmxUniverse::new(0);
    u.set_channel(1, 255).unwrap();
    u.set_channel(512, 128).unwrap();
    assert_eq!(u.channel(1).unwrap(), 255);
    assert_eq!(u.channel(512).unwrap(), 128);
    assert!(u.set_channel(0, 0).is_err());
    assert!(u.set_channel(513, 0).is_err());
    assert!(u.channel(0).is_err());
    u.blackout();
    assert_eq!(u.channel(1).unwrap(), 0);
    assert_eq!(u.channel(512).unwrap(), 0);
}

#[test]
fn dmx_rig_rejects_overlap_and_bad_footprints() {
    let mut rig = DmxRig::default();
    rig.universes.insert(0, DmxUniverse::new(0));
    rig.fixtures.push(DmxFixture {
        fixture_id: "wash-1".into(),
        universe_index: 0,
        start_address: 1,
        channels: vec![ch(1, "dimmer"), ch(2, "pan"), ch(3, "tilt")],
    });
    // Non-overlapping fixture is fine.
    rig.fixtures.push(DmxFixture {
        fixture_id: "wash-2".into(),
        universe_index: 0,
        start_address: 4,
        channels: vec![ch(1, "dimmer")],
    });
    assert!(rig.validate().is_ok());

    // Overlapping → typed conflict.
    rig.fixtures.push(DmxFixture {
        fixture_id: "wash-3".into(),
        universe_index: 0,
        start_address: 3,
        channels: vec![ch(1, "dimmer"), ch(2, "strobe")],
    });
    let errs = rig.validate().unwrap_err();
    assert!(errs.iter().any(|e| e.to_string().contains("overlap")));

    // Strobe above the safety bound is rejected at the descriptor.
    let mut rig2 = DmxRig::default();
    rig2.universes.insert(0, DmxUniverse::new(0));
    rig2.fixtures.push(DmxFixture {
        fixture_id: "strobe-1".into(),
        universe_index: 0,
        start_address: 1,
        channels: vec![DmxChannelDescriptor {
            offset: 1,
            function: "strobe".into(),
            strobe_max_hz: Some(MAX_STROBE_HZ + 5.0),
        }],
    });
    let errs = rig2.validate().unwrap_err();
    assert!(errs.iter().any(|e| e.to_string().contains("strobe")));

    // Fixture into a missing universe is an error too.
    let mut rig3 = DmxRig::default();
    rig3.fixtures.push(DmxFixture {
        fixture_id: "orphan".into(),
        universe_index: 9,
        start_address: 1,
        channels: vec![ch(1, "dimmer")],
    });
    assert!(rig3.validate().is_err());

    // Footprint spilling past 512.
    let mut rig4 = DmxRig::default();
    rig4.universes.insert(0, DmxUniverse::new(0));
    rig4.fixtures.push(DmxFixture {
        fixture_id: "spill".into(),
        universe_index: 0,
        start_address: 512,
        channels: vec![ch(1, "a"), ch(2, "b")],
    });
    assert!(rig4.validate().is_err());
    let _unused: BTreeMap<u16, u8> = BTreeMap::new();
}
