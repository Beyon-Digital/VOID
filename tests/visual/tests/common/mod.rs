#![allow(dead_code)] // shared fixtures: not every binary uses every helper
//! Shared fixtures for the visual test suite.

use void_visual::*;

pub const PROJECT: &str = "proj-visual-tests";
pub const EPOCH: u64 = 7;

/// Command envelope with sensible defaults; caller bumps revision.
pub fn cmd(n: u64, tx: &str, rev: u64, op: VisualOp) -> VisualCommand {
    VisualCommand {
        command_id: format!("cmd-{tx}-{n}"),
        transaction_id: tx.to_string(),
        project_id: PROJECT.to_string(),
        engine_epoch: EPOCH,
        expected_revision: rev,
        op,
    }
}

pub fn add_gen(tx: &str, rev: u64, layer_id: &str, index: i32, channel: VisualChannel) -> VisualCommand {
    cmd(
        rev * 100 + 1,
        tx,
        rev,
        VisualOp::AddVisualLayerOp {
            layer_id: layer_id.to_string(),
            kind: VisualLayerKind::Generator,
            name: layer_id.to_string(),
            index,
            channel,
            generator: Some(GeneratorSpec {
                preset: "color-bars".to_string(),
                seed: 1,
                param_json: "{}".to_string(),
            }),
        },
    )
}

/// Deterministic RGBA test image written to `path` (PNG).
pub fn write_png(path: &std::path::Path, w: u32, h: u32, seed: u8) -> String {
    use sha2::{Digest, Sha256};
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            px.extend_from_slice(&[
                (x as u8).wrapping_add(seed),
                (y as u8).wrapping_add(seed),
                seed,
                255,
            ]);
        }
    }
    let img = image::RgbaImage::from_raw(w, h, px.clone()).unwrap();
    img.save(path).unwrap();
    // sha of the FILE bytes (what verify_asset checks).
    let bytes = std::fs::read(path).unwrap();
    void_hex(&Sha256::digest(&bytes))
}

pub fn void_hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

/// ClockSnapshot advancing `sequence`/`timeline_sample`/`device_sample_counter`.
pub fn clock(seq: u64, sample: i64, dev: i64) -> ClockSnapshot {
    ClockSnapshot {
        project_id: PROJECT.to_string(),
        engine_epoch: EPOCH,
        timeline_sample: sample,
        device_sample_counter: dev,
        sample_rate: 48_000,
        transport_state: void_visual::clock::TRANSPORT_PLAYING,
        loop_start_ticks: 0,
        loop_end_ticks: -1,
        tempo_map_revision: 0,
        sequence: seq,
        host_clock_ns: seq * 1_000_000,
    }
}

/// Play-state snapshot at musical `tick` under a constant `bpm` tempo.
/// sample = tick * 60 * sr / (bpm * TPQ).
pub fn clock_at_tick(seq: u64, tick: i64, bpm: f64) -> ClockSnapshot {
    let sample = (tick as f64 * 60.0 * 48_000.0 / (bpm * TICKS_PER_QUARTER as f64)).round() as i64;
    clock(seq, sample, sample)
}
