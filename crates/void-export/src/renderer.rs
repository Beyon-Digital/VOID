//! Renderer abstraction + argv-only process invocation (CONTRACTS.md §7).
//!
//! "argv-only process invocation and scoped filenames, not shell
//! interpolation": `ArgvRenderer` builds a `Vec<OsString>` passed to
//! `std::process::Command` — no shell, no joining, no metacharacter
//! interpretation anywhere. Every argument boundary is explicit.

use crate::error::{ExportError, Result};
use crate::spec::{ChannelLayout, ExportFormat, ExportSpec, FramePlan};
use sha2::{Digest, Sha256};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Cooperative cancel: the job runner flips this; renderers must abort
/// promptly and the session treats it as cancellation, not failure.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    flag: Arc<AtomicBool>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }
    /// Error-shortcut for render implementations.
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            Err(ExportError::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// What the renderer produced into the staging dir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderOutcome {
    /// Scoped file name of the artifact inside staging.
    pub file: String,
    /// Renderer self-report for provenance: ("name","version") pairs.
    /// Absent means the renderer is unidentified — recorded as such.
    pub tool_versions: Vec<(String, String)>,
    /// Optional warnings surfaced to the user (bounded text, no paths).
    pub warnings: Vec<String>,
}

/// A renderer consumes the immutable input set and stages one artifact.
/// Implemented by the engine offline-render entrypoint (argv) and by
/// test fixtures.
pub trait Renderer {
    /// Render `spec` (input = `checkpoint_dir`) into `staging_dir` as
    /// exactly `out_name`. Honour `cancel` — return `ExportError::Cancelled`
    /// when it trips rather than a half-written success.
    fn render(
        &self,
        spec: &ExportSpec,
        plan: &FramePlan,
        checkpoint_dir: &Path,
        staging_dir: &Path,
        out_name: &str,
        cancel: &CancelToken,
    ) -> Result<RenderOutcome>;

    /// Provenance label (e.g. "argv", "engine-offline", "fixture").
    fn name(&self) -> Option<&str> {
        None
    }

    /// Executable base name for provenance, when the renderer is a process.
    fn exe_name(&self) -> Option<String> {
        None
    }

    /// Downcast hook so the session can hash the exact argv of an
    /// `ArgvRenderer` without making argv part of the trait.
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        None
    }
}

/// The exact argv an `ArgvRenderer` will execute. Kept inspectable so the
/// provenance hash covers the real invocation.
pub fn build_argv(
    spec: &ExportSpec,
    plan: &FramePlan,
    checkpoint_dir: &Path,
    staging_dir: &Path,
    out_name: &str,
    extra: &[OsString],
) -> Vec<OsString> {
    fn push(v: &mut Vec<OsString>, s: &str) {
        v.push(OsString::from(s));
    }
    let mut v: Vec<OsString> = Vec::with_capacity(32 + extra.len());
    for a in extra {
        v.push(a.clone());
    }
    push(&mut v, "render");
    push(&mut v, "--checkpoint");
    v.push(checkpoint_dir.as_os_str().to_os_string());
    push(&mut v, "--out");
    v.push(staging_dir.join(out_name).as_os_str().to_os_string());
    push(&mut v, "--format");
    match spec.format {
        ExportFormat::Wav => push(&mut v, "wav"),
        ExportFormat::Midi => push(&mut v, "midi"),
    }
    push(&mut v, "--start-ticks");
    push(&mut v, &spec.range_start_ticks);
    push(&mut v, "--end-ticks");
    push(&mut v, &spec.range_end_ticks);
    push(&mut v, "--range-frames");
    push(&mut v, &plan.range_frames);
    push(&mut v, "--tail-frames");
    push(&mut v, &plan.tail_frames);
    push(&mut v, "--total-frames");
    push(&mut v, &plan.total_frames);
    if let Some(ch) = spec.channels {
        push(&mut v, "--channels");
        match ch {
            ChannelLayout::Mono => push(&mut v, "1"),
            ChannelLayout::Stereo => push(&mut v, "2"),
        }
    }
    if let Some(sr) = spec.sample_rate {
        push(&mut v, "--sample-rate");
        v.push(OsString::from(sr.to_string()));
    }
    if let Some(bd) = spec.bit_depth {
        push(&mut v, "--bit-depth");
        match bd {
            crate::spec::BitDepth::Pcm16 => push(&mut v, "pcm16"),
            crate::spec::BitDepth::Pcm24 => push(&mut v, "pcm24"),
            crate::spec::BitDepth::Float32 => push(&mut v, "float32"),
        }
    }
    for h in &spec.asset_hashes {
        push(&mut v, "--asset");
        push(&mut v, h);
    }
    v
}

/// SHA-256 of the canonical argv encoding (entries joined with \x1f).
/// Recorded in provenance — evidence of the exact invocation without
/// embedding paths/secrets.
pub fn argv_sha256(argv: &[OsString]) -> String {
    let mut h = Sha256::new();
    for (i, a) in argv.iter().enumerate() {
        if i > 0 {
            h.update(b"\x1f");
        }
        h.update(a.as_encoded_bytes());
    }
    format!("{:x}", h.finalize())
}

/// Runs a renderer executable with an explicit argv vector. The
/// executable name goes to provenance as a base name only.
pub struct ArgvRenderer {
    pub executable: PathBuf,
    /// Extra leading argv entries (e.g. ["--offline", "--engine-mode"]).
    pub base_args: Vec<OsString>,
    /// Environment must be opt-in: argv-only also means no env smuggling.
    pub env: Vec<(OsString, OsString)>,
}

impl ArgvRenderer {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            base_args: Vec::new(),
            env: Vec::new(),
        }
    }

    /// The argv this renderer will execute for a render — same function
    /// the runner uses; provenance hashes this vector.
    pub fn argv_for(
        &self,
        spec: &ExportSpec,
        plan: &FramePlan,
        checkpoint_dir: &Path,
        staging_dir: &Path,
        out_name: &str,
    ) -> Vec<OsString> {
        let mut argv = vec![self.executable.as_os_str().to_os_string()];
        argv.extend(build_argv(
            spec,
            plan,
            checkpoint_dir,
            staging_dir,
            out_name,
            &self.base_args,
        ));
        argv
    }

    fn spawn(&self, argv: &[OsString]) -> Result<Child> {
        let mut cmd = Command::new(&argv[0]);
        cmd.args(&argv[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear();
        // Minimal inherited environment: never forward the session env.
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        // argv-only: Command::new + args never touches a shell.
        cmd.spawn()
            .map_err(|e| ExportError::RendererFailed(format!("spawn {}: {e}", self.exe_basename())))
    }

    fn exe_basename(&self) -> String {
        self.executable
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "renderer".into())
    }
}

/// Bounded stderr tail kept for error reporting (never huge, never paths
/// beyond what the worker itself chose to print — the worker is the
/// boundary for message sanitization).
fn tail_lossy(bytes: &[u8], max: usize) -> String {
    let s = String::from_utf8_lossy(bytes);
    let trimmed = s.trim();
    if trimmed.len() <= max {
        trimmed.to_string()
    } else {
        trimmed[trimmed.len() - max..].to_string()
    }
}

impl Renderer for ArgvRenderer {
    fn render(
        &self,
        spec: &ExportSpec,
        plan: &FramePlan,
        checkpoint_dir: &Path,
        staging_dir: &Path,
        out_name: &str,
        cancel: &CancelToken,
    ) -> Result<RenderOutcome> {
        cancel.check()?;
        let argv = self.argv_for(spec, plan, checkpoint_dir, staging_dir, out_name);
        let mut child = self.spawn(&argv)?;
        // Poll for cancel — a stuck renderer cannot hold the job hostage.
        let status: ExitStatus = loop {
            if cancel.is_cancelled() {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ExportError::Cancelled);
            }
            match child.try_wait()? {
                Some(s) => break s,
                None => std::thread::sleep(std::time::Duration::from_millis(10)),
            }
        };
        let out = child.wait_with_output()?;
        if !status.success() {
            return Err(ExportError::RendererFailed(format!(
                "{} exited {}: {}",
                self.exe_basename(),
                status,
                tail_lossy(&out.stderr, 400)
            )));
        }
        let expected = staging_dir.join(out_name);
        if !expected.is_file() {
            return Err(ExportError::RendererFailed(
                "renderer exited 0 but produced no artifact".into(),
            ));
        }
        Ok(RenderOutcome {
            file: out_name.to_string(),
            tool_versions: vec![
                ("argv-executable".into(), self.exe_basename()),
                ("void-export".into(), env!("CARGO_PKG_VERSION").into()),
            ],
            warnings: Vec::new(),
        })
    }

    fn name(&self) -> Option<&str> {
        Some("argv")
    }

    fn exe_name(&self) -> Option<String> {
        Some(self.exe_basename())
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }
}
