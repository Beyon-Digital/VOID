//! T88 — adversarial: denied/missing codec, malformed media, shell-like
//! filenames, worker timeout, full disk, argv-only invocation, env
//! hygiene, bounded output. Every negative must produce a typed error
//! or a `failed` job — never a fake success and never leaked output.

use std::time::Instant;
use void_av::{AvError, AvRunner, AvStep, FfmpegRunner};
use void_av_tests::*;
use void_jobs::JobStatus;

const J: &str = "00000000-0000-4000-8000-0000000000e1";
const C: &str = "00000000-0000-4000-8000-0000000000c1";

#[test]
fn t88_argv_is_pure_array_no_shell() {
    let runner = FfmpegRunner::new("/usr/bin/ffmpeg");
    let s = spec(J, C, "00000000-0000-4000-8000-0000000000aa");
    let plan = s.frame_plan().unwrap();
    let resolved: Vec<void_av::ResolvedInput> = s
        .inputs
        .iter()
        .map(|i| match i {
            void_av::AvInput::LavfiTest { src, .. } => void_av::ResolvedInput {
                input: i.clone(),
                argv_path: format!("lavfi:{src}"),
            },
            other => void_av::ResolvedInput {
                input: other.clone(),
                argv_path: "/scoped/path".into(),
            },
        })
        .collect();
    let argv = runner
        .build_argv(
            &s,
            &plan,
            &resolved,
            std::path::Path::new("/staging/out.mkv"),
        )
        .unwrap();
    // argv[0] is basename-only, never a path.
    assert_eq!(argv[0], "ffmpeg");
    // No shell metacharacters anywhere in the argv — each input is one
    // argv element; there is no joined command line to inject into.
    for a in &argv {
        assert!(
            !a.contains(';') && !a.contains("&&") && !a.contains('|') && !a.contains('`'),
            "argv element has shell metachars: {a}"
        );
    }
    assert!(argv.contains(&"-frames:v".to_string()));
    assert!(argv.contains(&"30000/1001".to_string()));
    assert!(argv.contains(&"-fs".to_string()));
}

#[test]
fn t88_shellish_output_names_rejected() {
    for bad in [
        "a;b",
        "../x",
        "a b",
        "$(rm -rf /)",
        "-i",
        ".hidden",
        "x\x00y",
    ] {
        let mut s = spec(J, C, "00000000-0000-4000-8000-0000000000aa");
        s.output_name = bad.into();
        assert!(s.validate().is_err(), "accepted unsafe output_name {bad:?}");
    }
}

#[test]
fn t88_unknown_codec_is_typed_codec_unavailable() {
    let (tmp, project_id) = tmp_project();
    make_checkpoint(tmp.path(), C, &project_id);
    let runner = AvRunner::memory(tmp.path()).unwrap();
    let mut s = spec(J, C, &project_id);
    s.codec_id = "hevc_silk_mkv".into();
    // submit() does not gate codecs (offline runner may lack the tool);
    // the run settles as failed with a typed error in the log path.
    runner.submit(&s).unwrap();
    let st = runner.run(J, &fake_tools(&[])).unwrap();
    assert_eq!(st, JobStatus::Failed);
    // The typed check is CodecUnavailable at the codec::codec layer:
    match void_av::codec("hevc_silk_mkv") {
        Err(AvError::CodecUnavailable(m)) => assert!(m.contains("hevc_silk_mkv")),
        other => panic!("expected CodecUnavailable, got {other:?}"),
    }
    assert!(!tmp.path().join("exports").join(format!("av-{J}")).exists());
}

#[test]
fn t88_missing_encoder_in_build_is_codec_unavailable() {
    // Fake reports only `ffv1` — flac is gone from this "build".
    let exe = fake_ffmpeg();
    let out = std::process::Command::new(&exe)
        .args(["-hide_banner", "-encoders"])
        .env("VOID_FAKE_FFMPEG_ENCODERS", "ffv1")
        .output()
        .unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    let set = void_av::ffmpeg::parse_encoders(&text);
    assert!(set.contains("ffv1") && !set.contains("flac"));
    let row = void_av::codec("ffv1_flac_mkv").unwrap();
    match void_av::require_encoders(row, &|n| set.contains(n)) {
        Err(AvError::CodecUnavailable(m)) => assert!(m.contains("flac")),
        other => panic!("expected CodecUnavailable, got {other:?}"),
    }
}

#[test]
fn t88_absent_ffmpeg_is_codec_unavailable() {
    let runner = FfmpegRunner::new("/nonexistent/ffmpeg-not-here");
    match runner.detect_encoders() {
        Err(AvError::CodecUnavailable(m)) => assert!(m.contains("ffmpeg-not-here")),
        other => panic!("expected CodecUnavailable, got {other:?}"),
    }
    // And through submit_checked — typed error, never submitted.
    let (tmp, project_id) = tmp_project();
    make_checkpoint(tmp.path(), C, &project_id);
    let runner = AvRunner::memory(tmp.path()).unwrap();
    let tools = void_av::AvTools::new(
        "/nonexistent/ffmpeg-not-here",
        "/nonexistent/ffprobe-not-here",
    );
    let s = spec(J, C, &project_id);
    match runner.submit_checked(&s, &tools) {
        Err(AvError::CodecUnavailable(_)) => {}
        other => panic!("expected CodecUnavailable at submit, got {other:?}"),
    }
    assert_eq!(runner.list(&project_id).unwrap().len(), 0);
}

#[test]
fn t88_malformed_media_fails_verify_no_publish() {
    let (tmp, project_id) = tmp_project();
    make_checkpoint(tmp.path(), C, &project_id);
    let runner = AvRunner::memory(tmp.path()).unwrap();
    let s = spec(J, C, &project_id);
    runner.submit(&s).unwrap();
    let st = runner
        .run(J, &fake_tools(&[("VOID_FAKE_FFMPEG_MODE", "badheader")]))
        .unwrap();
    assert_eq!(st, JobStatus::Failed);
    assert!(!tmp.path().join("exports").join(format!("av-{J}")).exists());
    // Staging cleaned too — nothing partial remains.
    assert!(!tmp.path().join("staging").join(format!("av-{J}")).exists());
}

#[test]
fn t88_timeout_kills_encoder() {
    let (tmp, project_id) = tmp_project();
    make_checkpoint(tmp.path(), C, &project_id);
    let runner = AvRunner::memory(tmp.path()).unwrap();
    let mut s = spec(J, C, &project_id);
    s.timeout_ms = "300".into();
    runner.submit(&s).unwrap();
    let t0 = Instant::now();
    let st = runner
        .run(J, &fake_tools(&[("VOID_FAKE_FFMPEG_MODE", "hang")]))
        .unwrap();
    let el = t0.elapsed().as_millis();
    assert_eq!(st, JobStatus::Failed);
    assert!(el < 5000, "timeout kill took too long: {el}ms");
    assert!(!tmp.path().join("exports").join(format!("av-{J}")).exists());
}

#[test]
fn t88_output_size_cap_enforced() {
    let (tmp, project_id) = tmp_project();
    make_checkpoint(tmp.path(), C, &project_id);
    let runner = AvRunner::memory(tmp.path()).unwrap();
    let mut s = spec(J, C, &project_id);
    // 30-frame artifact is ~31 KB; cap it at 1 KB → -fs forces failure.
    s.max_output_bytes = "1024".into();
    runner.submit(&s).unwrap();
    let st = runner.run(J, &fake_tools(&[])).unwrap();
    assert_eq!(st, JobStatus::Failed);
    assert!(!tmp.path().join("exports").join(format!("av-{J}")).exists());
}

#[test]
fn t88_underreported_frames_fail_verify() {
    let (tmp, project_id) = tmp_project();
    make_checkpoint(tmp.path(), C, &project_id);
    let runner = AvRunner::memory(tmp.path()).unwrap();
    let s = spec(J, C, &project_id);
    runner.submit(&s).unwrap();
    let st = runner
        .run(J, &fake_tools(&[("VOID_FAKE_FFMPEG_MODE", "underframes")]))
        .unwrap();
    assert_eq!(st, JobStatus::Failed);
}

#[test]
fn t88_env_is_cleared_but_allowlist_passes() {
    // envdump proves env_clear: the fake records how many env vars it
    // saw. Only the declared VOID_FAKE_FFMPEG_MODE may be visible (1) —
    // nothing from this (large) test process leaks through.
    let (tmp, project_id) = tmp_project();
    make_checkpoint(tmp.path(), C, &project_id);
    let runner = AvRunner::memory(tmp.path()).unwrap();
    let s = spec(J, C, &project_id);
    runner.submit(&s).unwrap();
    let st = runner
        .run(J, &fake_tools(&[("VOID_FAKE_FFMPEG_MODE", "envdump")]))
        .unwrap();
    assert_eq!(st, JobStatus::Succeeded);
    let art = tmp
        .path()
        .join("exports")
        .join(format!("av-{J}"))
        .join("viz-v1.mkv");
    // Probe the published artifact directly with the fake and check the
    // recorded env_count field.
    // env_count lives in the VFMT header — read it off the bytes.
    let data = std::fs::read(&art).unwrap();
    let header = String::from_utf8_lossy(&data[..data.len().min(4096)]);
    let env_line = header
        .split("\"env_count\":")
        .nth(1)
        .and_then(|t| t.split([',', '}']).next())
        .unwrap()
        .to_string();
    assert_eq!(
        env_line.parse::<u64>().unwrap(),
        1,
        "encoder saw env vars beyond allowlist"
    );
}

#[test]
fn t88_full_disk_refuses_without_partial_publish() {
    // Parent builds the container; the helper applies RLIMIT_FSIZE and
    // runs the encode in its own process.
    let (tmp, project_id) = tmp_project();
    make_checkpoint(tmp.path(), C, &project_id);
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_av_diskfull"))
        .arg(tmp.path())
        .output()
        .expect("spawn av_diskfull");
    assert!(
        out.status.success(),
        "diskfull helper failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(String::from_utf8_lossy(&out.stdout).contains("DISKFULL_OK"));
}

#[test]
fn t88_crash_boundaries_leave_no_partial_output() {
    // Every declared pipeline step is a crash boundary: trip each one,
    // expect `failed` + zero bytes published or staged.
    for step in [
        AvStep::StageDir,
        AvStep::ResolveInputs,
        AvStep::Render,
        AvStep::Verify,
        AvStep::Provenance,
        AvStep::Rename,
        AvStep::Publish,
    ] {
        let (tmp, project_id) = tmp_project();
        make_checkpoint(tmp.path(), C, &project_id);
        let runner = AvRunner::memory(tmp.path()).unwrap();
        runner.set_failpoints(vec![step]);
        let s = spec(J, C, &project_id);
        runner.submit(&s).unwrap();
        let st = runner.run(J, &fake_tools(&[])).unwrap();
        assert_eq!(st, JobStatus::Failed, "step {step:?} did not fail");
        assert!(
            !tmp.path().join("exports").join(format!("av-{J}")).exists(),
            "partial output published after crash at {step:?}"
        );
        assert!(
            !tmp.path().join("staging").join(format!("av-{J}")).exists(),
            "staging leaked after crash at {step:?}"
        );
    }
}

#[test]
fn t88_input_hash_pinning_enforced() {
    // A CheckpointFile input whose declared sha256 does not match the
    // bytes on disk is a hard error at begin() — never substituted.
    let (tmp, project_id) = tmp_project();
    let bad_sha = "b".repeat(64);
    let _vbin = make_checkpoint(tmp.path(), C, &project_id);
    let runner = AvRunner::memory(tmp.path()).unwrap();
    let s = spec_with_checkpoint_inputs(J, C, &project_id, &bad_sha);
    runner.submit(&s).unwrap();
    let st = runner.run(J, &fake_tools(&[])).unwrap();
    assert_eq!(st, JobStatus::Failed);
}
