//! T79 — mastering + continuation Linux-verifiable halves: real
//! BS.1770 measurement against known fixture math, gain/limiter
//! suggestions derived from measured deltas, explicit-accept lifecycle
//! (never meter-only), loudness-matched A–B audition spec, stale
//! handling on context change.

use void_producer::*;

fn sine(amp: f64, freq: f64, fs: u32, secs: f64, ch: usize) -> PcmBuffer {
    let n = (fs as f64 * secs) as usize;
    let l: Vec<f32> = (0..n)
        .map(|i| {
            (amp * (2.0 * std::f64::consts::PI * freq * i as f64 / fs as f64).sin()) as f32
        })
        .collect();
    PcmBuffer { sample_rate: fs, channels: vec![l; ch] }
}

#[test]
fn t79_fixture_math_known_lufs() {
    // Canonical fixture: 997 Hz at -20 dBFS peak, mono → -23.0 LUFS.
    let r = measure(&sine(0.1, 997.0, 48_000, 4.0, 1)).unwrap();
    assert!((r.integrated_lufs + 23.0).abs() < 0.5, "{}", r.integrated_lufs);
    // Stereo +3 LU.
    let r2 = measure(&sine(0.1, 997.0, 48_000, 4.0, 2)).unwrap();
    assert!((r2.integrated_lufs + 20.0).abs() < 0.5, "{}", r2.integrated_lufs);
    // Energy doubling = +3 dB (meter is an energy meter, not peak).
    let quiet = measure(&sine(0.05, 997.0, 48_000, 4.0, 1)).unwrap();
    assert!((quiet.integrated_lufs - r.integrated_lufs + 6.0).abs() < 0.4);
    // True peak tracks sample peak for a pure tone.
    assert!((r.true_peak_dbtp + 20.0).abs() < 0.3, "{}", r.true_peak_dbtp);
}

#[test]
fn t79_suggestions_come_from_measurement_not_meter_alone() {
    let target = MasteringTarget::default();
    // Quiet + low peak → gain, no limiter.
    let p = analyze(
        "proj",
        &"aa".repeat(32),
        "ctx-1",
        &sine(0.05, 997.0, 48_000, 4.0, 2),
        &target,
        (0, 960_000),
    )
    .unwrap();
    assert!(p.ops.iter().any(|o| matches!(o, MasteringOp::Gain { db } if *db > 0.0)));
    assert!(!p.ops.iter().any(|o| matches!(o, MasteringOp::TruePeakLimiter { .. })));
    // Audition spec is level-matched (the A-B rule).
    let ab = p.audition.as_ref().expect("audition present");
    assert!((ab.level_match_db - (target.integrated_lufs - p.measured.integrated_lufs)).abs() < 1e-9);

    // Hot + clipped peak → limiter AND gain-cut suggestions.
    let hot = sine(0.95, 997.0, 48_000, 4.0, 2);
    let p2 = analyze("proj", &"bb".repeat(32), "ctx-1", &hot, &target, (0, 960_000)).unwrap();
    assert!(p2.ops.iter().any(|o| matches!(o, MasteringOp::Gain { db } if *db < 0.0)));
}

#[test]
fn t79_explicit_accept_lifecycle() {
    let dir = tempfile::tempdir().unwrap();
    let store = MasteringStore::new(dir.path());
    let target = MasteringTarget::default();
    let mut p = analyze(
        "proj",
        &"cc".repeat(32),
        "ctx-A",
        &sine(0.05, 997.0, 48_000, 4.0, 2),
        &target,
        (0, 960_000),
    )
    .unwrap();
    assert_eq!(p.status, MasteringStatus::Ready);
    store.save(&p).unwrap();

    // Explicit accept — then terminal: no re-accept, no stale, no reject.
    p.accept().unwrap();
    store.save(&p).unwrap();
    assert!(p.accept().is_err());
    assert!(p.reject().is_err());
    assert!(p.mark_stale().is_err());
    assert_eq!(store.load(&p.proposal_id).unwrap().status, MasteringStatus::Accepted);

    // Reject path on a second record.
    let mut q = analyze("proj", &"dd".repeat(32), "ctx-A", &sine(0.05, 997.0, 48_000, 4.0, 2), &target, (0, 960_000)).unwrap();
    q.reject().unwrap();
    assert_eq!(q.status, MasteringStatus::Rejected);
    assert!(q.decided_utc.is_some());

    // Context sweep → stale; revalidation mints NEW record (supersedes),
    // the old one stays stale (never revived).
    let mut s = analyze("proj", &"ee".repeat(32), "ctx-A", &sine(0.05, 997.0, 48_000, 4.0, 2), &target, (0, 960_000)).unwrap();
    store.save(&s).unwrap();
    assert_eq!(store.sweep_stale("ctx-B").unwrap(), 1);
    let stale = store.load(&s.proposal_id).unwrap();
    assert_eq!(stale.status, MasteringStatus::Stale);
    let next = stale.revalidate("ctx-B").unwrap();
    assert_eq!(next.status, MasteringStatus::Pending);
    assert_eq!(next.supersedes.as_deref(), Some(s.proposal_id.as_str()));
    assert_eq!(store.load(&s.proposal_id).unwrap().status, MasteringStatus::Stale);
    let _ = &mut s;
}

#[test]
fn t79_wav_file_roundtrip_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let buf = sine(0.05, 440.0, 44_100, 3.0, 2);
    let path = dir.path().join("mix.wav");
    std::fs::write(&path, write_wav_f32(&buf).unwrap()).unwrap();
    let p = analyze_wav(
        "proj",
        "ctx",
        &path,
        &MasteringTarget::default(),
        (0, 960_000),
    )
    .unwrap();
    assert_eq!(p.measured.sample_rate, 44_100);
    assert_eq!(p.measured.channels, 2);
    assert!(p.measured.integrated_lufs.is_finite());
    assert_eq!(p.source_sha256.len(), 64);
}
