//! Media sources: image decode (real PNG/JPEG via `image`), bounded
//! ffmpeg video pull decoding (argv-only spawn per CONTRACTS.md §7), and
//! generator presets. Decode failures are visible (`DecodeFailed` /
//! `VisualAlertEvent`), never silent black.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::thread;

use sha2::{Digest, Sha256};

use crate::error::{Result, VisualError};
use crate::types::MediaRef;

/// Decoded pixel frame (RGBA8).
#[derive(Debug, Clone, PartialEq)]
pub struct RgbaFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl RgbaFrame {
    pub fn sha256(&self) -> String {
        let mut h = Sha256::new();
        h.update(self.width.to_le_bytes());
        h.update(self.height.to_le_bytes());
        h.update(&self.pixels);
        crate::ops::hex(&h.finalize())
    }
}

/// Decode/validation ceilings (CONTRACTS.md §2 spirit — bounded decode).
pub const MAX_IMAGE_DIM: u32 = 8192;
pub const MAX_IMAGE_PIXELS: u64 = 128 * 1024 * 1024; // 512 MiB RGBA budget
pub const MAX_MEDIA_BYTES: u64 = 512 * 1024 * 1024;
pub const VIDEO_RING_FRAMES: usize = 4;
pub const MAX_VIDEO_LAYERS: usize = 4;

/// Verify `rel_path` under `assets_root` exists, matches `media.sha256`,
/// and decode it (image) or hand it to the pull decoder (video).
pub fn verify_asset(assets_root: &Path, media: &MediaRef) -> Result<PathBuf> {
    let path = assets_root.join(&media.rel_path);
    if !path.is_file() {
        return Err(VisualError::AssetMissing(media.rel_path.clone()));
    }
    let mut f = std::fs::File::open(&path).map_err(|_| {
        VisualError::AssetMissing(media.rel_path.clone())
    })?;
    let mut h = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = f.read(&mut buf).map_err(|e| {
            VisualError::AssetMissing(format!("read {}: {e}", media.rel_path))
        })?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > MAX_MEDIA_BYTES {
            return Err(VisualError::BadRequest("media exceeds byte cap".into()));
        }
        h.update(&buf[..n]);
    }
    let hash = crate::ops::hex(&h.finalize());
    if hash != media.sha256 {
        return Err(VisualError::AssetMissing(format!(
            "sha256 mismatch for {} (expected {}, got {})",
            media.rel_path, media.sha256, hash
        )));
    }
    Ok(path)
}

/// Decode an image file to RGBA8 with bounded dims.
pub fn decode_image(path: &Path) -> Result<RgbaFrame> {
    let img = image::open(path)
        .map_err(|e| VisualError::DecodeFailed(format!("{}: {e}", path.display())))?;
    let img = img.to_rgba8();
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 || w > MAX_IMAGE_DIM || h > MAX_IMAGE_DIM {
        return Err(VisualError::DecodeFailed(format!(
            "image {w}x{h} outside 1..{MAX_IMAGE_DIM}"
        )));
    }
    if (w as u64) * (h as u64) * 4 > MAX_IMAGE_PIXELS * 4 {
        return Err(VisualError::DecodeFailed("image exceeds pixel budget".into()));
    }
    Ok(RgbaFrame {
        width: w,
        height: h,
        pixels: img.into_raw(),
    })
}

/// Bounded video pull decoder: spawns `ffmpeg` argv-only (no shell),
/// decodes rawvideo RGBA frames from stdout on a reader thread into a
/// bounded ring keyed by frame index. Render never blocks on it — a
/// missing/lagging frame resolves to the last decoded frame.
pub struct FfmpegPuller {
    child: Option<Child>,
    rx: mpsc::Receiver<PullerMsg>,
    #[allow(dead_code)]
    handle: Option<thread::JoinHandle<()>>,
    /// Frames received so far, keyed by decoded index.
    latest: std::collections::VecDeque<(u64, RgbaFrame)>,
    eof: bool,
    failed: Option<String>,
    pub width: u32,
    pub height: u32,
    /// Source frame index → decode order (starts at 0 each spawn).
    #[allow(dead_code)]
    first_index: u64,
    pub fps_num: u32,
    pub fps_den: u32,
}

enum PullerMsg {
    Frame(u64, RgbaFrame),
    Failed(String),
    Eof,
}

impl FfmpegPuller {
    /// Spawn a decoder for `path` starting at `start_sec`, output frames
    /// scaled to width×height RGBA. `ffmpeg_bin` allows tests to point at
    /// a stub; production passes "ffmpeg".
    pub fn spawn(
        path: &Path,
        start_sec: f64,
        width: u32,
        height: u32,
        fps_num: u32,
        fps_den: u32,
        ffmpeg_bin: &str,
    ) -> Result<Self> {
        if width == 0 || height == 0 || width > MAX_IMAGE_DIM || height > MAX_IMAGE_DIM {
            return Err(VisualError::DecodeFailed("video dims out of range".into()));
        }
        // argv-only invocation: never a shell, never user-interpolated
        // strings (CONTRACTS.md §7).
        let mut child = Command::new(ffmpeg_bin)
            .args([
                "-nostdin",
                "-v", "error",
                "-ss", &format!("{start_sec:.6}"),
                "-i", path.to_str().ok_or_else(|| {
                    VisualError::DecodeFailed("non-utf8 media path".into())
                })?,
                "-f", "rawvideo",
                "-pix_fmt", "rgba",
                "-s", &format!("{width}x{height}"),
                "-",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| VisualError::DecodeFailed(format!("spawn ffmpeg: {e}")))?;
        let mut stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take();
        let (tx, rx) = mpsc::sync_channel(VIDEO_RING_FRAMES * 2);
        let frame_bytes = (width as usize) * (height as usize) * 4;
        let handle = thread::spawn(move || {
            let mut idx = 0u64;
            let mut buf = vec![0u8; frame_bytes];
            let mut filled = 0usize;
            loop {
                match stdout.read(&mut buf[filled..]) {
                    Ok(0) => {
                        if filled == 0 {
                            let _ = tx.send(PullerMsg::Eof);
                        } else {
                            let _ = tx.send(PullerMsg::Failed(format!(
                                "truncated frame ({filled}/{frame_bytes})"
                            )));
                        }
                        return;
                    }
                    Ok(n) => {
                        filled += n;
                        if filled == frame_bytes {
                            let frame = RgbaFrame {
                                width,
                                height,
                                pixels: buf.clone(),
                            };
                            if tx.send(PullerMsg::Frame(idx, frame)).is_err() {
                                return;
                            }
                            idx += 1;
                            filled = 0;
                        }
                    }
                    Err(e) => {
                        let msg = format!("read: {e}");
                        let _ = tx.send(PullerMsg::Failed(msg));
                        return;
                    }
                }
            }
        });
        // Drain stderr so a chatty ffmpeg cannot deadlock on a full pipe.
        if let Some(mut err) = stderr {
            thread::spawn(move || {
                let mut b = [0u8; 4096];
                while matches!(err.read(&mut b), Ok(n) if n > 0) {}
            });
        }
        Ok(Self {
            child: Some(child),
            rx,
            handle: Some(handle),
            latest: std::collections::VecDeque::with_capacity(VIDEO_RING_FRAMES + 1),
            eof: false,
            failed: None,
            width,
            height,
            first_index: 0,
            fps_num,
            fps_den,
        })
    }

    /// Drain decoded frames; returns true if any arrived. Non-blocking.
    pub fn pump(&mut self) -> bool {
        let mut got = false;
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                PullerMsg::Frame(idx, f) => {
                    self.latest.push_back((idx, f));
                    while self.latest.len() > VIDEO_RING_FRAMES {
                        self.latest.pop_front();
                    }
                    got = true;
                }
                PullerMsg::Failed(m) => self.failed = Some(m),
                PullerMsg::Eof => self.eof = true,
            }
        }
        got
    }

    /// Frame for absolute decode index `idx` — exact frame if buffered,
    /// else the closest decoded frame (lagging video never blocks the
    /// render); None before any frame decodes.
    pub fn frame_at(&mut self, idx: u64) -> Option<RgbaFrame> {
        self.pump();
        // Drop frames older than the request if we have something newer.
        while self.latest.len() > 1 && self.latest.front().map(|(i, _)| *i) < Some(idx) {
            self.latest.pop_front();
        }
        self.latest
            .iter()
            .rev()
            .find(|(i, _)| *i <= idx)
            .or_else(|| self.latest.front())
            .map(|(_, f)| f.clone())
    }

    pub fn is_failed(&self) -> Option<&String> {
        self.failed.as_ref()
    }

    pub fn is_eof(&self) -> bool {
        self.eof
    }

    /// Terminate the decoder (layer removal, route change, shutdown).
    pub fn kill(&mut self) {
        if let Some(mut c) = self.child.take() {
            let _ = c.kill();
            let _ = c.wait();
        }
    }
}

impl Drop for FfmpegPuller {
    fn drop(&mut self) {
        self.kill();
    }
}
