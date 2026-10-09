//! T87 — rational frame-rate math against a known fixture table
//! (30000/1001 NTSC at 48 kHz). Every expected number below is
//! hand-computed exact-rational, not derived by re-running the code.

use void_av::rate::{place_cue, FrameRate, FrameRounding};
use void_av_tests::spec;

const SR: u32 = 48_000;
const NTSC: FrameRate = FrameRate {
    num: 30000,
    den: 1001,
};

/// samples-per-frame at 48 kHz NTSC = 48000*1001/30000 = 1601.6 — the
/// repeating non-integer that makes NTSC scheduling nontrivial.
#[test]
fn t87_ntsc_frame_start_fixture() {
    // frame_index → exact start sample (floor of index*1601.6)
    let floor: &[(u64, u64)] = &[
        (0, 0),
        (1, 1601),
        (2, 3203),
        (5, 8008),
        (10, 16016),
        (15, 24024),
        (30, 48048),
        (60, 96096),
        (300, 480480),
        (1799, 2_881_278),
    ];
    for (f, s) in floor {
        assert_eq!(
            NTSC.frame_start_sample(*f, SR, FrameRounding::Floor),
            *s,
            "frame {f} floor"
        );
    }
    // Nearest-ties-away: 1601.6 → 1602, 48048.0 → 48048.
    assert_eq!(
        NTSC.frame_start_sample(1, SR, FrameRounding::NearestTiesAway),
        1602
    );
    assert_eq!(
        NTSC.frame_start_sample(30, SR, FrameRounding::NearestTiesAway),
        48048
    );
    assert_eq!(
        NTSC.frame_start_sample(2, SR, FrameRounding::NearestTiesAway),
        3203
    );
}

#[test]
fn t87_sample_to_frame_index_boundaries() {
    // A sample ON a frame boundary belongs to the new frame — and the
    // floor-start convention means frame 1's first sample is
    // floor(1601.6) = 1601, so 1601 is already inside frame 1 even
    // though 1601*30000/(48000*1001) < 1 (fractional ideal boundary).
    assert_eq!(NTSC.frame_index_at_sample(0, SR), 0);
    assert_eq!(NTSC.frame_index_at_sample(1600, SR), 0);
    assert_eq!(NTSC.frame_index_at_sample(1601, SR), 1);
    assert_eq!(NTSC.frame_index_at_sample(1602, SR), 1);
    assert_eq!(NTSC.frame_index_at_sample(48047, SR), 29);
    assert_eq!(NTSC.frame_index_at_sample(48048, SR), 30);
    // End of one minute at 30000/1001: sample 2880879 → frame 1798.
    assert_eq!(NTSC.frame_index_at_sample(2_880_879, SR), 1798);
}

#[test]
fn t87_one_minute_is_1799_frames_not_1800() {
    // The classic NTSC trap: 60 s @ 29.97 fps needs 1799 frames
    // (1800 frames of 30000/1001 is 60.06 s of media).
    assert_eq!(NTSC.frames_covering_samples(60 * SR as u64, SR), 1799);
    assert_eq!(NTSC.frames_covering_samples(SR as u64, SR), 30);
    assert_eq!(NTSC.frames_covering_samples(1, SR), 1);
}

#[test]
fn t87_ntsc24_fixture() {
    let f = FrameRate::new(24000, 1001).unwrap();
    // 1 s of 48 kHz audio → ceil(23.976…) = 24 frames.
    assert_eq!(f.frames_covering_samples(SR as u64, SR), 24);
    // frame 24 starts exactly at sample 48048.
    assert_eq!(f.frame_start_sample(24, SR, FrameRounding::Floor), 48048);
    assert_eq!(f.frame_index_at_sample(48047, SR), 23);
}

#[test]
fn t87_cue_placement_declared_tolerance() {
    // Cue exactly on frame-15 boundary: zero drift.
    let c = place_cue(24024, &NTSC, SR, FrameRounding::NearestTiesAway);
    assert_eq!(c.frame_index, 15);
    assert_eq!(c.offset_samples, 0);
    assert!(c.within_cue_tolerance);
    // Cue mid-frame at 25000: offset 25000-24024=976 ≤ one frame (1602).
    let c = place_cue(25000, &NTSC, SR, FrameRounding::NearestTiesAway);
    assert_eq!(c.frame_index, 15);
    assert_eq!(c.offset_samples, 976);
    assert!(c.within_cue_tolerance);
}

#[test]
fn t87_spec_plan_carries_frame_math_into_dto() {
    let s = spec(
        "00000000-0000-4000-8000-0000000000e1",
        "00000000-0000-4000-8000-0000000000c1",
        "00000000-0000-4000-8000-0000000000aa",
    );
    let p = s.frame_plan().unwrap();
    assert_eq!(p.video_total_frames, "30");
    assert_eq!(p.video_range_frames, "30");
    assert_eq!(p.audio_total_samples, "48000");
    assert_eq!(p.frame_rate_num, 30000);
    assert_eq!(p.frame_rate_den, 1001);
    // Cue evidence is precomputed into the plan — the runner never
    // re-derives it.
    let c0 = &p.cues[0];
    assert_eq!(c0.cue_id, "boundary");
    assert_eq!(c0.placement.frame_index, 15);
    assert_eq!(c0.placement.offset_samples, 0);
    assert!(c0.placement.within_cue_tolerance);
    let c1 = &p.cues[1];
    assert_eq!(c1.placement.offset_samples, 976);
    // Serde DTO keeps big numbers decimal-string shaped.
    let v = serde_json::to_value(&p).unwrap();
    assert_eq!(v["videoTotalFrames"], "30");
    assert_eq!(v["audioTotalSamples"], "48000");
}

#[test]
fn t87_duration_seconds_is_exact_micro_rational() {
    let s = spec(
        "00000000-0000-4000-8000-0000000000e1",
        "00000000-0000-4000-8000-0000000000c1",
        "00000000-0000-4000-8000-0000000000aa",
    );
    let p = s.frame_plan().unwrap();
    assert_eq!(s.duration_seconds(&p).unwrap(), "1.000000");
    // 48001 samples at 48 kHz = 1.000020833… s → nearest µs 1.000021.
    let mut s2 = s.clone();
    s2.range_samples = "48001".into();
    let p2 = s2.frame_plan().unwrap();
    assert_eq!(s2.duration_seconds(&p2).unwrap(), "1.000021");
}

#[test]
fn t87_tail_policy_explicit() {
    let mut s = spec(
        "00000000-0000-4000-8000-0000000000e1",
        "00000000-0000-4000-8000-0000000000c1",
        "00000000-0000-4000-8000-0000000000aa",
    );
    s.cues.clear();
    s.tail = void_av::AvTailPolicy::Samples { samples: 4800 }; // 0.1 s
    let p = s.frame_plan().unwrap();
    assert_eq!(p.audio_total_samples, "52800");
    // ceil(52800*30000/48048000) = ceil(32.967) = 33
    assert_eq!(p.video_total_frames, "33");
    s.tail = void_av::AvTailPolicy::Frames { frames: 5 };
    let p = s.frame_plan().unwrap();
    assert_eq!(p.video_total_frames, "35");
    assert_eq!(p.audio_total_samples, "48000");
}
