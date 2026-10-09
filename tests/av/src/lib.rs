//! Shared fixtures for the W24 av suite. The fake encoder/probe binary
//! is built on demand (same pattern as crates/void-jobs runner tests
//! build workers/fake); a missing build is a loud failure, never a
//! silent skip.

use std::path::{Path, PathBuf};
use void_av::rate::FrameRounding;
use void_av::spec::AvCue;
use void_av::{AvExportSpec, AvInput, AvInputRole, AvTailPolicy};
use void_export::ChannelLayout;

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// Build + locate the fake ffmpeg. Fails loudly when the build fails.
/// Build once per test process: parallel `cargo build` invocations race on
/// the destination binary (cargo unlinks+relinks even when fresh), which
/// flakes the is_file() check underneath running tests on all platforms.
pub fn fake_ffmpeg() -> PathBuf {
    static EXE: std::sync::OnceLock<PathBuf> = std::sync::OnceLock::new();
    EXE.get_or_init(|| {
        let root = repo_root();
        let st = std::process::Command::new("cargo")
            .args(["build", "--manifest-path"])
            .arg(root.join("workers/ffmpeg-fake/Cargo.toml"))
            .status()
            .expect("spawn cargo build for void-fake-ffmpeg");
        assert!(st.success(), "cargo build of void-fake-ffmpeg failed");
        let p = root.join(format!(
            "workers/ffmpeg-fake/target/debug/void-fake-ffmpeg{}",
            std::env::consts::EXE_SUFFIX
        ));
        assert!(p.is_file(), "void-fake-ffmpeg binary missing at {p:?}");
        p
    })
    .clone()
}

/// Real ffmpeg iff present (feature-detect, per lane assignment).
pub fn real_ffmpeg() -> Option<PathBuf> {
    which("ffmpeg")
}

pub fn real_ffprobe() -> Option<PathBuf> {
    which("ffprobe")
}

fn which(name: &str) -> Option<PathBuf> {
    let out = std::process::Command::new("which")
        .arg(name)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    (!s.is_empty()).then(|| PathBuf::from(s))
}

/// Minimal container: checkpoints/ + assets/sha256/.
pub fn tmp_project() -> (tempfile::TempDir, String) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("checkpoints")).unwrap();
    std::fs::create_dir_all(dir.path().join("assets/sha256")).unwrap();
    (dir, "00000000-0000-4000-8000-0000000000aa".to_string())
}

/// Stage a complete, verified checkpoint (same shape void-export tests
/// use) plus an extra `program.vbin` file lanes can hash-pin as a
/// CheckpointFile input.
pub fn make_checkpoint(root: &Path, checkpoint_id: &str, project_id: &str) -> String {
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
    // Extra media input file — pinned by sha256 in specs that use it.
    let vbin: &[u8] = b"FAKE-VIDEO-PROGRAM-BYTES";
    std::fs::write(dir.join("program.vbin"), vbin).unwrap();
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
    void_assets::hex_sha256(vbin)
}

/// Baseline spec: lavfi fixture inputs, NTSC 30000/1001 @ 48 kHz, 1 s
/// program, ffv1+flac/mkv, 10 MB cap, 30 s encode bound.
pub fn spec(job_id: &str, checkpoint_id: &str, project_id: &str) -> AvExportSpec {
    AvExportSpec {
        job_id: job_id.into(),
        project_id: project_id.into(),
        checkpoint_id: checkpoint_id.into(),
        source_revision: "7".into(),
        asset_hashes: vec![],
        range_samples: "48000".into(),
        sample_rate: 48_000,
        channels: ChannelLayout::Stereo,
        frame_rate_num: 30000,
        frame_rate_den: 1001,
        rounding: FrameRounding::NearestTiesAway,
        width: 160,
        height: 120,
        codec_id: "ffv1_flac_mkv".into(),
        inputs: vec![
            AvInput::LavfiTest {
                src: "testsrc2=size=160x120".into(),
                role: AvInputRole::Video,
            },
            AvInput::LavfiTest {
                src: "sine=frequency=440".into(),
                role: AvInputRole::Audio,
            },
        ],
        cues: vec![
            AvCue {
                cue_id: "boundary".into(),
                at_sample: "24024".into(),
            },
            AvCue {
                cue_id: "midframe".into(),
                at_sample: "25000".into(),
            },
        ],
        tail: AvTailPolicy::None,
        output_name: "viz-v1".into(),
        timeout_ms: "30000".into(),
        max_output_bytes: "10485760".into(),
        deadline_monotonic_ns: None,
    }
}

/// Spec with checkpoint-pinned inputs instead of lavfi fixtures.
pub fn spec_with_checkpoint_inputs(
    job_id: &str,
    checkpoint_id: &str,
    project_id: &str,
    vbin_sha: &str,
) -> AvExportSpec {
    let mut s = spec(job_id, checkpoint_id, project_id);
    s.inputs = vec![
        AvInput::CheckpointFile {
            rel_path: "program.vbin".into(),
            sha256: vbin_sha.into(),
            role: AvInputRole::Video,
        },
        AvInput::CheckpointFile {
            rel_path: "engine.tracktionedit".into(),
            sha256: void_assets::hex_sha256(b"<edit/>"),
            role: AvInputRole::Audio,
        },
    ];
    s
}

/// AvTools over the fake binary for both encode and probe. `env` sets
/// the declared env allowlist on the encoder (VOID_FAKE_FFMPEG_MODE…).
pub fn fake_tools(env: &[(&str, &str)]) -> void_av::AvTools {
    let exe = fake_ffmpeg();
    let mut t = void_av::AvTools::new(&exe, &exe);
    for (k, v) in env {
        t.encoder.env.push((k.to_string(), v.to_string()));
    }
    t
}
