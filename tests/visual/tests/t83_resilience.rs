//! T83 — Renderer failure and mixed undo: drop-under-load proof
//! (counters increment, audio-side clock source unaffected), joint
//! transaction undo, failure/decode boundary — never labeled atomic
//! success.

mod common;
use common::*;
use void_visual::*;

#[test]
fn t83_drop_under_load_counters_and_audio_unaffected() {
    // Synthetic slow render (25 ms/frame ≫ clock cadence): the runtime
    // must DROP/SKIP visual work — never delay the audio-side timestamp
    // source. Evidence: counters rise, and a clock pushed after the
    // storm still renders a frame stamped with ITS sequence.
    let mut rt = VisualRuntime::spawn(PROJECT, EPOCH, 128, 72, std::env::temp_dir(), "ffmpeg".into())
        .expect("runtime");
    rt.send_command(add_gen("t", 0, "g", 0, VisualChannel::Program)).unwrap();
    rt.set_frame_delay_us(25_000); // 25ms per render, ~10x a 60fps cadence

    // Storm of clocks; each push is instantaneous for the caller.
    for seq in 1..=120u64 {
        rt.push_clock(clock_at_tick(seq, seq as i64 * 8_000, 120.0));
    }
    std::thread::sleep(std::time::Duration::from_millis(400));
    // Fresh clock after the storm.
    rt.push_clock(clock_at_tick(1_000, 999 * TICKS_PER_QUARTER, 120.0));
    std::thread::sleep(std::time::Duration::from_millis(200));
    rt.set_frame_delay_us(0);
    rt.push_clock(clock_at_tick(1_001, 1_000 * TICKS_PER_QUARTER, 120.0));
    std::thread::sleep(std::time::Duration::from_millis(200));

    let mut last: Option<FrameEvent> = None;
    while let Some(ev) = rt.take_frame() {
        last = Some(ev);
    }
    rt.shutdown();
    let c = rt.counters();
    assert!(
        c.dropped_clocks + c.skipped_renders + c.dropped_frames > 0,
        "overload must produce counted drops: {c:?}"
    );
    assert!(
        c.produced_frames < 120,
        "slow renderer cannot have produced all 120 clocks' worth: {c:?}"
    );
    let last = last.expect("at least one frame after the storm");
    // The surviving frame carries an AUDIO clock stamp — the clock source
    // was never disturbed by the overload.
    assert!(last.frame.clock_sequence > 0);
    assert!(last.frame.device_sample_counter > 0);
    assert!(last.counters.produced_frames > 0);
}

#[test]
fn t83_mixed_undo_rewinds_whole_transaction() {
    // One transaction (a combined "edit gesture") = add layer + set
    // transform + set blend + attach anchor bind. Undo(tx) rewinds all
    // of it — the visible scene returns to pre-transaction state, and
    // redo replays it.
    let mut scene = Scene::new(PROJECT, EPOCH);
    scene.apply(&cmd(1, "setup", 0, VisualOp::SetVisualAnchorOp {
        anchor_id: "a1".into(),
        kind: AnchorKind::BeatTick,
        position_ticks: 2 * TICKS_PER_QUARTER,
        position_sample: -1,
        timecode_ns: -1,
    }));
    // Mixed transaction under tx "mix-1".
    let mut rev = 1;
    for (i, op) in [
        VisualOp::AddVisualLayerOp {
            layer_id: "l".into(),
            kind: VisualLayerKind::Generator,
            name: "l".into(),
            index: 0,
            channel: VisualChannel::Program,
            generator: Some(GeneratorSpec {
                preset: "plasma".into(),
                seed: 3,
                param_json: "{}".into(),
            }),
        },
        VisualOp::SetLayerBlendOp {
            layer_id: "l".into(),
            blend_mode: BlendMode::Add,
        },
        VisualOp::SetLayerTransformOp {
            layer_id: "l".into(),
            transform: VisualTransform {
                x: 5.0,
                y: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
                rotation_rad: 0.0,
                opacity: 1.0,
            },
        },
        VisualOp::BindLayerAnchorOp {
            layer_id: "l".into(),
            in_anchor_id: "a1".into(),
            out_anchor_id: String::new(),
        },
    ]
    .into_iter()
    .enumerate()
    {
        let r = scene.apply(&cmd(10 + i as u64, "mix-1", rev, op));
        assert_eq!(r.status, VisualAckStatus::Applied, "{:?}", r);
        rev += 1;
    }
    assert!(scene.layer("l").is_some());
    let undo = scene.apply(&cmd(50, "mix-1", rev, VisualOp::VisualUndoOp {
        transaction_id: "mix-1".into(),
    }));
    assert_eq!(undo.status, VisualAckStatus::Applied, "{}", undo.message);
    // Whole transaction rewound — layer gone.
    assert!(scene.layer("l").is_none(), "undo must rewind the whole transaction");
    let redo = scene.apply(&cmd(51, "mix-1", rev + 1, VisualOp::VisualRedoOp {
        transaction_id: "mix-1".into(),
    }));
    assert_eq!(redo.status, VisualAckStatus::Applied, "{}", redo.message);
    let l = scene.layer("l").expect("redo replays the layer");
    assert_eq!(l.blend, BlendMode::Add);
}

#[test]
fn t83_failure_boundary_never_atomic_success() {
    // Rejected ops must not advance revision or leave partial state —
    // a partial transaction is compensated or rejected, never silently
    // half-applied.
    let mut scene = Scene::new(PROJECT, EPOCH);
    // Stale revision → rejected, no state change.
    let r = scene.apply(&cmd(1, "t", 99, VisualOp::ClearVisualSceneOp {}));
    assert_eq!(r.status, VisualAckStatus::Rejected);
    assert_eq!(r.error, VisualErrorCode::StaleRevision);
    // Unknown layer refs rejected.
    let r = scene.apply(&cmd(2, "t", 0, VisualOp::SetLayerBlendOp {
        layer_id: "ghost".into(),
        blend_mode: BlendMode::Screen,
    }));
    assert_eq!(r.status, VisualAckStatus::Rejected);
    // Duplicate command id with different payload → COMMAND_ID_REUSE.
    scene.apply(&cmd(3, "t", 0, VisualOp::ClearVisualSceneOp {}));
    let dup = VisualCommand {
        op: VisualOp::SetVisualAnchorOp {
            anchor_id: "x".into(),
            kind: AnchorKind::BeatTick,
            position_ticks: 0,
            position_sample: -1,
            timecode_ns: -1,
        },
        ..cmd(3, "t", 1, VisualOp::ClearVisualSceneOp {})
    };
    let r = scene.apply(&dup);
    assert_eq!(r.error, VisualErrorCode::CommandIdReuse);
    // Same command id + same payload → DUPLICATE (idempotent).
    let r2 = scene.apply(&cmd(3, "t", 1, VisualOp::ClearVisualSceneOp {}));
    assert_eq!(r2.status, VisualAckStatus::Duplicate);
    // Undo with nothing to undo → rejected/no-op, not a fake success.
    let r3 = scene.apply(&cmd(4, "t", 1, VisualOp::VisualUndoOp {
        transaction_id: "never-happened".into(),
    }));
    assert_ne!(r3.status, VisualAckStatus::Applied);
}

#[test]
fn t83_decode_failure_surfaces_as_alert_not_black() {
    // Missing/corrupt media → AssetMissing/DecodeFailed path that the
    // runtime reports on the alert channel; rendering continues.
    let mut rt = VisualRuntime::spawn(PROJECT, EPOCH, 64, 64, std::env::temp_dir(), "ffmpeg".into())
        .unwrap();
    rt.send_command(cmd(1, "t", 0, VisualOp::AddVisualLayerOp {
        layer_id: "img".into(),
        kind: VisualLayerKind::Image,
        name: "i".into(),
        index: 0,
        channel: VisualChannel::Program,
        generator: None,
    }))
    .unwrap();
    rt.send_command(cmd(2, "t", 1, VisualOp::AttachVisualMediaOp {
        layer_id: "img".into(),
        asset_id: "a".into(),
        sha256: "ab".repeat(32),
        media_kind: "image".into(),
        rel_path: "no/such/file.png".into(),
        duration_ticks: 0,
    }))
    .unwrap();
    rt.push_clock(clock_at_tick(1, 0, 120.0));
    std::thread::sleep(std::time::Duration::from_millis(300));
    let mut saw = false;
    while let Some(a) = rt.take_alert() {
        if let VisualAlert::DecodeFailed { layer_id, .. } = a {
            if layer_id == "img" {
                saw = true;
            }
        }
    }
    // Engine still renders other layers after the failure.
    rt.send_command(add_gen("t", 2, "g", 1, VisualChannel::Program)).unwrap();
    rt.push_clock(clock_at_tick(2, 24_000, 120.0));
    std::thread::sleep(std::time::Duration::from_millis(300));
    let mut frames = 0;
    while let Some(_) = rt.take_frame() {
        frames += 1;
    }
    rt.shutdown();
    assert!(saw, "decode failure must surface as a VisualAlert");
    assert!(frames > 0, "engine keeps rendering after a media failure");
}
