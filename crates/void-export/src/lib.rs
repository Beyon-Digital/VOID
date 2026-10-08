//! void-export — render/export job pipeline (W10 non-native,
//! CONTRACTS.md §7 render/export contract).
//!
//! - Inputs are immutable: a verified checkpoint id, its asset hashes and
//!   the selected timeline range — edits that land after submit can never
//!   change what the render reads.
//! - Output stages under `staging/export-<jobId>/` and publishes by
//!   rename into `exports/<jobId>/` only after verification — an
//!   incomplete file is never a result.
//! - Process invocation is argv-only (`ArgvRenderer`: `Command` + argv
//!   vector, `env_clear`, never a shell) with scoped filenames.
//! - Frame counts and tail duration are explicit: `ExportSpec::frame_plan`
//!   computes them in exact integer arithmetic (nearest, ties away).
//! - Lifecycle rides the `void-jobs` state machine; late/cancelled
//!   results quarantine instead of applying.
//! - Every publish carries `provenance.json`: params, checkpoint input,
//!   artifact SHA-256 and tool versions.

mod error;
mod job;
mod layout;
mod provenance;
mod renderer;
mod session;
mod spec;
mod wav;

pub use error::{ExportError, Result};
pub use job::{job_spec_for, ExportRunner};
pub use layout::{
    export_dir, exports_dir, list_exports, staging_dir, EXPORTS_DIR, PROVENANCE_FILE,
    RESULT_FILE, STAGING_PREFIX,
};
pub use provenance::{ArtifactRecord, ExportProvenance, ToolRecord};
pub use renderer::{
    argv_sha256, build_argv, ArgvRenderer, CancelToken, RenderOutcome, Renderer,
};
pub use session::{ExportReceipt, ExportSession, ExportStep};
pub use spec::{
    range_to_frames, sanitize_output_name, ticks_to_frames, BitDepth, ChannelLayout,
    ExportFormat, ExportSpec, FramePlan, TailPolicy, TempoSegment, APPROVED_SAMPLE_RATES,
    MAX_RENDER_FRAMES, TICKS_PER_QUARTER,
};
pub use wav::{probe_wav, verify_midi, verify_wav, WavInfo};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use void_jobs::JobStatus;

    /// A spec-validating renderer fixture: writes a structurally real WAV
    /// (or MThd MIDI) of exactly the declared total frames. Test asset —
    /// it produces bytes shaped like the spec, never presented as a real
    /// render.
    struct WavFixture {
        fail: Option<String>,
    }

    impl WavFixture {
        fn ok() -> Self {
            Self { fail: None }
        }
        fn failing(msg: &str) -> Self {
            Self {
                fail: Some(msg.to_string()),
            }
        }
    }

    /// Minimal WAV writer mirroring the fixture in wav.rs tests.
    fn write_wav(
        path: &Path,
        channels: u16,
        sr: u32,
        bits: u16,
        fmt: u16,
        frames: u64,
    ) -> std::io::Result<()> {
        let bpf = (channels as u64) * ((bits as u64) / 8);
        let len = frames * bpf;
        let mut f = std::fs::File::create(path)?;
        use std::io::Write;
        f.write_all(b"RIFF")?;
        f.write_all(&((36 + len) as u32).to_le_bytes())?;
        f.write_all(b"WAVEfmt ")?;
        f.write_all(&16u32.to_le_bytes())?;
        f.write_all(&fmt.to_le_bytes())?;
        f.write_all(&channels.to_le_bytes())?;
        f.write_all(&sr.to_le_bytes())?;
        f.write_all(&(sr * channels as u32 * (bits as u32 / 8)).to_le_bytes())?;
        f.write_all(&((channels * (bits / 8)) as u16).to_le_bytes())?;
        f.write_all(&bits.to_le_bytes())?;
        f.write_all(b"data")?;
        f.write_all(&(len as u32).to_le_bytes())?;
        let z = [0u8; 8192];
        let mut left = len;
        while left > 0 {
            let n = (left as usize).min(z.len());
            f.write_all(&z[..n])?;
            left -= n as u64;
        }
        Ok(())
    }

    impl Renderer for WavFixture {
        fn render(
            &self,
            spec: &ExportSpec,
            plan: &FramePlan,
            _checkpoint_dir: &Path,
            staging_dir: &Path,
            out_name: &str,
            cancel: &CancelToken,
        ) -> Result<RenderOutcome> {
            cancel.check()?;
            if let Some(msg) = &self.fail {
                return Err(ExportError::RendererFailed(msg.clone()));
            }
            let total: u64 = plan.total_frames.parse().unwrap();
            let file = staging_dir.join(out_name);
            match spec.format {
                ExportFormat::Wav => {
                    let (fmt, bits) = match spec.bit_depth.unwrap() {
                        BitDepth::Pcm16 => (1, 16),
                        BitDepth::Pcm24 => (1, 24),
                        BitDepth::Float32 => (3, 32),
                    };
                    let ch = match spec.channels.unwrap() {
                        ChannelLayout::Mono => 1,
                        ChannelLayout::Stereo => 2,
                    };
                    write_wav(
                        &file,
                        ch,
                        spec.sample_rate.unwrap(),
                        bits,
                        fmt,
                        total,
                    )
                    .map_err(ExportError::Io)?;
                }
                ExportFormat::Midi => {
                    std::fs::write(&file, b"MThd\x00\x00\x00\x06\x00\x01\x00\x01\x03\xc0")
                        .map_err(ExportError::Io)?;
                }
            }
            Ok(RenderOutcome {
                file: out_name.to_string(),
                tool_versions: vec![("wav-fixture".into(), "0.0.0-test".into())],
                warnings: Vec::new(),
            })
        }
        fn name(&self) -> Option<&str> {
            Some("wav-fixture")
        }
    }

    fn tmp_project() -> (tempfile::TempDir, String) {
        let dir = tempfile::tempdir().unwrap();
        // Minimal container: project.json + CURRENT-less checkpoints dir.
        std::fs::create_dir_all(dir.path().join("checkpoints")).unwrap();
        std::fs::create_dir_all(dir.path().join("assets/sha256")).unwrap();
        (dir, "00000000-0000-4000-8000-0000000000aa".to_string())
    }

    /// Stage a complete, verified checkpoint directly (bypasses SaveSession
    /// so the export tests don't depend on engine snapshot providers).
    fn make_checkpoint(root: &Path, checkpoint_id: &str, project_id: &str) {
        let dir = root.join("checkpoints").join(checkpoint_id);
        std::fs::create_dir_all(&dir).unwrap();
        let files: [(&str, &[u8]); 3] = [
            ("engine.tracktionedit", b"<edit/>"),
            ("app-state.json", b"{}"),
            ("command-receipts.json", b"[]"),
        ];
        let mut refs = Vec::new();
        for (name, bytes) in files {
            std::fs::write(dir.join(name), bytes).unwrap();
            refs.push(serde_json::json!({
                "path": name,
                "sha256": void_assets::hex_sha256(bytes),
                "bytes": bytes.len().to_string(),
            }));
        }
        let m = serde_json::json!({
            "formatVersion": 1,
            "projectId": project_id,
            "checkpointId": checkpoint_id,
            "parentCheckpointId": serde_json::Value::Null,
            "revision": "7",
            "engineRevision": "e1",
            "createdAt": "2026-10-08T00:00:00Z",
            "engineSnapshot": refs[0],
            "appState": refs[1],
            "commandReceipts": refs[2],
            "assetHashes": [],
            "manifestState": "complete",
        });
        std::fs::write(
            dir.join("manifest.json"),
            serde_json::to_vec_pretty(&m).unwrap(),
        )
        .unwrap();
    }

    fn spec(job_id: &str, checkpoint_id: &str, project_id: &str) -> ExportSpec {
        ExportSpec {
            job_id: job_id.into(),
            project_id: project_id.into(),
            checkpoint_id: checkpoint_id.into(),
            source_revision: "7".into(),
            asset_hashes: vec![],
            range_start_ticks: "0".into(),
            range_end_ticks: "3840000".into(),
            format: ExportFormat::Wav,
            channels: Some(ChannelLayout::Stereo),
            bit_depth: Some(BitDepth::Pcm24),
            sample_rate: Some(48_000),
            tail: TailPolicy::None,
            tempo_map: vec![TempoSegment {
                at_ticks: "0".into(),
                bpm: 120.0,
            }],
            output_name: "mix-v1".into(),
            deadline_monotonic_ns: None,
        }
    }

    const J1: &str = "00000000-0000-4000-8000-0000000000e1";
    const C1: &str = "00000000-0000-4000-8000-0000000000c1";

    #[test]
    fn happy_path_publishes_with_provenance() {
        let (tmp, project_id) = tmp_project();
        make_checkpoint(tmp.path(), C1, &project_id);
        let runner = ExportRunner::memory(tmp.path()).unwrap();
        runner.submit(&spec(J1, C1, &project_id)).unwrap();
        assert_eq!(runner.status(J1).unwrap().status, JobStatus::Queued);

        let st = runner.run(J1, &WavFixture::ok()).unwrap();
        assert_eq!(st, JobStatus::Succeeded);

        let rec = runner.status(J1).unwrap();
        assert_eq!(rec.status, JobStatus::Succeeded);
        assert!(!rec.quarantined);
        let result = rec.result.unwrap();
        assert_eq!(result["kind"], "ExportResult");
        assert_eq!(result["file"], "mix-v1.wav");
        assert_eq!(result["frames"], "96000");

        // Published tree: artifact + provenance + result card.
        let dir = export_dir(tmp.path(), J1);
        assert!(dir.join("mix-v1.wav").is_file());
        let prov: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(dir.join(PROVENANCE_FILE)).unwrap(),
        )
        .unwrap();
        assert_eq!(prov["checkpointId"], C1);
        assert_eq!(prov["spec"]["rangeStartTicks"], "0");
        assert_eq!(prov["artifact"]["sha256"].as_str().unwrap().len(), 64);
        assert_eq!(prov["framePlan"]["totalFrames"], "96000");
        assert_eq!(prov["result"], "succeeded");
        // Staging is gone — nothing partial remains.
        assert!(!staging_dir(tmp.path(), J1).exists());
    }

    #[test]
    fn midi_export_path() {
        let (tmp, project_id) = tmp_project();
        make_checkpoint(tmp.path(), C1, &project_id);
        let runner = ExportRunner::memory(tmp.path()).unwrap();
        let mut s = spec(J1, C1, &project_id);
        s.format = ExportFormat::Midi;
        s.channels = None;
        s.bit_depth = None;
        s.sample_rate = None;
        s.output_name = "clip-a".into();
        runner.submit(&s).unwrap();
        assert_eq!(runner.run(J1, &WavFixture::ok()).unwrap(), JobStatus::Succeeded);
        assert!(export_dir(tmp.path(), J1).join("clip-a.mid").is_file());
    }

    #[test]
    fn renderer_failure_marks_failed_and_leaves_no_artifact() {
        let (tmp, project_id) = tmp_project();
        make_checkpoint(tmp.path(), C1, &project_id);
        let runner = ExportRunner::memory(tmp.path()).unwrap();
        runner.submit(&spec(J1, C1, &project_id)).unwrap();
        let st = runner.run(J1, &WavFixture::failing("engine exploded")).unwrap();
        assert_eq!(st, JobStatus::Failed);
        assert!(!export_dir(tmp.path(), J1).exists());
        assert!(!staging_dir(tmp.path(), J1).exists());
    }

    #[test]
    fn unverifiable_checkpoint_rejected_at_submit() {
        let (tmp, project_id) = tmp_project();
        // No checkpoint dir exists for this id.
        let runner = ExportRunner::memory(tmp.path()).unwrap();
        assert!(matches!(
            runner.submit(&spec(J1, "00000000-0000-4000-8000-00000000dead", &project_id)),
            Err(ExportError::CheckpointInvalid(_))
        ));
    }

    #[test]
    fn tampered_artifact_fails_verify_and_stays_unpublished() {
        struct BadBytes;
        impl Renderer for BadBytes {
            fn render(
                &self,
                _s: &ExportSpec,
                _p: &FramePlan,
                _c: &Path,
                staging_dir: &Path,
                out_name: &str,
                _cancel: &CancelToken,
            ) -> Result<RenderOutcome> {
                std::fs::write(staging_dir.join(out_name), b"garbage").unwrap();
                Ok(RenderOutcome {
                    file: out_name.into(),
                    tool_versions: vec![],
                    warnings: vec![],
                })
            }
        }
        let (tmp, project_id) = tmp_project();
        make_checkpoint(tmp.path(), C1, &project_id);
        let runner = ExportRunner::memory(tmp.path()).unwrap();
        runner.submit(&spec(J1, C1, &project_id)).unwrap();
        assert_eq!(runner.run(J1, &BadBytes).unwrap(), JobStatus::Failed);
        assert!(!export_dir(tmp.path(), J1).exists());
        assert!(!staging_dir(tmp.path(), J1).exists());
    }

    #[test]
    fn crash_at_each_boundary_never_publishes() {
        for step in [
            ExportStep::Render,
            ExportStep::Verify,
            ExportStep::Provenance,
            ExportStep::Rename,
            ExportStep::Publish,
        ] {
            let (tmp, project_id) = tmp_project();
            make_checkpoint(tmp.path(), C1, &project_id);
            let mut s = ExportSession::begin(tmp.path(), spec(J1, C1, &project_id)).unwrap();
            s.set_failpoint(step);
            let cancel = CancelToken::new();
            let r = s
                .render(&WavFixture::ok(), &cancel)
                .and_then(|_| s.verify())
                .and_then(|_| s.write_provenance())
                .and_then(|_| s.publish());
            assert!(r.is_err(), "failpoint {step:?} should error");
            let exported = export_dir(tmp.path(), J1);
            match step {
                // Crashes after the rename see a COMPLETE export dir —
                // verification ran pre-rename, so artifact + provenance
                // are already durable; the caller's receipt is the only
                // thing lost (OUTCOME_UNKNOWN-shaped, recoverable).
                ExportStep::Rename | ExportStep::Publish => {
                    assert!(exported.join("mix-v1.wav").is_file());
                    assert!(exported.join(PROVENANCE_FILE).is_file());
                }
                // Earlier crashes: staging may be left for the recovery
                // quarantine sweep, but nothing may exist under exports/.
                _ => assert!(!exported.exists()),
            }
            drop(s);
        }
    }

    #[test]
    fn cancel_before_run_and_during_run() {
        let (tmp, project_id) = tmp_project();
        make_checkpoint(tmp.path(), C1, &project_id);
        let runner = ExportRunner::memory(tmp.path()).unwrap();
        runner.submit(&spec(J1, C1, &project_id)).unwrap();
        // Queued cancel → straight to cancelled.
        assert_eq!(runner.cancel(J1).unwrap(), JobStatus::Cancelled);
        assert_eq!(runner.status(J1).unwrap().status, JobStatus::Cancelled);
        // A queued-cancelled job can no longer run.
        assert!(runner.run(J1, &WavFixture::ok()).is_err());

        // Running cancel: the fixture honours the token → Cancelled.
        let (tmp2, project_id2) = tmp_project();
        make_checkpoint(tmp2.path(), C1, &project_id2);
        let runner2 = ExportRunner::memory(tmp2.path()).unwrap();
        runner2.submit(&spec(J1, C1, &project_id2)).unwrap();
        struct CancelFirst;
        impl Renderer for CancelFirst {
            fn render(
                &self,
                _s: &ExportSpec,
                _p: &FramePlan,
                _c: &Path,
                _st: &Path,
                _o: &str,
                cancel: &CancelToken,
            ) -> Result<RenderOutcome> {
                // Simulate the db-side cancel arriving first: flip token.
                cancel.cancel();
                cancel.check()?;
                unreachable!()
            }
        }
        let st = runner2.run(J1, &CancelFirst).unwrap();
        assert_eq!(st, JobStatus::Cancelled);
        assert!(!export_dir(tmp2.path(), J1).exists());
        assert!(!staging_dir(tmp2.path(), J1).exists());
    }

    #[test]
    fn argv_is_structured_never_a_shell_string() {
        let r = ArgvRenderer::new("/usr/bin/false");
        let s = spec(J1, C1, "00000000-0000-4000-8000-0000000000aa");
        let plan = s.frame_plan().unwrap();
        let argv = r.argv_for(
            &s,
            &plan,
            Path::new("/p/checkpoints/c"),
            Path::new("/p/staging/export-x"),
            "mix-v1.wav",
        );
        // Each argument is its own entry — no ";rm", "|", "&&" joining is
        // possible because there is no string the shell re-parses.
        let args: Vec<String> = argv
            .iter()
            .map(|a| a.to_string_lossy().to_string())
            .collect();
        assert!(args.len() > 10);
        assert_eq!(args[0], "/usr/bin/false");
        assert!(args.iter().any(|a| a == "--format"));
        assert!(args.iter().any(|a| a == "wav"));
        assert!(args.iter().any(|a| a == "--total-frames"));
        assert!(args.iter().any(|a| a == "96000"));
        assert_eq!(argv_sha256(&argv).len(), 64);
    }
}
