//! T89 — end-to-end: submit → run → publish through the void-jobs state
//! machine, provenance completeness, cue-drift evidence, cancel/quarantine,
//! checkpoint-pinned inputs, and the real-ffmpeg path when the tool is
//! installed (feature-detected, per lane assignment).

use void_av::{AvRunner, AvTools};
use void_av_tests::*;
use void_jobs::JobStatus;

const J: &str = "00000000-0000-4000-8000-0000000000e1";
const C: &str = "00000000-0000-4000-8000-0000000000c1";

fn provenance(tmp: &tempfile::TempDir) -> serde_json::Value {
    let p = tmp
        .path()
        .join("exports")
        .join(format!("av-{J}"))
        .join("provenance.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}

#[test]
fn t89_happy_path_publishes_with_complete_provenance() {
    let (tmp, project_id) = tmp_project();
    make_checkpoint(tmp.path(), C, &project_id);
    let runner = AvRunner::memory(tmp.path()).unwrap();
    let s = spec(J, C, &project_id);
    runner.submit(&s).unwrap();
    assert_eq!(runner.status(J).unwrap().status, JobStatus::Queued);

    let st = runner.run(J, &fake_tools(&[])).unwrap();
    assert_eq!(st, JobStatus::Succeeded);
    let rec = runner.status(J).unwrap();
    assert_eq!(rec.status, JobStatus::Succeeded);
    assert!(!rec.quarantined);
    assert_eq!(rec.result.as_ref().unwrap()["kind"], "AvExportResult");

    let dir = tmp.path().join("exports").join(format!("av-{J}"));
    assert!(dir.join("viz-v1.mkv").is_file());
    assert!(dir.join("provenance.json").is_file());
    assert!(dir.join("result.json").is_file());
    // Staging dir is gone after publish.
    assert!(!tmp.path().join("staging").join(format!("av-{J}")).exists());

    // Provenance completeness — every field the spec demands.
    let p = provenance(&tmp);
    assert_eq!(p["formatVersion"], 1);
    assert_eq!(p["jobId"], J);
    assert_eq!(p["checkpointId"], C);
    assert_eq!(p["sourceRevision"], "7");
    for k in ["configSha256", "argvSha256", "checkpointManifestSha256"] {
        assert_eq!(p[k].as_str().unwrap().len(), 64, "{k} not sha256");
    }
    assert_eq!(p["codecId"], "ffv1_flac_mkv");
    assert_eq!(p["codecRights"], "cleared");
    assert!(!p["toolVersions"].as_array().unwrap().is_empty());
    let art = &p["artifact"];
    assert_eq!(art["file"], "viz-v1.mkv");
    assert_eq!(art["videoFrames"], "30");
    assert_eq!(art["declaredVideoFrames"], "30");
    assert_eq!(art["audioSamples"], "48000");
    assert_eq!(art["declaredAudioSamples"], "48000");
    assert_eq!(art["sha256"].as_str().unwrap().len(), 64);
    // Artifact sha matches actual bytes on disk.
    let actual = void_assets::file_sha256(&dir.join("viz-v1.mkv")).unwrap();
    assert_eq!(art["sha256"], actual);
    // Cue evidence: declared tolerance = one frame.
    let cues = p["cues"].as_array().unwrap();
    assert_eq!(cues.len(), 2);
    assert_eq!(cues[0]["cueId"], "boundary");
    assert_eq!(cues[0]["offsetSamples"], "0");
    assert_eq!(cues[0]["withinTolerance"], true);
    assert_eq!(cues[1]["offsetSamples"], "976");
    assert_eq!(cues[1]["withinTolerance"], true);
    // Honest nondeterminism note, never a bit-identical claim.
    assert!(p["nondeterminismNote"]
        .as_str()
        .unwrap()
        .contains("bit-identical rerenders are NOT claimed"));
    assert_eq!(p["encoderExe"], "void-fake-ffmpeg");
}

#[test]
fn t89_checkpoint_pinned_inputs_flow() {
    let (tmp, project_id) = tmp_project();
    let vbin = make_checkpoint(tmp.path(), C, &project_id);
    let runner = AvRunner::memory(tmp.path()).unwrap();
    let s = spec_with_checkpoint_inputs(J, C, &project_id, &vbin);
    runner.submit(&s).unwrap();
    let st = runner.run(J, &fake_tools(&[])).unwrap();
    assert_eq!(st, JobStatus::Succeeded);
}

#[test]
fn t89_cancel_queued_and_running() {
    // Queued cancel: settles Cancelled before any run.
    let (tmp, project_id) = tmp_project();
    make_checkpoint(tmp.path(), C, &project_id);
    let runner = AvRunner::memory(tmp.path()).unwrap();
    let s = spec(J, C, &project_id);
    runner.submit(&s).unwrap();
    assert_eq!(runner.cancel(J).unwrap(), JobStatus::Cancelled);
    assert!(
        !tmp.path().join("exports").exists()
            || std::fs::read_dir(tmp.path().join("exports"))
                .unwrap()
                .next()
                .is_none()
    );

    // Running cancel: hang-mode encoder is really spawned, then killed
    // when the token trips (same shape as void-export's CancelFirst,
    // but through the real subprocess kill path, not a stub).
    let (tmp2, project_id2) = tmp_project();
    make_checkpoint(tmp2.path(), C, &project_id2);
    let runner2 = AvRunner::memory(tmp2.path()).unwrap();
    let mut s2 = spec(J, C, &project_id2);
    s2.timeout_ms = "30000".into();
    runner2.submit(&s2).unwrap();
    let tools = fake_tools(&[("VOID_FAKE_FFMPEG_MODE", "hang")]);
    let encoder = CancelFirstRunner(tools.encoder);
    let st = runner2.run_parts(J, &encoder, &tools.probe).unwrap();
    assert_eq!(st, JobStatus::Cancelled);
    assert!(!tmp2.path().join("exports").join(format!("av-{J}")).exists());
    assert!(!tmp2.path().join("staging").join(format!("av-{J}")).exists());
}

#[test]
fn t89_late_result_is_quarantined_never_published() {
    // §6 late-result rule: a result landing after terminal state is
    // quarantined by JobDb — and no artifact may stand.
    let (tmp, project_id) = tmp_project();
    make_checkpoint(tmp.path(), C, &project_id);
    let runner = AvRunner::memory(tmp.path()).unwrap();
    let s = spec(J, C, &project_id);
    runner.submit(&s).unwrap();
    runner.cancel(J).unwrap();
    let st = runner
        .db()
        .record_result(J, &serde_json::json!({"kind":"AvExportResult"}))
        .unwrap();
    assert_eq!(st, JobStatus::Cancelled);
    assert!(runner.status(J).unwrap().quarantined);
    assert!(!tmp.path().join("exports").join(format!("av-{J}")).exists());
}

#[test]
fn t89_audio_drift_within_tolerance_is_recorded_not_failed() {
    // jitter mode: measured audio duration is +7 samples — inside the
    // declared one-frame audio tolerance, so verify passes AND the
    // drift is recorded in provenance (expected-vs-measured).
    let (tmp, project_id) = tmp_project();
    make_checkpoint(tmp.path(), C, &project_id);
    let runner = AvRunner::memory(tmp.path()).unwrap();
    let s = spec(J, C, &project_id);
    runner.submit(&s).unwrap();
    let st = runner
        .run(J, &fake_tools(&[("VOID_FAKE_FFMPEG_MODE", "jitter")]))
        .unwrap();
    assert_eq!(st, JobStatus::Succeeded);
    let p = provenance(&tmp);
    assert_eq!(p["artifact"]["audioSamples"], "48007");
    assert_eq!(p["artifact"]["declaredAudioSamples"], "48000");
}

#[test]
fn t89_audio_drift_beyond_tolerance_fails() {
    let (tmp, project_id) = tmp_project();
    make_checkpoint(tmp.path(), C, &project_id);
    let runner = AvRunner::memory(tmp.path()).unwrap();
    let s = spec(J, C, &project_id);
    runner.submit(&s).unwrap();
    let st = runner
        .run(J, &fake_tools(&[("VOID_FAKE_FFMPEG_MODE", "bigdrift")]))
        .unwrap();
    assert_eq!(st, JobStatus::Failed);
    assert!(!tmp.path().join("exports").join(format!("av-{J}")).exists());
}

#[test]
fn t89_double_submit_and_status_shapes() {
    let (tmp, project_id) = tmp_project();
    make_checkpoint(tmp.path(), C, &project_id);
    let runner = AvRunner::memory(tmp.path()).unwrap();
    let s = spec(J, C, &project_id);
    runner.submit(&s).unwrap();
    assert!(runner.submit(&s).is_err(), "duplicate job id accepted");
    let jobs = runner.list(&project_id).unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].spec.kind, void_jobs::JobKind::AvExport);
    assert_eq!(jobs[0].spec.runtime_id, "void-av");
}

/// Renderer that trips the cancel token the moment its subprocess
/// render starts — drives the real kill-on-cancel path through the
/// hang-mode fake binary.
struct CancelFirstRunner(void_av::FfmpegRunner);

impl void_av::AvRenderer for CancelFirstRunner {
    fn render(
        &self,
        spec: &void_av::AvExportSpec,
        plan: &void_av::AvFramePlan,
        resolved: &[void_av::ResolvedInput],
        staging_dir: &std::path::Path,
        out_name: &str,
        cancel: &void_export::CancelToken,
    ) -> void_av::Result<void_av::RenderOutcome> {
        cancel.cancel();
        self.0
            .render(spec, plan, resolved, staging_dir, out_name, cancel)
    }
    fn name(&self) -> &'static str {
        self.0.name()
    }
    fn exe_name(&self) -> String {
        self.0.exe_name()
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self.0.as_any()
    }
}

/// Real-ffmpeg path — feature-detected per the lane assignment ("real
/// ffmpeg used iff present"). Missing tool or missing encoder prints
/// the gap and returns; it never fabricates a pass.
#[test]
fn t89_real_ffmpeg_smoke_when_present() {
    let (Some(ff), Some(fp)) = (real_ffmpeg(), real_ffprobe()) else {
        eprintln!("SKIP real-ffmpeg path: ffmpeg/ffprobe not installed");
        return;
    };
    let set = void_av::FfmpegRunner::new(ff.clone())
        .detect_encoders()
        .unwrap_or_default();
    if !set.contains("ffv1") || !set.contains("flac") {
        eprintln!("SKIP real-ffmpeg path: build lacks ffv1/flac encoders");
        return;
    }
    let (tmp, project_id) = tmp_project();
    make_checkpoint(tmp.path(), C, &project_id);
    let runner = AvRunner::memory(tmp.path()).unwrap();
    let mut s = spec(J, C, &project_id);
    s.cues.clear(); // lavfi fixture has no declared cues needed here
    runner.submit(&s).unwrap();
    let tools = AvTools::new(&ff, &fp);
    let st = runner.run(J, &tools).unwrap();
    assert_eq!(st, JobStatus::Succeeded, "real ffmpeg encode failed");
    let p = provenance(&tmp);
    assert_eq!(p["artifact"]["videoFrames"], "30");
    assert_eq!(p["codecId"], "ffv1_flac_mkv");
    assert!(p["toolVersions"][0]
        .as_str()
        .unwrap()
        .starts_with("ffmpeg version"));
}
