//! Scoped IPC transport: per-launch socket directory, UDS listeners,
//! length-prefixed frames via void-protocol.

use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use void_protocol::frame::{encode_frame, FrameError, FrameReader};
use void_protocol::limits::CONTROL_FRAME_MAX;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("frame: {0}")]
    Frame(#[from] FrameError),
    #[error("peer closed")]
    Closed,
}

/// Paths handed to a worker child process via environment.
#[derive(Debug, Clone)]
pub struct SocketPaths {
    pub dir: PathBuf,
    pub control: PathBuf,
    pub telemetry: PathBuf,
}

/// Create a per-launch socket directory owned by the current user (0700).
pub fn create_launch_dir(base: &Path, launch_id: &str) -> Result<SocketPaths, TransportError> {
    let dir = base.join(launch_id);
    fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))?;
    }
    Ok(SocketPaths {
        control: dir.join("control.sock"),
        telemetry: dir.join("telemetry.sock"),
        dir,
    })
}

/// Server side of the worker control channel.
pub struct ControlChannel {
    stream: UnixStream,
    reader: FrameReader,
}

/// Write half of a split control channel (sends are serialized by the caller).
pub struct ControlWriter {
    stream: tokio::net::unix::OwnedWriteHalf,
}

/// Read half of a split control channel (frames in worker -> app order).
pub struct ControlReader {
    stream: tokio::net::unix::OwnedReadHalf,
    reader: FrameReader,
}

/// Listener pair awaiting a worker's two connections.
pub struct WorkerListeners {
    pub control: UnixListener,
    pub telemetry: UnixListener,
}

pub fn bind(paths: &SocketPaths) -> Result<WorkerListeners, TransportError> {
    let _ = fs::remove_file(&paths.control);
    let _ = fs::remove_file(&paths.telemetry);
    Ok(WorkerListeners {
        control: UnixListener::bind(&paths.control)?,
        telemetry: UnixListener::bind(&paths.telemetry)?,
    })
}

impl ControlChannel {
    pub fn new(stream: UnixStream) -> Self {
        Self {
            stream,
            reader: FrameReader::default(),
        }
    }

    pub async fn send(&mut self, payload: &[u8]) -> Result<(), TransportError> {
        let frame = encode_frame(payload)?;
        self.stream.write_all(&frame).await?;
        self.stream.flush().await?;
        Ok(())
    }

    /// Read the next complete frame payload.
    pub async fn recv(&mut self) -> Result<Vec<u8>, TransportError> {
        let mut chunk = [0u8; 16 * 1024];
        loop {
            if let Some(frame) = self.reader.next_frame()? {
                return Ok(frame);
            }
            let n = self.stream.read(&mut chunk).await?;
            if n == 0 {
                return Err(TransportError::Closed);
            }
            self.reader.push(&chunk[..n]);
        }
    }

    /// Split into independent write/read halves so sends and the event
    /// dispatcher can run concurrently (e.g. PANIC while a receipt is owed).
    pub fn into_split(self) -> (ControlWriter, ControlReader) {
        let (r, w) = self.stream.into_split();
        (
            ControlWriter { stream: w },
            ControlReader {
                stream: r,
                reader: self.reader,
            },
        )
    }
}

impl ControlWriter {
    pub async fn send(&mut self, payload: &[u8]) -> Result<(), TransportError> {
        let frame = encode_frame(payload)?;
        self.stream.write_all(&frame).await?;
        self.stream.flush().await?;
        Ok(())
    }
}

impl ControlReader {
    /// Read the next complete frame payload.
    pub async fn recv(&mut self) -> Result<Vec<u8>, TransportError> {
        let mut chunk = [0u8; 16 * 1024];
        loop {
            if let Some(frame) = self.reader.next_frame()? {
                return Ok(frame);
            }
            let n = self.stream.read(&mut chunk).await?;
            if n == 0 {
                return Err(TransportError::Closed);
            }
            self.reader.push(&chunk[..n]);
        }
    }
}

/// Telemetry channel is lossy: old coalescible frames may be dropped by the
/// reader when a newer sequence arrives (handled by subscribers upstream).
pub struct TelemetryChannel {
    stream: UnixStream,
    reader: FrameReader,
}

impl TelemetryChannel {
    pub fn new(stream: UnixStream) -> Self {
        Self {
            stream,
            reader: FrameReader::default(),
        }
    }

    pub async fn recv(&mut self) -> Result<Vec<u8>, TransportError> {
        let mut chunk = [0u8; 16 * 1024];
        loop {
            if let Some(frame) = self.reader.next_frame()? {
                return Ok(frame);
            }
            let n = self.stream.read(&mut chunk).await?;
            if n == 0 {
                return Err(TransportError::Closed);
            }
            self.reader.push(&chunk[..n]);
        }
    }
}

/// Worker-side helpers (used by the reference mock worker in tests and by
/// any Rust-native worker): connect to the supervisor's sockets.
#[allow(dead_code)]
pub async fn connect(paths: &SocketPaths) -> Result<(UnixStream, UnixStream), TransportError> {
    let control = UnixStream::connect(&paths.control).await?;
    let telemetry = UnixStream::connect(&paths.telemetry).await?;
    Ok((control, telemetry))
}

#[allow(dead_code)]
pub async fn send_frame(stream: &mut UnixStream, payload: &[u8]) -> Result<(), TransportError> {
    let frame = encode_frame(payload)?;
    stream.write_all(&frame).await?;
    stream.flush().await?;
    Ok(())
}

const _: u32 = CONTROL_FRAME_MAX;
