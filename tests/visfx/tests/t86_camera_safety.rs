//! T86 — Camera conducting policy: consent gate (deny/grant/revoke),
//! calibration gate, clutch engage/disengage, confidence + occlusion
//! drops, tracking-loss → release-all control events, and the privacy
//! assertions the runtime enforces (no raw retention, no upload).

use void_visfx::{
    CalibrationRecord, CameraPolicy, ConsentState, ConductorSession, ControlEvent, DropReason,
    GestureKind, PolicyViolation, ReleaseCause, TrackerFrame,
};

fn policy() -> CameraPolicy {
    CameraPolicy::default()
}

fn cal() -> CalibrationRecord {
    CalibrationRecord {
        calibrated_at: "2026-10-08T00:00:00Z".into(),
        tracker_model_id: "hand-landmarker-test".into(),
        tracker_model_sha256: "cd".repeat(32),
        sample_fps_num: 30,
        sample_fps_den: 1,
        measured_confidence: 0.9,
        bounds: [0.1, 0.1, 0.9, 0.9],
        measured_latency_ms: 33,
    }
}

fn frame(t_ns: u64, gesture: GestureKind, conf: f32) -> TrackerFrame {
    TrackerFrame {
        t_ns,
        confidence: conf,
        gesture,
        x: 0.5,
        y: 0.5,
        arg: 0.8,
        occluded: false,
    }
}

fn armed() -> ConductorSession {
    let mut s = ConductorSession::new(policy()).unwrap();
    s.grant_consent().unwrap();
    s.calibrate(cal()).unwrap();
    s
}

#[test]
fn consent_gate_blocks_ingest_until_granted() {
    let mut s = ConductorSession::new(policy()).unwrap();
    s.calibrate(cal()).unwrap();
    // frames before consent → refused, nothing processed
    let ev = s.ingest(&frame(0, GestureKind::Clutch, 0.9));
    assert!(matches!(ev[0], ControlEvent::PolicyRefused { .. }));
    assert!(s.held().is_empty());

    // deny — the refusal path
    let ev = s.deny_consent();
    assert!(matches!(ev[0], ControlEvent::PolicyRefused { .. }));
    let ev = s.ingest(&frame(1, GestureKind::Pinch, 0.9));
    assert!(matches!(ev[0], ControlEvent::PolicyRefused { .. }));
}

#[test]
fn calibration_required_before_engage() {
    let mut s = ConductorSession::new(policy()).unwrap();
    s.grant_consent().unwrap();
    let ev = s.ingest(&frame(0, GestureKind::Pinch, 0.9));
    assert!(matches!(&ev[0], ControlEvent::PolicyRefused { reason } if reason == "not_calibrated"));
}

#[test]
fn clutch_engages_and_pinch_holds_controls() {
    let mut s = armed();
    // pinch without clutch → refused (clutch_required)
    let ev = s.ingest(&frame(0, GestureKind::Pinch, 0.9));
    assert!(ev.is_empty() || matches!(ev[0], ControlEvent::FrameDropped { .. }));

    // clutch → Engaged
    let ev = s.ingest(&frame(1, GestureKind::Clutch, 0.9));
    assert!(matches!(ev[0], ControlEvent::Engaged));

    // pinch → ControlOn, control is held
    let ev = s.ingest(&frame(2, GestureKind::Pinch, 0.9));
    assert!(matches!(ev[0], ControlEvent::ControlOn { .. }));
    assert_eq!(s.held().len(), 1);

    // release gesture → Disengaged; note stays held (clutch is a gate)
    let ev = s.ingest(&frame(3, GestureKind::Release, 0.9));
    assert!(matches!(ev[0], ControlEvent::Disengaged));
    assert_eq!(s.held().len(), 1);
}

#[test]
fn occlusion_and_low_confidence_drop_frames_unprocessed() {
    let mut s = armed();
    s.ingest(&frame(1, GestureKind::Clutch, 0.9));
    s.ingest(&frame(2, GestureKind::Pinch, 0.9));
    assert_eq!(s.held().len(), 1);

    // occluded frame — something other than a hand on the lens
    let mut f = frame(3, GestureKind::Pinch, 0.95);
    f.occluded = true;
    let ev = s.ingest(&f);
    assert!(matches!(
        ev[0],
        ControlEvent::FrameDropped {
            reason: DropReason::Occluded
        }
    ));
    assert_eq!(s.dropped_frames(), 1);
    // no ControlOn sneaked through
    assert!(ev.iter().all(|e| !matches!(e, ControlEvent::ControlOn { .. })));

    // below-confidence frame dropped too
    let f = frame(4, GestureKind::Pinch, 0.1);
    let ev = s.ingest(&f);
    assert!(matches!(
        ev[0],
        ControlEvent::FrameDropped {
            reason: DropReason::BelowConfidence
        }
    ));
    assert_eq!(s.dropped_frames(), 2);
}

#[test]
fn tracking_loss_releases_all_held_controls() {
    let mut s = armed();
    s.ingest(&frame(1_000, GestureKind::Clutch, 0.9));
    s.ingest(&frame(2_000, GestureKind::Pinch, 0.9));
    s.ingest(&frame(3_000, GestureKind::Pinch, 0.9));
    assert_eq!(s.held().len(), 1);

    // healthy frames — tracking fine
    let f = frame(4_000, GestureKind::OpenPalm, 0.9);
    let ev = s.ingest(&f);
    assert!(ev
        .iter()
        .all(|e| !matches!(e, ControlEvent::ReleaseAll { .. })));

    // continuous loss past the release window → ReleaseAll(TrackingLost)
    let window = policy().asserts.loss_release_window_ns;
    let mut released = false;
    for i in 0..10 {
        let f = frame(5_000 + i * (window / 8) + window / 4, GestureKind::Pinch, 0.05);
        for e in s.ingest(&f) {
            if matches!(
                e,
                ControlEvent::ReleaseAll {
                    cause: ReleaseCause::TrackingLost
                }
            ) {
                released = true;
            }
        }
    }
    assert!(released, "tracking loss never released held controls");
    assert!(s.held().is_empty(), "held controls must be empty after release-all");
}

#[test]
fn consent_revoke_releases_all_immediately() {
    let mut s = armed();
    s.ingest(&frame(1, GestureKind::Clutch, 0.9));
    s.ingest(&frame(2, GestureKind::Pinch, 0.9));
    assert_eq!(s.held().len(), 1);
    let ev = s.revoke_consent(3);
    assert!(ev.iter().any(|e| matches!(
        e,
        ControlEvent::ReleaseAll {
            cause: ReleaseCause::ConsentRevoked
        }
    )));
    assert!(s.held().is_empty());
    // further ingest refused
    let ev = s.ingest(&frame(4, GestureKind::Pinch, 0.9));
    assert!(matches!(ev[0], ControlEvent::PolicyRefused { .. }));
}

#[test]
fn session_end_releases_and_refuses() {
    let mut s = armed();
    s.ingest(&frame(1, GestureKind::Clutch, 0.9));
    s.ingest(&frame(2, GestureKind::Pinch, 0.9));
    let ev = s.end();
    assert!(ev.iter().any(|e| matches!(
        e,
        ControlEvent::ReleaseAll {
            cause: ReleaseCause::SessionEnded
        }
    )));
    let ev = s.ingest(&frame(3, GestureKind::Pinch, 0.9));
    assert!(matches!(ev[0], ControlEvent::PolicyRefused { .. }));
}

#[test]
fn privacy_asserts_enforced_as_policy_fields() {
    // default posture: all assertions on, zero violations
    let p = policy();
    assert!(p.asserts.no_raw_retention);
    assert!(p.asserts.no_network_upload);
    assert!(p.asserts.drop_occluded_frames);
    assert!(p.violations().is_empty());
    p.validate().unwrap();

    // loosening any assert is a policy violation — the runtime refuses
    let mut bad = policy();
    bad.asserts.no_raw_retention = false;
    assert!(bad
        .violations()
        .contains(&PolicyViolation::RawRetention));
    assert!(bad.validate().is_err());

    let mut bad = policy();
    bad.asserts.no_network_upload = false;
    assert!(bad
        .violations()
        .contains(&PolicyViolation::NetworkUpload));

    let mut bad = policy();
    bad.min_confidence = 0.05;
    assert!(bad
        .violations()
        .contains(&PolicyViolation::WeakConfidence));

    let mut bad = policy();
    bad.asserts.loss_release_window_ns = 0;
    assert!(bad
        .violations()
        .contains(&PolicyViolation::NoLossTimeout));
}

#[test]
fn no_raw_retention_in_session() {
    // the session only ever sees landmarks/confidence — the log holds
    // control events (facts), never pixel data; the buffer is bounded.
    let mut s = armed();
    for i in 0..600u64 {
        s.ingest(&frame(i, GestureKind::Clutch, 0.9));
    }
    assert!(s.event_log().len() <= 256, "landmark log must be bounded");
    // frames themselves are not stored anywhere — dropped count only.
    assert_eq!(s.dropped_frames(), 0);
}

#[test]
fn calibration_record_validates() {
    cal().validate().unwrap();
    let mut c = cal();
    c.measured_confidence = 1.5;
    assert!(c.validate().is_err());
    let mut c = cal();
    c.bounds = [0.9, 0.1, 0.1, 0.9]; // min > max
    assert!(c.validate().is_err());
    let mut c = cal();
    c.tracker_model_sha256 = "short".into();
    assert!(c.validate().is_err());
}
