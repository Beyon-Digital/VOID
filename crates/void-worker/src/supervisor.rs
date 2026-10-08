//! Worker supervisor: lifecycle, handshake auth, heartbeat, bounded restart.

use crate::transport::{self, ControlChannel, SocketPaths, TelemetryChannel};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use thiserror::Error;
use tokio::process::{Child, Command};
use tokio::sync::{watch, Mutex};
use void_protocol::limits::WORKER_RESTART_MAX;
use void_protocol::proto;
use void_protocol::validate;

#[derive(Debug, Error)]
pub enum SupervisorError {
    #[error("transport: {0}")]
    Transport(#[from] transport::TransportError),
    #[error("spawn failed: {0}")]
    Spawn(std::io::Error),
    #[error("handshake timeout after {0}ms")]
    HandshakeTimeout(u64),
    #[error("handshake rejected: {0}")]
    HandshakeRejected(String),
    #[error("worker exited unexpectedly: {0:?}")]
    WorkerExited(Option<i32>),
    #[error("restart budget exhausted")]
    RestartBudgetExhausted,
}

#[derive(Debug, Clone)]
pub struct RestartPolicy {
    pub max_attempts: u32,
    pub backoff_base_ms: u64,
}

impl Default for RestartPolicy {
    fn default() -> Self {
        Self {
            max_attempts: WORKER_RESTART_MAX,
            backoff_base_ms: 500,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    pub executable: PathBuf,
    pub args: Vec<String>,
    /// Extra env passed to the child (on top of the three socket/token vars).
    pub env: Vec<(String, String)>,
    /// Base dir for per-launch sockets, e.g. $XDG_RUNTIME_DIR/void.
    pub launch_base: PathBuf,
    pub handshake_timeout: Duration,
    pub restart: RestartPolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerState {
    Starting,
    Handshaking,
    Ready,
    Degraded,
    Restarting,
    Stopped,
    Failed,
}

#[derive(Debug, Clone)]
pub struct HeartbeatStatus {
    pub last_seen: Option<Instant>,
    pub missed: u32,
}

/// Handle to one supervised worker.
pub struct WorkerHandle {
    pub worker_id: String,
    pub paths: SocketPaths,
    pub launch_token: String,
    pub protocol_major: u16,
    pub protocol_minor: u16,
    pub engine_epoch: u64,
    pub capabilities: Vec<String>,
    control: ControlChannel,
    #[allow(dead_code)]
    telemetry: TelemetryChannel,
    child: Child,
    state_tx: watch::Sender<WorkerState>,
    state_rx: watch::Receiver<WorkerState>,
}

impl WorkerHandle {
    pub fn state(&self) -> WorkerState {
        *self.state_rx.borrow()
    }

    pub fn state_watch(&self) -> watch::Receiver<WorkerState> {
        self.state_rx.clone()
    }

    pub fn control(&mut self) -> &mut ControlChannel {
        &mut self.control
    }

    /// Orderly shutdown: drop the connection, then kill if it lingers.
    pub async fn shutdown(&mut self) {
        let _ = self.state_tx.send(WorkerState::Stopped);
        let _ = self.child.kill().await;
        let _ = self.child.wait().await;
        cleanup_dir(&self.paths.dir);
    }
}

fn cleanup_dir(dir: &std::path::Path) {
    let _ = std::fs::remove_dir_all(dir);
}

/// Supervisor: spawns a worker and returns a ready WorkerHandle after a
/// successful authenticated handshake.
pub struct Supervisor {
    config: SupervisorConfig,
    launch_id: String,
}

impl Supervisor {
    pub fn new(config: SupervisorConfig) -> Self {
        Self {
            config,
            launch_id: uuid::Uuid::new_v4().to_string(),
        }
    }

    pub fn launch_id(&self) -> &str {
        &self.launch_id
    }

    /// Spawn worker, bind sockets, wait for authenticated WorkerHello.
    pub async fn spawn(&self) -> Result<WorkerHandle, SupervisorError> {
        let paths = transport::create_launch_dir(&self.config.launch_base, &self.launch_id)
            .map_err(SupervisorError::Transport)?;
        let listeners = transport::bind(&paths).map_err(SupervisorError::Transport)?;

        let launch_token = uuid::Uuid::new_v4().simple().to_string();

        let mut cmd = Command::new(&self.config.executable);
        cmd.args(&self.config.args)
            .env(crate::ENV_CONTROL_SOCK, &paths.control)
            .env(crate::ENV_TELEMETRY_SOCK, &paths.telemetry)
            .env(crate::ENV_WORKER_TOKEN, &launch_token)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            // Detach into its own process group so shutdown can signal the
            // whole tree and no orphan survives the app.
            .process_group(0);
        for (k, v) in &self.config.env {
            cmd.env(k, v);
        }
        let mut child = cmd.spawn().map_err(SupervisorError::Spawn)?;

        let deadline = self.config.handshake_timeout;
        let hello = async {
            let (control_stream, _) = listeners
                .control
                .accept()
                .await
                .map_err(transport::TransportError::Io)?;
            let (telemetry_stream, _) = listeners
                .telemetry
                .accept()
                .await
                .map_err(transport::TransportError::Io)?;
            let mut control = ControlChannel::new(control_stream);
            let telemetry = TelemetryChannel::new(telemetry_stream);
            let frame = control.recv().await?;
            Ok::<_, transport::TransportError>((frame, control, telemetry))
        };

        let (frame, control, telemetry) = match tokio::time::timeout(deadline, hello).await {
            Ok(Ok(v)) => v,
            Ok(Err(e)) => {
                let _ = child.kill().await;
                cleanup_dir(&paths.dir);
                return Err(SupervisorError::Transport(e));
            }
            Err(_) => {
                let _ = child.kill().await;
                cleanup_dir(&paths.dir);
                return Err(SupervisorError::HandshakeTimeout(deadline.as_millis() as u64));
            }
        };

        // Verify the buffer, unwrap the envelope, authenticate token+version.
        let envelope = flatbuffers::root::<proto::ControlEnvelope>(&frame).map_err(|_| {
            SupervisorError::HandshakeRejected("invalid control envelope".into())
        })?;
        let hello = envelope.frame_as_worker_hello().ok_or_else(|| {
            SupervisorError::HandshakeRejected(format!(
                "first frame must be WorkerHello, got {:?}",
                envelope.frame_type().variant_name()
            ))
        })?;
        if hello.launch_token() != Some(launch_token.as_str()) {
            let _ = child.kill().await;
            cleanup_dir(&paths.dir);
            return Err(SupervisorError::HandshakeRejected("bad launch token".into()));
        }
        validate::negotiate(hello).map_err(|r| {
            SupervisorError::HandshakeRejected(format!("{}: {}", r.code.variant_name().unwrap_or("?"), r.message))
        })?;

        let (state_tx, state_rx) = watch::channel(WorkerState::Ready);
        Ok(WorkerHandle {
            worker_id: hello.worker_instance_id().unwrap_or_default().to_string(),
            paths,
            launch_token,
            protocol_major: hello.protocol_major(),
            protocol_minor: hello.protocol_minor(),
            engine_epoch: hello.engine_epoch(),
            capabilities: hello
                .capabilities()
                .map(|v| v.iter().map(|s| s.to_string()).collect())
                .unwrap_or_default(),
            control,
            telemetry,
            child,
            state_tx,
            state_rx,
        })
    }
}

/// Shared supervisor handle for app wiring.
pub type SharedSupervisor = Arc<Mutex<Supervisor>>;
