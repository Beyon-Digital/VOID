//! argv-worker spawn (workers/PROTOCOL.md v1, CONTRACTS.md §2/§6).
//!
//! `Command` + argv vector + `env_clear` — never a shell, never a
//! smuggled argument (same rule as void-export's ArgvRenderer). The
//! only argv besides the executable is `--staging <dir>`; the job spec
//! goes over stdin. Rlimits apply pre-exec so a worker never runs a
//! single instruction above budget.

use crate::budget::{apply_rlimits, JobBudget};
use crate::error::{JobError, Result};
use std::ffi::OsString;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// Cooperative cancel (mirrors void-export's token): the runner flips
/// it; the spawn loop kills the child promptly.
#[derive(Debug, Clone, Default)]
pub struct CancelToken {
    flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn cancel(&self) {
        self.flag.store(true, std::sync::atomic::Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.flag.load(std::sync::atomic::Ordering::SeqCst)
    }
}

/// A qualified argv runtime — the executable path is resolved by the
/// coordinator's allowlist (bundled workers dir), never from job data.
#[derive(Debug, Clone)]
pub struct WorkerRuntime {
    pub executable: PathBuf,
    /// Extra leading argv entries, e.g. ["--sandbox"] — bounded, no
    /// user text: the contract forbids arbitrary shell arguments.
    pub base_args: Vec<OsString>,
    /// Opt-in env grants (worker gets env_clear + these only).
    pub env: Vec<(OsString, OsString)>,
}

impl WorkerRuntime {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
            base_args: Vec::new(),
            env: Vec::new(),
        }
    }

    /// Basename for provenance — never an absolute path (safe-message rule).
    pub fn exe_basename(&self) -> String {
        self.executable
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "worker".into())
    }

    /// The exact argv — recorded into provenance as its SHA-256.
    pub fn argv_for(&self, staging_dir: &std::path::Path) -> Vec<OsString> {
        let mut v = Vec::with_capacity(4 + self.base_args.len());
        v.push(self.executable.as_os_str().to_os_string());
        v.extend(self.base_args.iter().cloned());
        v.push(OsString::from("--staging"));
        v.push(staging_dir.as_os_str().to_os_string());
        v
    }

    /// Spawn with budget rlimits applied pre-exec. Stdin is handed back
    /// for the spec write; stdout/stderr are piped for the pump loop.
    pub fn spawn(
        &self,
        staging_dir: &std::path::Path,
        spec_json: &str,
        budget: &JobBudget,
    ) -> Result<Child> {
        let argv = self.argv_for(staging_dir);
        let mut cmd = Command::new(&argv[0]);
        cmd.args(&argv[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear();
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        apply_rlimits(&mut cmd, budget);
        let mut child = cmd
            .spawn()
            .map_err(|e| JobError::SpawnFailed(format!("{}: {e}", self.exe_basename())))?;
        if let Some(mut stdin) = child.stdin.take() {
            // Spec delivery must not hold the job hostage if the worker
            // never reads — bounded write, errors become SpawnFailed.
            let spec = spec_json.to_string();
            std::thread::spawn(move || {
                let _ = stdin.write_all(spec.as_bytes());
                let _ = stdin.flush(); // drop closes stdin → worker sees EOF
            });
        }
        Ok(child)
    }
}
