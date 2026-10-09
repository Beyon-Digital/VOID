//! T85-side — audio-feature mapping determinism: rms/onset/band-energy
//! over fixture PCM → normalized 0..1, bit-for-bit repeatable.

mod common;
use common::*;
use void_visfx::{analyze, AnalyzerSet, FeatureKind, MapperKind, ParamMapper};

#[test]
fn analyzer_deterministic_on_fixture_pcm() {
    // amp 0.05 carrier: impulse energy dominates frame-to-frame
    // fluctuation so the onset register is unambiguous.
    let pcm = pcm_fixture(48_000, 24_000, 0.05);
    let set = AnalyzerSet {
        frame_size: 1024,
        hop: 512,
        sample_rate: 48_000,
        features: vec![
            FeatureKind::Rms,
            FeatureKind::Peak,
            FeatureKind::Onset,
            FeatureKind::BandEnergy { band: 2, bands: 8 },
        ],
    };
    let a = analyze(&pcm, &set).unwrap();
    let b = analyze(&pcm, &set).unwrap();
    // bit-for-bit determinism — same fixture, same descriptors
    assert_eq!(a.rms, b.rms);
    assert_eq!(a.peak, b.peak);
    assert_eq!(a.onset, b.onset);
    assert_eq!(a.band_energy, b.band_energy);

    // shape + normalization invariants
    assert!(!a.rms.is_empty());
    assert!(a.rms.iter().all(|v| (0.0..=1.0).contains(v)));
    assert!(a.peak.iter().all(|v| (0.0..=1.0).contains(v)));
    // sine at 0.05 amplitude → rms ≈ 0.035 (frame away from impulse)
    let mid = a.rms[20];
    assert!(mid > 0.025 && mid < 0.045, "rms {mid}");
    // the impulse registers as the global onset maximum (normalized →
    // 1.0); it straddles the overlapping frames around sample 24000.
    let max_onset = a.onset.iter().copied().fold(0f32, f32::max);
    assert_eq!(max_onset, 1.0);
    let impulse_frame = 24_000 / 512; // 46; impulse also inside frame 45's window
    assert!(a.onset[impulse_frame - 1..=impulse_frame]
        .iter()
        .any(|v| *v == max_onset));
    // band energies normalized: sum across bands ≈ 1.0 per frame
    let (b2, bs, v) = &a.band_energy[0];
    assert_eq!((*b2, *bs), (2, 8));
    assert!(v.iter().all(|x| (0.0..=1.0).contains(x)));
}

#[test]
fn analyzer_rejects_bad_descriptors() {
    let pcm = pcm_fixture(4096, 0, 0.5);
    for set in [
        AnalyzerSet {
            frame_size: 0,
            hop: 1,
            sample_rate: 48_000,
            features: vec![],
        },
        AnalyzerSet {
            frame_size: 256,
            hop: 512,
            sample_rate: 48_000,
            features: vec![],
        },
        AnalyzerSet {
            frame_size: 256,
            hop: 128,
            sample_rate: 0,
            features: vec![],
        },
        AnalyzerSet {
            frame_size: 256,
            hop: 128,
            sample_rate: 48_000,
            features: vec![FeatureKind::BandEnergy { band: 8, bands: 8 }],
        },
    ] {
        assert!(analyze(&pcm, &set).is_err());
    }
}

#[test]
fn mappers_normalize_and_stay_deterministic() {
    // mapper kinds shape the normalized feature — deterministic
    let series = [0.0, 0.25, 0.5, 0.75, 1.0, 0.5, 0.0];
    // attack=1, release=1: instant tracking — shape-only comparison
    let mut lin = ParamMapper::new(MapperKind::Linear, 1.0, 1.0, 1.0).unwrap();
    let mut sq = ParamMapper::new(MapperKind::Sqrt, 1.0, 1.0, 1.0).unwrap();
    let mut db = ParamMapper::new(MapperKind::DbFloor, 1.0, 1.0, 1.0).unwrap();
    let l1 = lin.map_series(&series);
    lin.reset();
    let l2 = lin.map_series(&series);
    assert_eq!(l1, l2, "linear mapper non-deterministic");
    let s = sq.map_series(&series);
    let d = db.map_series(&series);
    for v in l1.iter().chain(s.iter()).chain(d.iter()) {
        assert!((0.0..=1.0).contains(v));
    }
    // sqrt expands the low end; db_floor floors at -60dB
    assert!(s[1] > l1[1]);
    assert_eq!(d[0], 0.0);
    assert_eq!(d[4], 1.0);
}

#[test]
fn attack_release_smoothing_is_bidirectional() {
    // attack/release are independent constants (rise vs fall)
    let mut m = ParamMapper::new(MapperKind::Linear, 1.0, 0.5, 0.25).unwrap();
    let up = m.step(1.0);
    assert!((up - 0.5).abs() < 1e-6, "attack step {up}");
    let up2 = m.step(1.0);
    assert!(up2 > up);
    let down = m.step(0.0);
    assert!((down - up2 * 0.75).abs() < 1e-6, "release step {down}");
    // invalid coefficients rejected
    assert!(ParamMapper::new(MapperKind::Linear, 1.0, 1.5, 0.0).is_err());
    assert!(ParamMapper::new(MapperKind::Linear, f32::NAN, 0.0, 0.0).is_err());
    // gain beyond 1 clamps to the 0..1 contract (attack=1 → instant)
    let mut g = ParamMapper::new(MapperKind::Linear, 2.0, 1.0, 1.0).unwrap();
    assert_eq!(g.step(0.8), 1.0);
}
