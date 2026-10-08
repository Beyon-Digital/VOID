//! T82 — Clock and display calibration: visual events land on the
//! audio-derived tick (never wall time), quantized transitions fire on
//! beat boundaries, frames carry audio-clock stamps, and the audio side
//! is never blocked.

mod common;
use common::*;
use void_visual::*;

#[test]
fn t82_frames_are_stamped_with_audio_clock() {
    // Every produced frame must carry the CLOCK SNAPSHOT it was rendered
    // for — timeline_sample/device_sample_counter/sequence from the
    // audio clock, not a visual-side timer.
    let mut rt = VisualRuntime::spawn(PROJECT, EPOCH, 160, 90, std::env::temp_dir(), "ffmpeg".into())
        .expect("runtime");
    rt.send_command(add_gen("t", 0, "g", 0, VisualChannel::Program)).unwrap();
    // Drive clocks until a frame lands (bounded wait — the engine thread
    // consumes snapshots on its own schedule).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
    let mut got = None;
    let mut seq = 1u64;
    while std::time::Instant::now() < deadline {
        rt.push_clock(clock_at_tick(seq, seq as i64 * 24_000, 120.0));
        seq += 1;
        // Both channel routes are enabled by default — keep draining
        // until the PROGRAM frame lands (latest-wins slot).
        while let Some(ev) = rt.take_frame() {
            if ev.frame.channel == VisualChannel::Program {
                got = Some(ev);
            }
        }
        if got.is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let ev = got.expect("a program frame was produced");
    let f = ev.frame;
    // Stamp equals an AUDIO clock snapshot: at 120bpm the tempo map
    // converts tick = sample × 120 × 960000 / (60 × 48000) = sample × 40.
    assert!(f.timeline_sample > 0);
    assert_eq!(f.position_ticks, f.timeline_sample * 40);
    assert!(f.device_sample_counter > 0);
    assert_eq!(f.channel, VisualChannel::Program);
    assert!(f.position_ticks >= 0);
    rt.shutdown();
}

#[test]
fn t82_audio_side_never_blocks() {
    // push_clock + send_command must return immediately even while the
    // engine is mid-render — verified by counting: a caller pushing 500
    // clocks back-to-back sees the calls themselves complete (no waits)
    // and unconsumed snapshots are counted as dropped_clocks.
    let mut rt = VisualRuntime::spawn(PROJECT, EPOCH, 320, 180, std::env::temp_dir(), "ffmpeg".into())
        .expect("runtime");
    rt.send_command(add_gen("t", 0, "g", 0, VisualChannel::Program)).unwrap();
    let t0 = std::time::Instant::now();
    for seq in 1..=500u64 {
        rt.push_clock(clock_at_tick(seq, seq as i64 * 1_000, 120.0));
    }
    let elapsed = t0.elapsed();
    assert!(
        elapsed.as_millis() < 2_000,
        "audio clock submission blocked for {elapsed:?} — violates audio-never-waits"
    );
    rt.shutdown();
    let c = rt.counters();
    assert!(
        c.dropped_clocks + c.skipped_renders + 1 >= 500 - c.produced_frames * 2,
        "every unconsumed clock must be accounted: {c:?}"
    );
}

#[test]
fn t82_quantized_transition_fires_on_beat() {
    // Take with NEXT_BEAT quantize fires at the next quarter boundary,
    // not at the take tick; NEXT_BAR at the next 4/4 bar.
    let mut scene = Scene::new(PROJECT, EPOCH);
    scene.apply(&add_gen("t", 0, "a", 0, VisualChannel::Program));
    scene.apply(&add_gen("t", 1, "b", 0, VisualChannel::Preview));
    scene.apply(&cmd(3, "t", 2, VisualOp::SetTransitionOp {
        channel: VisualChannel::Program,
        kind: TransitionKind::Cut,
        duration_ticks: 0,
        quantize: QuantizeMode::NextBeat,
        wipe_angle: 0.0,
    }));
    scene.apply(&cmd(4, "t", 3, VisualOp::TakeTransitionOp {
        channel: VisualChannel::Program,
    }));
    let tick = 960_000 + 123_456; // inside beat 2
    let events = scene.advance(tick);
    assert!(
        !events.iter().any(|e| matches!(e, SceneEvent::TransitionFired { .. })),
        "quantized take fired early inside the beat"
    );
    let events = scene.advance(2 * TICKS_PER_QUARTER);
    assert!(
        events.iter().any(|e| matches!(e, SceneEvent::TransitionFired { .. })),
        "take must fire exactly on the next beat boundary"
    );
}

#[test]
fn t82_clock_rejects_stale_epoch_and_reorders() {
    // Snapshots from an older epoch or out-of-order sequences are
    // discarded, never rendered against.
    let mut tr = ClockTracker::new(EPOCH);
    tr.push(clock(1, 0, 0)).unwrap();
    let mut old = clock(2, 100, 100);
    old.engine_epoch = EPOCH - 1;
    assert!(matches!(tr.push(old), Err(ClockReject::StaleEpoch)));
    let mut dup = clock(1, 200, 200); // sequence went backwards
    dup.engine_epoch = EPOCH;
    assert!(matches!(tr.push(dup), Err(ClockReject::StaleSequence)));
    assert_eq!(tr.rejected_stale_epoch, 1);
    assert_eq!(tr.rejected_stale_sequence, 1);
}

#[test]
fn t82_rational_frame_rate_and_route_bounds() {
    // Output route accepts rational fps (30000/1001 = 29.97 broadcast
    // rate) and rejects nonsense dims; preview vs program are
    // independent routes.
    let mut scene = Scene::new(PROJECT, EPOCH);
    let ok = scene.apply(&cmd(1, "t", 0, VisualOp::SetOutputRouteOp {
        channel: VisualChannel::Program,
        target: OutputTarget::Offscreen,
        display_id: String::new(),
        width: 1920,
        height: 1080,
        fps_num: 30_000,
        fps_den: 1_001,
        readback: true,
    }));
    assert_eq!(ok.status, VisualAckStatus::Applied, "{}", ok.message);
    assert_eq!(scene.route(VisualChannel::Program).fps_num, 30_000);
    let bad = scene.apply(&cmd(2, "t", 1, VisualOp::SetOutputRouteOp {
        channel: VisualChannel::Program,
        target: OutputTarget::Offscreen,
        display_id: String::new(),
        width: 20_000, // over the 16384 bound
        height: 1080,
        fps_num: 0,
        fps_den: 0,
        readback: true,
    }));
    assert_eq!(bad.status, VisualAckStatus::Rejected);
}
