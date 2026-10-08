//! VOID native worker supervisor (W02): spawns a native process, creates
//! per-launch UDS control+telemetry sockets, authenticates the handshake,
//! monitors heartbeats, restarts within bounds, and shuts down cleanly.
//!
//! Transport: Unix-domain sockets under a `0700` per-launch directory
//! (named pipes on Windows — `pipe` module is cfg-gated; Linux/macOS is
//! implemented here). The 128-bit launch token and socket paths reach the
//! child via environment, never argv (CONTRACTS.md §2).

mod supervisor;
mod transport;

pub use supervisor::{
    HeartbeatStatus, RestartPolicy, Supervisor, SupervisorConfig, SupervisorError, WorkerHandle,
    WorkerState,
};
pub use transport::{ControlChannel, SocketPaths, TelemetryChannel};

pub const ENV_CONTROL_SOCK: &str = "VOID_CONTROL_SOCK";
pub const ENV_TELEMETRY_SOCK: &str = "VOID_TELEMETRY_SOCK";
pub const ENV_WORKER_TOKEN: &str = "VOID_WORKER_TOKEN";
