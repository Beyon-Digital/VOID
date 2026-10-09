//! ffprobe probe-back (verify step). The produced artifact is re-read
//! by a *separate* argv-only tool before publish: video frame count and
//! audio stream duration/samples must match the declared plan. Real
//! ffprobe is used when present; the fake binary answers the same argv
//! shape so the verify path is identical offline (T87/T88).

use crate::error::{AvError, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const MAX_PROBE_OUTPUT: usize = 8 << 20;

/// What the probe measured about the artifact.
#[derive(Debug, Clone, PartialEq)]
pub struct ProbeInfo {
    /// Decoded video frames actually in the file (`nb_read_frames`).
    pub video_frames: u64,
    /// Rational frame rate the container declares.
    pub video_fps_num: u64,
    pub video_fps_den: u64,
    /// Audio samples the stream actually contains. `None` until the
    /// decode-count fallback runs (see `probe`).
    pub audio_samples: Option<u64>,
    pub audio_sample_rate: u32,
    pub audio_channels: u32,
    pub video_codec: String,
    pub audio_codec: String,
    /// Container bytes on disk.
    pub bytes: u64,
}

pub trait MediaProbe: Send + Sync {
    fn probe(&self, file: &Path) -> Result<ProbeInfo>;
    /// Basename recorded in provenance — never a full path.
    fn exe_name(&self) -> String;
}

/// ffprobe subprocess. Same argv-only rules as the encoder.
pub struct FfprobeRunner {
    pub executable: PathBuf,
}

impl FfprobeRunner {
    pub fn new(executable: impl Into<PathBuf>) -> Self {
        Self {
            executable: executable.into(),
        }
    }

    /// The exact argv — `-count_frames` makes ffprobe decode and count
    /// real frames rather than trusting container metadata (T87 asks
    /// for measured counts, not header claims).
    pub fn argv(&self, file: &Path) -> Vec<String> {
        vec![
            self.exe_name(),
            "-v".into(), "error".into(),
            "-count_frames".into(),
            "-show_entries".into(),
            "stream=index,codec_type,codec_name,nb_read_frames,r_frame_rate,sample_rate,channels,duration_ts,time_base,duration:format=size".into(),
            "-of".into(), "json".into(),
            file.to_string_lossy().into_owned(),
        ]
    }

    /// Second pass, run only when the container carries no usable audio
    /// duration field (matroska/nut store none): a bounded decode
    /// counting `nb_samples` per audio frame. T88's bounded-decode
    /// requirement — output is still capped by MAX_PROBE_OUTPUT.
    pub fn frames_argv(&self, file: &Path) -> Vec<String> {
        vec![
            self.exe_name(),
            "-v".into(),
            "error".into(),
            "-select_streams".into(),
            "a:0".into(),
            "-show_entries".into(),
            "frame=nb_samples".into(),
            "-of".into(),
            "json".into(),
            file.to_string_lossy().into_owned(),
        ]
    }

    fn exe_name_pub(&self) -> String {
        self.executable
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }
}

impl MediaProbe for FfprobeRunner {
    fn exe_name(&self) -> String {
        self.exe_name_pub()
    }

    fn probe(&self, file: &Path) -> Result<ProbeInfo> {
        let argv = self.argv(file);
        let out = Command::new(&self.executable)
            .args(&argv[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear()
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AvError::ProbeFailed(format!("probe tool '{}' not found", self.exe_name_pub()))
                } else {
                    AvError::Io(e)
                }
            })?;
        if out.stdout.len() > MAX_PROBE_OUTPUT {
            return Err(AvError::ProbeFailed("probe output exceeds bound".into()));
        }
        if !out.status.success() {
            let tail = String::from_utf8_lossy(&out.stderr);
            let tail: String = tail
                .chars()
                .rev()
                .take(256)
                .collect::<String>()
                .chars()
                .rev()
                .collect();
            return Err(AvError::ProbeFailed(format!(
                "{} exited {}: {tail}",
                self.exe_name_pub(),
                out.status
            )));
        }
        let mut info = parse_probe(&out.stdout, file)?;
        if info.audio_samples.is_none() {
            info.audio_samples = Some(self.audio_samples_via_frames(file)?);
        }
        Ok(info)
    }
}

impl FfprobeRunner {
    /// Bounded audio decode-count fallback (matroska/nut have no
    /// container audio duration).
    fn audio_samples_via_frames(&self, file: &Path) -> Result<u64> {
        let argv = self.frames_argv(file);
        let out = Command::new(&self.executable)
            .args(&argv[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_clear()
            .output()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AvError::ProbeFailed(format!("probe tool '{}' not found", self.exe_name_pub()))
                } else {
                    AvError::Io(e)
                }
            })?;
        if out.stdout.len() > MAX_PROBE_OUTPUT {
            return Err(AvError::ProbeFailed(
                "probe frames output exceeds bound".into(),
            ));
        }
        if !out.status.success() {
            return Err(AvError::ProbeFailed(format!(
                "{} frames pass exited {}",
                self.exe_name_pub(),
                out.status
            )));
        }
        parse_audio_samples(&out.stdout)
    }
}

#[derive(Deserialize)]
struct ProbeDoc {
    #[serde(default)]
    streams: Vec<ProbeStream>,
    #[serde(default)]
    format: Option<ProbeFormat>,
}

#[derive(Deserialize)]
struct ProbeFormat {
    #[serde(default)]
    size: Option<String>,
}

#[derive(Deserialize)]
struct ProbeStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    nb_read_frames: Option<String>,
    r_frame_rate: Option<String>,
    sample_rate: Option<String>,
    channels: Option<u32>,
    duration_ts: Option<u64>,
    /// `num/den` e.g. "1/48000".
    time_base: Option<String>,
    /// Seconds as decimal string — containers like matroska carry no
    /// duration_ts, only this.
    duration: Option<String>,
}

fn parse_probe(json: &[u8], file: &Path) -> Result<ProbeInfo> {
    let doc: ProbeDoc = serde_json::from_slice(json)
        .map_err(|e| AvError::ProbeFailed(format!("unparseable probe json: {e}")))?;
    let mut v: Option<&ProbeStream> = None;
    let mut a: Option<&ProbeStream> = None;
    for s in &doc.streams {
        match s.codec_type.as_deref() {
            Some("video") if v.is_none() => v = Some(s),
            Some("audio") if a.is_none() => a = Some(s),
            _ => {}
        }
    }
    let v = v.ok_or_else(|| AvError::ProbeFailed("artifact has no video stream".into()))?;
    let a = a.ok_or_else(|| AvError::ProbeFailed("artifact has no audio stream".into()))?;

    let video_frames = v
        .nb_read_frames
        .as_deref()
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or_else(|| AvError::ProbeFailed("video stream reports no nb_read_frames".into()))?;
    let (fps_num, fps_den) = parse_ratio(v.r_frame_rate.as_deref().unwrap_or("0/1"));
    if fps_num == 0 || fps_den == 0 {
        return Err(AvError::ProbeFailed(
            "video stream reports invalid frame rate".into(),
        ));
    }
    let sample_rate = a
        .sample_rate
        .as_deref()
        .and_then(|s| s.parse::<u32>().ok())
        .ok_or_else(|| AvError::ProbeFailed("audio stream reports no sample rate".into()))?;
    let (tb_num, tb_den) = parse_ratio(a.time_base.as_deref().unwrap_or("0/1"));
    let audio_samples = if let Some(ts) = a.duration_ts {
        if tb_num == 0 || tb_den == 0 {
            return Err(AvError::ProbeFailed(
                "audio stream reports invalid time_base".into(),
            ));
        }
        // exact: samples = duration_ts * tb_num/tb_den * sample_rate
        let num = ts as u128 * tb_num as u128 * sample_rate as u128;
        Some(
            u64::try_from(num / tb_den as u128)
                .map_err(|_| AvError::ProbeFailed("audio duration overflows u64".into()))?,
        )
    } else if let Some(d) = &a.duration {
        // Stream-level seconds × sample_rate, nearest.
        let secs = d
            .parse::<f64>()
            .map_err(|_| AvError::ProbeFailed("audio duration unparseable".into()))?;
        if !(secs.is_finite() && secs >= 0.0) {
            return Err(AvError::ProbeFailed(
                "audio stream reports invalid duration".into(),
            ));
        }
        Some((secs * sample_rate as f64).round() as u64)
    } else {
        // Container-level duration is NOT used for audio: matroska
        // reports max-of-streams (video tail inflates it past the
        // declared tolerance). Caller falls back to a bounded decode
        // count of nb_samples instead.
        None
    };
    let bytes = match doc
        .format
        .and_then(|f| f.size)
        .as_deref()
        .and_then(|s| s.parse::<u64>().ok())
    {
        Some(b) => b,
        None => std::fs::metadata(file).map(|m| m.len()).unwrap_or(0),
    };
    Ok(ProbeInfo {
        video_frames,
        video_fps_num: fps_num,
        video_fps_den: fps_den,
        audio_samples,
        audio_sample_rate: sample_rate,
        audio_channels: a.channels.unwrap_or(0),
        video_codec: v.codec_name.clone().unwrap_or_default(),
        audio_codec: a.codec_name.clone().unwrap_or_default(),
        bytes,
    })
}

/// Sum `nb_samples` across decoded audio frames (bounded decode
/// fallback). ffprobe emits nb_samples either as a number or a string
/// depending on version — accept both.
fn parse_audio_samples(json: &[u8]) -> Result<u64> {
    #[derive(Deserialize)]
    struct FramesDoc {
        #[serde(default)]
        frames: Vec<FrameEntry>,
    }
    #[derive(Deserialize)]
    struct FrameEntry {
        nb_samples: Option<serde_json::Value>,
    }
    let doc: FramesDoc = serde_json::from_slice(json)
        .map_err(|e| AvError::ProbeFailed(format!("unparseable frames json: {e}")))?;
    if doc.frames.is_empty() {
        return Err(AvError::ProbeFailed(
            "audio decode produced no frames".into(),
        ));
    }
    let mut total: u128 = 0;
    for f in &doc.frames {
        let n = match &f.nb_samples {
            Some(serde_json::Value::Number(n)) => n.as_u64(),
            Some(serde_json::Value::String(s)) => s.parse::<u64>().ok(),
            _ => None,
        }
        .ok_or_else(|| AvError::ProbeFailed("audio frame lacks nb_samples".into()))?;
        total += n as u128;
    }
    u64::try_from(total).map_err(|_| AvError::ProbeFailed("audio samples overflow u64".into()))
}

fn parse_ratio(s: &str) -> (u64, u64) {
    let mut it = s.split('/');
    let n = it.next().and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    let d = it.next().and_then(|x| x.parse::<u64>().ok()).unwrap_or(0);
    (n, d)
}
