//! ffmpeg argv-only runner (CONTRACTS.md §2/§7, T88).
//!
//! The encoder is invoked as a pure argv array: exe is resolved from a
//! configured path whose *recorded* name is the basename only, the
//! environment is cleared and opt-in, stdin is null, stdout/stderr are
//! captured through bounded drains, and the child is killed on cancel
//! or timeout. There is no shell anywhere in this file.

use crate::codec;
use crate::error::{AvError, Result};
use crate::spec::{parse_u64, AvExportSpec, AvFramePlan, AvInput, AvInputRole};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};
use void_export::{argv_sha256, CancelToken};

const POLL_TICK: Duration = Duration::from_millis(10);
/// Bound on captured stdout/stderr — a runaway log must not OOM us.
const MAX_LOG_BYTES: usize = 1 << 20;

/// What the encode step produced.
#[derive(Debug, Clone)]
pub struct RenderOutcome {
    /// Filename of the staged artifact (basename only).
    pub file: String,
    /// Tool version strings recorded into provenance
    /// (`ffmpeg -version` first line, or the fake's self-report).
    pub tool_versions: Vec<String>,
    /// Non-fatal observations worth recording.
    pub warnings: Vec<String>,
}

/// Renderer contract — the fake binary implements the same argv
/// surface, so tests exercise the identical code path.
pub trait AvRenderer: Send + Sync {
    /// Encode `spec`/`plan` into `staging_dir/out_name`. `resolved`
    /// pairs each `AvInput` (in spec order) with the scoped on-disk
    /// path it resolves to — `LavfiTest` entries resolve to their
    /// `lavfi:` pseudo-path and are passed to `-i` verbatim.
    fn render(
        &self,
        spec: &AvExportSpec,
        plan: &AvFramePlan,
        resolved: &[ResolvedInput],
        staging_dir: &Path,
        out_name: &str,
        cancel: &CancelToken,
    ) -> Result<RenderOutcome>;
    fn name(&self) -> &'static str;
    /// Basename recorded in provenance — never a full path.
    fn exe_name(&self) -> String;
    fn as_any(&self) -> &dyn std::any::Any;
}

#[derive(Debug, Clone)]
pub struct ResolvedInput {
    pub input: AvInput,
    /// For CheckpointFile/AssetBlob: absolute scoped path. For
    /// LavfiTest: `lavfi:<src>` marker passed to `-f lavfi -i`.
    pub argv_path: String,
}

/// ffmpeg subprocess runner. `executable` may be a path (real ffmpeg)
/// or the fake binary — identical argv contract either way.
pub struct FfmpegRunner {
    pub executable: PathBuf,
    /// Opt-in environment (env_clear + these only).
    pub env: Vec<(String, String)>,
}

impl FfmpegRunner {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            env: Vec::new(),
        }
    }

    /// Detected capabilities of the tool build — encoder names.
    /// Absent tool → `CodecUnavailable` (typed; T88).
    pub fn detect_encoders(&self) -> Result<HashSet<String>> {
        let argv = [
            self.executable.clone(),
            PathBuf::from("-hide_banner"),
            PathBuf::from("-encoders"),
        ];
        let out = Command::new(&argv[0])
            .args(&argv[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear()
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AvError::CodecUnavailable(format!(
                        "encoder tool '{}' not found",
                        base_name(&self.executable)
                    ))
                } else {
                    AvError::Io(e)
                }
            })?;
        if !out.status.success() {
            return Err(AvError::CodecUnavailable(format!(
                "encoder tool '{}' refused -encoders (status {})",
                base_name(&self.executable),
                out.status
            )));
        }
        Ok(parse_encoders(&String::from_utf8_lossy(&out.stdout)))
    }

    /// `ffmpeg -version` first line, for provenance. Missing tool →
    /// `CodecUnavailable` (same gate as detect_encoders).
    pub fn tool_version(&self) -> Result<String> {
        let out = Command::new(&self.executable)
            .arg("-version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear()
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AvError::CodecUnavailable(format!(
                        "encoder tool '{}' not found",
                        base_name(&self.executable)
                    ))
                } else {
                    AvError::Io(e)
                }
            })?;
        let s = String::from_utf8_lossy(&out.stdout);
        Ok(s.lines().next().unwrap_or("unknown").trim().to_string())
    }

    /// Build the exact argv for an encode. Pure function — unit-tested
    /// for the argv contract without spawning anything (T88).
    pub fn build_argv(
        &self,
        spec: &AvExportSpec,
        plan: &AvFramePlan,
        resolved: &[ResolvedInput],
        out_path: &Path,
    ) -> Result<Vec<String>> {
        let row = codec::codec(&spec.codec_id)?;
        let mut a: Vec<String> = vec![
            self.exe_name(),
            "-hide_banner".into(),
            "-nostdin".into(),
            "-y".into(),
        ];
        let audio_dur = spec.duration_seconds(plan)?;
        for r in resolved {
            // `-t` scopes to the AUDIO INPUT, not the output: an output
            // `-t` would clip the video stream at the audio duration,
            // which under NTSC rates (30 frames in 1.001 s) drops the
            // tail frame. Video is bounded exactly by `-frames:v`.
            let is_audio = matches!(&r.input,
                AvInput::CheckpointFile{role,..} | AvInput::AssetBlob{role,..} | AvInput::LavfiTest{role,..}
                if *role == AvInputRole::Audio);
            match &r.input {
                AvInput::LavfiTest { src, .. } => {
                    a.push("-f".into());
                    a.push("lavfi".into());
                    if is_audio {
                        a.push("-t".into());
                        a.push(audio_dur.clone());
                    }
                    a.push("-i".into());
                    a.push(src.clone());
                }
                _ => {
                    if is_audio {
                        a.push("-t".into());
                        a.push(audio_dur.clone());
                    }
                    a.push("-i".into());
                    a.push(r.argv_path.clone());
                }
            }
        }
        // Deterministic stream selection: role order in spec.inputs
        // fixes the -map indices we emit.
        let vidx = input_index(resolved, AvInputRole::Video)
            .ok_or_else(|| AvError::InvalidSpec("no video input".into()))?;
        let aidx = input_index(resolved, AvInputRole::Audio)
            .ok_or_else(|| AvError::InvalidSpec("no audio input".into()))?;
        a.push("-map".into());
        a.push(format!("{vidx}:v"));
        a.push("-map".into());
        a.push(format!("{aidx}:a"));
        a.push("-r".into());
        a.push(format!("{}/{}", plan.frame_rate_num, plan.frame_rate_den));
        a.push("-frames:v".into());
        a.push(plan.video_total_frames.clone());
        a.push("-s".into());
        a.push(format!("{}x{}", spec.width, spec.height));
        a.push("-c:v".into());
        a.push(row.video_encoder.into());
        a.push("-pix_fmt".into());
        a.push(row.pixel_format.into());
        a.push("-c:a".into());
        a.push(row.audio_encoder.into());
        a.push("-ar".into());
        a.push(spec.sample_rate.to_string());
        a.push("-ac".into());
        a.push(
            match spec.channels {
                void_export::ChannelLayout::Mono => "1",
                void_export::ChannelLayout::Stereo => "2",
            }
            .into(),
        );
        for x in row.extra_args {
            a.push((*x).to_string());
        }
        let cap = parse_u64("maxOutputBytes", &spec.max_output_bytes)?;
        a.push("-fs".into());
        a.push(cap.to_string());
        a.push(out_path.to_string_lossy().into_owned());
        Ok(a)
    }
}

/// Bounded pipe drain running on a thread; first byte of the returned
/// buffer tags the stream (1 = stderr, 0 = stdout).
fn drain_bounded(mut r: impl Read, tx: mpsc::Sender<Vec<u8>>, is_err: bool) {
    let mut buf = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        match r.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if buf.len() < MAX_LOG_BYTES {
                    let take = n.min(MAX_LOG_BYTES - buf.len());
                    buf.extend_from_slice(&chunk[..take]);
                }
            }
        }
    }
    let _ = tx.send(if is_err {
        [&[1u8], &buf[..]].concat()
    } else {
        [&[0u8], &buf[..]].concat()
    });
}

fn input_index(resolved: &[ResolvedInput], role: AvInputRole) -> Option<usize> {
    resolved.iter().position(|r| {
        matches!((&r.input, role),
            (AvInput::CheckpointFile{role: rr,..} | AvInput::AssetBlob{role: rr,..} | AvInput::LavfiTest{role: rr,..}, _)
            if *rr == role)
    })
}

impl AvRenderer for FfmpegRunner {
    fn render(
        &self,
        spec: &AvExportSpec,
        plan: &AvFramePlan,
        resolved: &[ResolvedInput],
        staging_dir: &Path,
        out_name: &str,
        cancel: &CancelToken,
    ) -> Result<RenderOutcome> {
        // Codec gate: id must be declared AND both encoders present in
        // the detected tool build. Typed CodecUnavailable either way.
        let row = codec::codec(&spec.codec_id)?;
        let encoders = self.detect_encoders()?;
        codec::require_encoders(row, &|n| encoders.contains(n))?;
        let version = self.tool_version()?;

        let out_name = format!("{out_name}.{}", row.container_ext);
        let out_path = staging_dir.join(&out_name);
        let argv = self.build_argv(spec, plan, resolved, &out_path)?;
        let argv_os: Vec<OsString> = argv.iter().map(OsString::from).collect();
        tracing::info!(job = %spec.job_id, argv_sha = %argv_sha256(&argv_os), "av encode argv");

        let mut child = Command::new(&self.executable)
            .args(&argv[1..])
            .current_dir(staging_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear()
            .envs(self.env.iter().cloned())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AvError::CodecUnavailable(format!(
                        "encoder tool '{}' not found",
                        base_name(&self.executable)
                    ))
                } else {
                    AvError::Io(e)
                }
            })?;

        let timeout = Duration::from_millis(parse_u64("timeoutMs", &spec.timeout_ms)?);
        let cap = parse_u64("maxOutputBytes", &spec.max_output_bytes)?;
        let out_for_stat = out_path.clone();

        // Bounded stdout/stderr drain on threads.
        let stdout = child.stdout.take().expect("stdout piped");
        let stderr = child.stderr.take().expect("stderr piped");
        let (tx, rx) = mpsc::channel();
        let tx2 = tx.clone();
        let t1 = thread::spawn(move || drain_bounded(stdout, tx, false));
        let t2 = thread::spawn(move || drain_bounded(stderr, tx2, true));

        let deadline = Instant::now() + timeout;
        let mut killed = false;
        let status = loop {
            if cancel.is_cancelled() {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(&out_for_stat);
                return Err(AvError::Cancelled);
            }
            if Instant::now() > deadline {
                let _ = child.kill();
                killed = true;
            }
            // -fs already bounds the encoder; a tool that ignores it is
            // killed here so the cap is real either way.
            let over_cap = std::fs::metadata(&out_for_stat)
                .map(|m| m.len() > cap)
                .unwrap_or(false);
            if over_cap {
                let _ = child.kill();
                let _ = child.wait();
                let _ = std::fs::remove_file(&out_for_stat);
                return Err(AvError::OutputLimitExceeded);
            }
            match child.try_wait()? {
                Some(s) => break s,
                None => thread::sleep(POLL_TICK),
            }
        };
        let _ = t1.join();
        let _ = t2.join();
        let mut err_tail = Vec::new();
        while let Ok(buf) = rx.try_recv() {
            if buf.first() == Some(&1) {
                err_tail = buf[1..].to_vec();
            }
        }

        if killed {
            let _ = std::fs::remove_file(&out_for_stat);
            return Err(AvError::Timeout);
        }
        if cancel.is_cancelled() {
            let _ = std::fs::remove_file(&out_for_stat);
            return Err(AvError::Cancelled);
        }
        if !status.success() {
            let _ = std::fs::remove_file(&out_for_stat);
            let tail = String::from_utf8_lossy(&err_tail);
            let tail: String = tail
                .chars()
                .rev()
                .take(512)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            return Err(AvError::EncoderFailed(format!(
                "{} exited {status}: {tail}",
                self.exe_name()
            )));
        }
        if !out_path.is_file() {
            return Err(AvError::EncoderFailed(format!(
                "{} reported success but produced no artifact",
                self.exe_name()
            )));
        }
        Ok(RenderOutcome {
            file: out_name,
            tool_versions: vec![version],
            warnings: Vec::new(),
        })
    }

    fn name(&self) -> &'static str {
        "ffmpeg"
    }

    fn exe_name(&self) -> String {
        base_name(&self.executable)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// `ffmpeg -encoders` output → set of encoder names. Lines look like
/// ` V..... libx264    libx264 H.264 / AVC ...` — six flag columns then
/// the name. The fake emits the same shape.
pub fn parse_encoders(text: &str) -> HashSet<String> {
    text.lines()
        .filter_map(|l| {
            let l = l.trim_start();
            (l.len() > 7 && l.as_bytes()[0].is_ascii_alphabetic())
                .then(|| l[7..].split_whitespace().next().map(str::to_string))
                .flatten()
        })
        .collect()
}

fn base_name(p: &Path) -> String {
    p.file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned()
}

/// Re-exported so callers hashing argv don't re-implement the scheme.
pub use void_export::argv_sha256 as av_argv_sha256;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodecGateReport {
    pub codec_id: String,
    pub ffmpeg_present: bool,
    pub video_encoder_present: bool,
    pub audio_encoder_present: bool,
    pub selectable: bool,
    pub reason: Option<String>,
}

/// Probe what selecting `codec_id` against `runner` would do — used by
/// the studio layer + tests to show unavailability honestly (T88).
pub fn codec_gate(runner: &FfmpegRunner, codec_id: &str) -> CodecGateReport {
    let row = match codec::codec(codec_id) {
        Ok(r) => r,
        Err(e) => {
            return CodecGateReport {
                codec_id: codec_id.into(),
                ffmpeg_present: true,
                video_encoder_present: false,
                audio_encoder_present: false,
                selectable: false,
                reason: Some(e.to_string()),
            }
        }
    };
    match runner.detect_encoders() {
        Ok(set) => {
            let v = set.contains(row.video_encoder);
            let a = set.contains(row.audio_encoder);
            CodecGateReport {
                codec_id: codec_id.into(),
                ffmpeg_present: true,
                video_encoder_present: v,
                audio_encoder_present: a,
                selectable: v && a,
                reason: if v && a {
                    None
                } else {
                    Some(format!(
                        "missing {}",
                        if !v {
                            row.video_encoder
                        } else {
                            row.audio_encoder
                        }
                    ))
                },
            }
        }
        Err(e) => CodecGateReport {
            codec_id: codec_id.into(),
            ffmpeg_present: false,
            video_encoder_present: false,
            audio_encoder_present: false,
            selectable: false,
            reason: Some(e.to_string()),
        },
    }
}
