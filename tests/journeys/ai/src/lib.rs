//! Shared helpers for the W16 F2 regression-gate suite (T63/T64/T65).
//! Everything here drives REAL code paths: `crates/void-jobs` JobRunner,
//! `crates/void-proposals` lifecycle, `crates/void-project` save path and
//! the real argv worker binaries under `workers/`.

use std::fs;
use std::path::{Path, PathBuf};
use void_assets::AssetStore;
use void_jobs::{JobDb, JobKind, JobRunner, JobSpec, Reservations};

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..").canonicalize().unwrap()
}

/// tests/journeys/ai/<exe> helpers that live in this crate's fixtures dir.
pub fn fixture_exe(name: &str) -> PathBuf {
    let p = repo_root().join("tests/journeys/ai/fixtures").join(name);
    assert!(p.is_file(), "fixture worker missing: {p:?}");
    p
}

/// `workers/fake` reference worker (built by this suite's setup).
pub fn fake_worker() -> PathBuf {
    let p = repo_root().join(format!(
        "workers/fake/target/debug/void-fake-worker{}",
        std::env::consts::EXE_SUFFIX
    ));
    assert!(p.is_file(), "void-fake-worker not built — run `cargo build --manifest-path workers/fake/Cargo.toml`");
    p
}

pub fn symbolic_worker() -> PathBuf {
    let p = repo_root().join(format!(
        "workers/symbolic/target/debug/void-symbolic-worker{}",
        std::env::consts::EXE_SUFFIX
    ));
    assert!(p.is_file(), "void-symbolic-worker not built — run `cargo build --manifest-path workers/symbolic/Cargo.toml`");
    p
}

/// A provisioned `workers/audio/<dir>` launcher (fails loudly, never skips).
pub fn audio_worker(dir: &str, launcher: &str) -> PathBuf {
    let d = repo_root().join("workers/audio").join(dir);
    let exe = d.join(launcher);
    assert!(exe.is_file(), "worker launcher missing: {exe:?}");
    assert!(
        d.join(".venv/bin/python3").is_file(),
        "{dir} not provisioned — run workers/audio/{dir}/setup.sh first"
    );
    exe
}

/// One project container: its own asset store + job/proposal dirs.
pub struct ProjectEnv {
    pub id: String,
    pub root: tempfile::TempDir,
    pub store: AssetStore,
}

impl ProjectEnv {
    pub fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let store = AssetStore::new(root.path().join("assets"), 8 * 1024 * 1024 * 1024).unwrap();
        Self { id: uuid::Uuid::new_v4().to_string(), root, store }
    }
    pub fn runner(&self, db_path: &Path) -> JobRunner {
        let db = JobDb::open(db_path).unwrap();
        JobRunner::new(db, self.store.clone(), self.root.path().to_path_buf())
    }
    pub fn job_dir(&self, job_id: &str) -> PathBuf {
        self.root.path().join("jobs").join(job_id)
    }
    pub fn staging_dir(&self, job_id: &str) -> PathBuf {
        self.root.path().join("staging").join(format!("job-{job_id}"))
    }
}

/// The app-private index shared across open projects (db.rs model).
pub fn app_db(app_root: &Path) -> JobDb {
    JobDb::open(app_root.join("index.db")).unwrap()
}

#[allow(clippy::too_many_arguments)]
pub fn spec(
    project_id: &str,
    kind: JobKind,
    runtime_id: &str,
    model_id: &str,
    params: serde_json::Value,
    ram_bytes: u64,
    source_revision: &str,
) -> JobSpec {
    JobSpec {
        job_id: uuid::Uuid::new_v4().to_string(),
        project_id: project_id.to_string(),
        source_revision: source_revision.to_string(),
        context_sha256: "ab".repeat(32),
        kind,
        runtime_id: runtime_id.into(),
        runtime_sha256: "cd".repeat(32),
        model_id: Some(model_id.into()),
        model_sha256: Some("ef".repeat(32)),
        inputs: vec![],
        parameters: params,
        reservations: Reservations {
            ram_bytes: ram_bytes.to_string(),
            vram_bytes: "0".into(),
            cpu_threads: 4,
        },
        deadline_monotonic_ns: "0".into(),
        output_scope_token: format!("scope-{project_id}"),
        cloud_consent_id: None,
    }
}

/// Find a spawned worker's PID by a unique argv substring (its staging
/// dir names the job) — used to SIGKILL a real worker mid-run.
pub fn find_pid_with_argv(needle: &str) -> Option<i32> {
    for e in fs::read_dir("/proc").ok()? {
        let e = e.ok()?;
        let name = e.file_name();
        let pid: i32 = match name.to_str().and_then(|s| s.parse().ok()) {
            Some(p) => p,
            None => continue,
        };
        if let Ok(cmd) = fs::read(e.path().join("cmdline")) {
            let s = String::from_utf8_lossy(&cmd);
            if s.contains(needle) {
                return Some(pid);
            }
        }
    }
    None
}

/// SIGKILL a pid the hard way — no cooperation asked of the child.
pub fn kill9(pid: i32) {
    unsafe { libc::kill(pid, 9) };
}

pub fn sha256_bytes(b: &[u8]) -> String {
    use sha2::Digest;
    let d: [u8; 32] = sha2::Sha256::digest(b).into();
    d.iter().map(|x| format!("{x:02x}")).collect()
}

pub fn sha256_file(p: &Path) -> String {
    sha256_bytes(&fs::read(p).unwrap_or_else(|e| panic!("read {p:?}: {e}")))
}

// ---------- shared audio fixtures (same synth as tests/models) ----------

pub const SR: u32 = 48_000;
/// 8-note monophonic melody, exact onsets (0.55 s spacing).
pub const TRUTH: &[(u8, f64)] = &[
    (60, 0.05), (64, 0.60), (67, 1.15), (69, 1.70),
    (67, 2.25), (64, 2.80), (62, 3.35), (60, 3.90),
];

fn midi_hz(m: u8) -> f64 {
    440.0 * 2f64.powf((m as f64 - 69.0) / 12.0)
}

/// Real 48 kHz stereo PCM16 WAV of the 8-note melody.
pub fn synth_melody_wav() -> Vec<u8> {
    let note_dur = 0.5f64;
    let total = TRUTH.last().unwrap().1 + note_dur + 0.25;
    let n = (total * SR as f64) as usize;
    let mut mix = vec![0f64; n];
    for (m, t0) in TRUTH {
        let f = midi_hz(*m);
        let i0 = (*t0 * SR as f64) as usize;
        let nn = (note_dur * SR as f64) as usize;
        for i in 0..nn.min(n - i0) {
            let t = i as f64 / SR as f64;
            let atk = (t / 0.015).min(1.0);
            let rel = ((note_dur - t) / 0.04).min(1.0).max(0.0);
            let env = (atk * rel).powf(0.5);
            let ph = 2.0 * std::f64::consts::PI * f * t;
            let v = (ph.sin() + 0.3 * (2.0 * ph).sin() + 0.15 * (3.0 * ph).sin() + 0.07 * (4.0 * ph).sin())
                * env * 0.5;
            mix[i0 + i] += v;
        }
    }
    let delay = (0.006 * SR as f64) as usize;
    let mut pcm = Vec::with_capacity(n * 4);
    for i in 0..n {
        let l = (mix[i].clamp(-1.0, 1.0) * 32000.0) as i16;
        let rv = if i >= delay { mix[i - delay] } else { 0.0 };
        let r = (rv.clamp(-1.0, 1.0) * 32000.0) as i16;
        pcm.extend_from_slice(&l.to_le_bytes());
        pcm.extend_from_slice(&r.to_le_bytes());
    }
    let mut w = Vec::with_capacity(44 + pcm.len());
    w.extend_from_slice(b"RIFF");
    w.extend_from_slice(&(36u32 + pcm.len() as u32).to_le_bytes());
    w.extend_from_slice(b"WAVEfmt ");
    w.extend_from_slice(&16u32.to_le_bytes());
    w.extend_from_slice(&1u16.to_le_bytes());
    w.extend_from_slice(&2u16.to_le_bytes());
    w.extend_from_slice(&SR.to_le_bytes());
    w.extend_from_slice(&(SR * 4).to_le_bytes());
    w.extend_from_slice(&4u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    w.extend_from_slice(&pcm);
    w
}

/// PCM `data` chunk byte length of a WAV file.
pub fn wav_data_bytes(data: &[u8]) -> usize {
    assert_eq!(&data[..4], b"RIFF");
    let mut off = 12usize;
    while off + 8 <= data.len() {
        let id = &data[off..off + 4];
        let len = u32::from_le_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]) as usize;
        if id == b"data" {
            return len;
        }
        off += 8 + len + (len & 1);
    }
    panic!("no data chunk");
}

pub fn wav_sample_rate(data: &[u8]) -> u32 {
    assert_eq!(&data[..4], b"RIFF");
    assert_eq!(&data[8..12], b"WAVE");
    u32::from_le_bytes([data[24], data[25], data[26], data[27]])
}

/// RMS of 16-bit PCM payload (proves generated audio is not silence).
pub fn wav_rms(data: &[u8]) -> f64 {
    let off = data.len() - wav_data_bytes(data);
    let mut acc = 0f64;
    let mut n = 0usize;
    for ch in data[off..].chunks_exact(2) {
        let v = i16::from_le_bytes([ch[0], ch[1]]) as f64 / 32768.0;
        acc += v * v;
        n += 1;
    }
    (acc / n as f64).sqrt()
}

fn read_varlen(b: &[u8], i: &mut usize) -> u64 {
    let mut v = 0u64;
    loop {
        let c = b[*i];
        *i += 1;
        v = (v << 7) | (c & 0x7f) as u64;
        if c & 0x80 == 0 {
            return v;
        }
    }
}

/// Note-on times (seconds) of every note in an SMF file via its tempo map.
pub fn midi_note_ons(data: &[u8]) -> Vec<f64> {
    assert_eq!(&data[..4], b"MThd", "not an SMF file");
    let div = u16::from_be_bytes([data[12], data[13]]) as u64;
    assert!(div > 0 && div & 0x8000 == 0, "SMPTE division unsupported");
    let ntracks = u16::from_be_bytes([data[10], data[11]]) as usize;
    let mut off = 14;
    let mut tempos: Vec<(u64, u64)> = vec![(0, 500_000)];
    let mut on_ticks = Vec::new();
    for _ in 0..ntracks {
        assert_eq!(&data[off..off + 4], b"MTrk");
        let len = u32::from_be_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]]) as usize;
        let mut i = off + 8;
        let end = i + len;
        let mut tick = 0u64;
        let mut status = 0u8;
        while i < end {
            tick += read_varlen(data, &mut i);
            let mut st = data[i];
            if st < 0x80 {
                st = status;
            } else {
                i += 1;
                if st < 0xF0 {
                    status = st;
                }
            }
            match st {
                0xFF => {
                    let meta = data[i];
                    i += 1;
                    let l = read_varlen(data, &mut i) as usize;
                    if meta == 0x51 && l == 3 {
                        let us = ((data[i] as u64) << 16) | ((data[i + 1] as u64) << 8) | data[i + 2] as u64;
                        tempos.push((tick, us));
                    }
                    i += l;
                }
                0xF0 | 0xF7 => {
                    let l = read_varlen(data, &mut i) as usize;
                    i += l;
                }
                _ => {
                    let op = st & 0xF0;
                    let b1 = data[i];
                    i += 1;
                    let b2 = if matches!(op, 0xC0 | 0xD0) { 0 } else { data[i] };
                    if !matches!(op, 0xC0 | 0xD0) {
                        i += 1;
                    }
                    if op == 0x90 && b2 > 0 {
                        let _ = b1;
                        on_ticks.push(tick);
                    }
                }
            }
        }
        off = end;
    }
    tempos.sort_by_key(|t| t.0);
    let mut out = Vec::with_capacity(on_ticks.len());
    for &t in &on_ticks {
        let mut us = 0u64;
        let mut last_tick = 0u64;
        let mut cur = 500_000u64;
        for &(tt, us_qn) in tempos.iter().skip(1) {
            if tt >= t {
                break;
            }
            us += (tt - last_tick) * cur / div;
            last_tick = tt;
            cur = us_qn;
        }
        us += (t - last_tick) * cur / div;
        out.push(us as f64 / 1e6);
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap());
    out
}
