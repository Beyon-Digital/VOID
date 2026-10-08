//! T81 — Conventional visual timeline: import image/preset layers,
//! trim/fade/transform, timing anchors, and save/reopen with joint
//! checkpoint revision. Evidence: identical inputs → identical pixels
//! (headless sha256), same layer state after checkpoint round-trip.

mod common;
use common::*;
use void_visual::*;

const W: u32 = 320;
const H: u32 = 180;

fn renderer() -> Renderer {
    Renderer::new_headless(W, H).expect("headless adapter (llvmpipe/lavapipe)")
}

#[test]
fn t81_render_determinism_same_inputs_same_sha() {
    // Same scene, two fresh renderers → identical frame sha256.
    let mut hashes = Vec::new();
    for _ in 0..2 {
        let mut scene = Scene::new(PROJECT, EPOCH);
        scene.apply(&add_gen("a", 0, "bars", 0, VisualChannel::Program));
        let mut r = renderer();
        let plan = scene.resolve(VisualChannel::Program, 0);
        let f = r
            .render(&plan, 0, 0, 0, 1, 0.0, true)
            .expect("render");
        assert!(!f.frame_sha256.is_empty());
        assert_eq!(f.pixels.as_ref().unwrap().len(), (W * H * 4) as usize);
        hashes.push(f.frame_sha256);
    }
    assert_eq!(hashes[0], hashes[1], "identical inputs must produce identical pixels");
    // And the composition is not trivially empty — color bars ≠ black.
    let mut r = renderer();
    let mut scene = Scene::new(PROJECT, EPOCH);
    scene.apply(&add_gen("a", 0, "bars", 0, VisualChannel::Program));
    let plan = scene.resolve(VisualChannel::Program, 0);
    let f = r.render(&plan, 0, 0, 0, 1, 0.0, true).unwrap();
    let px = f.pixels.unwrap();
    assert!(px.chunks(4).any(|p| p[0] > 0 || p[1] > 0 || p[2] > 0));
}

#[test]
fn t81_image_layer_real_decode_and_blend() {
    // Real PNG through `image` decode → texture → blend — no fake pixels.
    let dir = tempfile::tempdir().unwrap();
    let png = dir.path().join("media/logo.png");
    std::fs::create_dir_all(png.parent().unwrap()).unwrap();
    let sha = write_png(&png, 64, 64, 0x40);
    let frame = media::decode_image(&png).expect("decode");
    assert_eq!((frame.width, frame.height), (64, 64));

    let mut r = renderer();
    let mut scene = Scene::new(PROJECT, EPOCH);
    let mut rev = 0u64;
    let key_l = "img-layer";
    let mut c = cmd(1, "t", rev, VisualOp::AddVisualLayerOp {
        layer_id: key_l.into(),
        kind: VisualLayerKind::Image,
        name: "img".into(),
        index: 0,
        channel: VisualChannel::Program,
        generator: None,
    });
    assert_eq!(scene.apply(&c).status, VisualAckStatus::Applied);
    rev += 1;
    c = cmd(2, "t", rev, VisualOp::AttachVisualMediaOp {
        layer_id: key_l.into(),
        asset_id: "asset-1".into(),
        sha256: sha.clone(),
        media_kind: "image".into(),
        rel_path: "media/logo.png".into(),
        duration_ticks: 0,
    });
    assert_eq!(scene.apply(&c).status, VisualAckStatus::Applied);
    rev += 1;
    // Full-opacity center, solid generator underneath to prove blend.
    c = add_gen("t", rev, "bg", 0, VisualChannel::Program);
    assert_eq!(scene.apply(&c).status, VisualAckStatus::Applied);
    let _ = rev;

    r.set_layer_texture(&render::texture_key(key_l, &sha), &frame);
    let plan = scene.resolve(VisualChannel::Program, 0);
    let f = r.render(&plan, 0, 0, 0, 1, 0.0, true).unwrap();
    let px = f.pixels.unwrap();
    // Center pixel must carry the image's red component (x seed 0x40).
    let center = ((H / 2) * W + (W / 2)) as usize * 4;
    assert_eq!(px[center + 3], 255, "opaque image pixel alpha");
    assert!(px[center] > 0 || px[center + 1] > 0, "image content present");
}

#[test]
fn t81_beat_anchor_lands_on_tick() {
    // Anchor at beat 4: layer window [anchor, ∞) → layer appears exactly
    // at the anchor tick and not before.
    let mut scene = Scene::new(PROJECT, EPOCH);
    let a_tick = 4 * TICKS_PER_QUARTER;
    assert!(scene
        .apply(&cmd(1, "t", 0, VisualOp::SetVisualAnchorOp {
            anchor_id: "song-start".into(),
            kind: AnchorKind::BeatTick,
            position_ticks: a_tick,
            position_sample: -1,
            timecode_ns: -1,
        }))
        .status == VisualAckStatus::Applied);
    assert!(scene
        .apply(&add_gen("t", 1, "l", 0, VisualChannel::Program))
        .status
        == VisualAckStatus::Applied);
    assert!(scene
        .apply(&cmd(3, "t", 2, VisualOp::BindLayerAnchorOp {
            layer_id: "l".into(),
            in_anchor_id: "song-start".into(),
            out_anchor_id: String::new(),
        }))
        .status == VisualAckStatus::Applied);

    let before = scene.resolve(VisualChannel::Program, a_tick - 1);
    let at = scene.resolve(VisualChannel::Program, a_tick);
    assert!(before.stack.is_empty(), "layer inactive before its beat anchor");
    assert_eq!(at.stack.len(), 1, "layer lands exactly on anchor tick");
}

#[test]
fn t81_checkpoint_roundtrip_same_state() {
    // save → reopen: snapshot + restore reproduces identical layer state
    // AND the same revision (joint checkpoint semantics).
    let mut scene = Scene::new(PROJECT, EPOCH);
    scene.apply(&add_gen("t", 0, "l1", 0, VisualChannel::Program));
    scene.apply(&cmd(2, "t", 1, VisualOp::SetLayerTransformOp {
        layer_id: "l1".into(),
        transform: VisualTransform {
            x: 10.0,
            y: -5.0,
            scale_x: 2.0,
            scale_y: 0.5,
            rotation_rad: 0.25,
            opacity: 0.75,
        },
    }));
    scene.apply(&cmd(3, "t", 2, VisualOp::SetLayerFadeOp {
        layer_id: "l1".into(),
        fade_in_ticks: 480_000,
        fade_out_ticks: 960_000,
    }));
    scene.apply(&cmd(4, "t", 3, VisualOp::SetVisualAnchorOp {
        anchor_id: "a1".into(),
        kind: AnchorKind::Timecode,
        position_ticks: -1,
        position_sample: -1,
        timecode_ns: 5_000_000_000,
    }));
    let cp = scene.checkpoint();
    let json = checkpoint::to_canonical_json(&cp);
    let sha = checkpoint::payload_sha256(&json);

    // Restore into a fresh scene (simulating reopen).
    let mut reopened = Scene::new(PROJECT, EPOCH);
    reopened.restore(&checkpoint::from_json(&json).unwrap());
    let cp2 = reopened.checkpoint();
    assert_eq!(cp, cp2, "checkpoint round-trip must be identical");
    assert_eq!(checkpoint::payload_sha256(&checkpoint::to_canonical_json(&cp2)), sha);
    // Same visible state at any tick.
    for t in [0, 1_000_000, 50 * TICKS_PER_QUARTER] {
        assert_eq!(
            scene.resolve(VisualChannel::Program, t).stack.len(),
            reopened.resolve(VisualChannel::Program, t).stack.len()
        );
    }
}

#[test]
fn t81_restore_via_wire_op_rejects_bad_hash() {
    let mut scene = Scene::new(PROJECT, EPOCH);
    scene.apply(&add_gen("t", 0, "l1", 0, VisualChannel::Program));
    let cp = scene.checkpoint();
    let json = checkpoint::to_canonical_json(&cp);
    let receipt = scene.apply(&cmd(9, "t", 1, VisualOp::RestoreVisualStateOp {
        checkpoint_id: "ck-1".into(),
        state_sha256: "00".repeat(32), // wrong hash
        snapshot_json: json.clone(),
    }));
    assert_eq!(receipt.status, VisualAckStatus::Rejected);
    assert_eq!(receipt.error, VisualErrorCode::BadRequest);
    // Correct hash round-trips.
    let receipt = scene.apply(&cmd(10, "t", 1, VisualOp::RestoreVisualStateOp {
        checkpoint_id: "ck-1".into(),
        state_sha256: checkpoint::payload_sha256(&json),
        snapshot_json: json,
    }));
    assert_eq!(receipt.status, VisualAckStatus::Applied, "{}", receipt.message);
}
