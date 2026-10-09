//! W15 qualification suite (TEST_MATRIX T60–T62) — the four real
//! `workers/audio/` argv workers driven through the real
//! `crates/void-jobs` JobRunner: spawn → spec stdin → stdout stream →
//! artifact verify+import → provenance/result publish.
//!
//! Fixtures are synthesized here (no copyrighted audio): an 8-note
//! monophonic melody with exact ground-truth onsets, rendered as a real
//! 48 kHz stereo PCM16 WAV and imported through the real AssetStore.
//!
//! These tests require the worker bundles to be provisioned
//! (`workers/audio/<dir>/setup.sh`). A missing venv fails loudly with
//! the exact command — never skipped, never faked.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use void_assets::AssetStore;
use void_jobs::{
    JobBudget, JobDb, JobKind, JobRunner, JobSpec, JobStatus, Reservations, RunContext, RunOutcome,
    WorkerRuntime,
};
use void_models::ModelManifest;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn worker_dir(name: &str) -> PathBuf {
    repo_root().join("workers/audio").join(name)
}

/// Resolve the worker launcher, requiring a provisioned venv.
fn worker_exe(dir: &str, launcher: &str) -> PathBuf {
    let d = worker_dir(dir);
    let exe = d.join(launcher);
    assert!(exe.is_file(), "worker launcher missing: {exe:?}");
    assert!(
        d.join(".venv/bin/python3").is_file(),
        "{dir} not provisioned — run workers/audio/{dir}/setup.sh first"
    );
    exe
}

// ---------- fixture: synthesized 48 kHz stereo melody WAV ----------

const SR: u32 = 48_000;
/// Ground truth: MIDI note + onset seconds (0.55 s spacing, legato).
const TRUTH: &[(u8, f64)] = &[
    (60, 0.05),
    (64, 0.60),
    (67, 1.15),
    (69, 1.70),
    (67, 2.25),
    (64, 2.80),
    (62, 3.35),
    (60, 3.90),
];

fn midi_hz(m: u8) -> f64 {
    440.0 * 2f64.powf((m as f64 - 69.0) / 12.0)
}

/// Render the melody: sine + 3 harmonics, 15 ms attack / 40 ms release.
fn synth_melody_wav() -> Vec<u8> {
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
                * env
                * 0.5;
            mix[i0 + i] += v;
        }
    }
    // interleave stereo (R = L delayed 6 ms for a real stereo file)
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
    w.extend_from_slice(&1u16.to_le_bytes()); // PCM
    w.extend_from_slice(&2u16.to_le_bytes()); // stereo
    w.extend_from_slice(&SR.to_le_bytes());
    w.extend_from_slice(&(SR * 4).to_le_bytes());
    w.extend_from_slice(&4u16.to_le_bytes());
    w.extend_from_slice(&16u16.to_le_bytes());
    w.extend_from_slice(b"data");
    w.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    w.extend_from_slice(&pcm);
    w
}

// ---------- minimal SMF parser (note-on times + tempo map) ----------

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

/// (tick, sec) of every note-on across tracks, using tempo events.
fn midi_note_ons(data: &[u8]) -> Vec<f64> {
    assert_eq!(&data[..4], b"MThd", "not an SMF file");
    let div = u16::from_be_bytes([data[12], data[13]]) as u64;
    assert!(div > 0 && div & 0x8000 == 0, "SMPTE division unsupported");
    let ntracks = u16::from_be_bytes([data[10], data[11]]) as usize;
    let mut off = 14;
    let mut tempos: Vec<(u64, u64)> = vec![(0, 500_000)]; // tick -> us/qn
    let mut on_ticks = Vec::new();
    for _ in 0..ntracks {
        assert_eq!(&data[off..off + 4], b"MTrk");
        let len = u32::from_be_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]])
            as usize;
        let mut i = off + 8;
        let end = i + len;
        let mut tick = 0u64;
        let mut status = 0u8;
        while i < end {
            tick += read_varlen(data, &mut i);
            let mut st = data[i];
            if st < 0x80 {
                st = status; // running status
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
                        let us = ((data[i] as u64) << 16)
                            | ((data[i + 1] as u64) << 8)
                            | data[i + 2] as u64;
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
    // tick -> seconds via the tempo map
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

/// PCM data size of a 16-bit WAV (byte count of the `data` chunk).
fn wav_data_bytes(data: &[u8]) -> usize {
    assert_eq!(&data[..4], b"RIFF");
    let mut off = 12usize;
    while off + 8 <= data.len() {
        let id = &data[off..off + 4];
        let len = u32::from_le_bytes([data[off + 4], data[off + 5], data[off + 6], data[off + 7]])
            as usize;
        if id == b"data" {
            return len;
        }
        off += 8 + len + (len & 1);
    }
    panic!("no data chunk");
}

fn wav_sample_rate(data: &[u8]) -> u32 {
    assert_eq!(&data[..4], b"RIFF");
    assert_eq!(&data[8..12], b"WAVE");
    u32::from_le_bytes([data[24], data[25], data[26], data[27]])
}

// ---------- job fixture ----------

struct Fixture {
    root: tempfile::TempDir,
    db: JobDb,
    store: AssetStore,
}

fn fixture() -> Fixture {
    let root = tempfile::tempdir().unwrap();
    let db = JobDb::open(root.path().join("index.db")).unwrap();
    let store = AssetStore::new(root.path().join("assets"), 4 * 1024 * 1024 * 1024).unwrap();
    Fixture { root, db, store }
}

fn runner(f: &Fixture) -> JobRunner {
    let db = JobDb::open(f.root.path().join("index.db")).unwrap();
    JobRunner::new(db, f.store.clone(), f.root.path().to_path_buf())
}

fn spec(kind: JobKind, runtime_id: &str, model_id: &str, inputs: Vec<String>, params: serde_json::Value) -> JobSpec {
    JobSpec {
        job_id: uuid::Uuid::new_v4().to_string(),
        project_id: uuid::Uuid::new_v4().to_string(),
        source_revision: "7".into(),
        context_sha256: "ab".repeat(32),
        kind,
        runtime_id: runtime_id.into(),
        runtime_sha256: "cd".repeat(32),
        model_id: Some(model_id.into()),
        model_sha256: Some("ef".repeat(32)),
        inputs,
        parameters: params,
        reservations: Reservations {
            ram_bytes: (8 * 1024 * 1024 * 1024u64).to_string(),
            vram_bytes: "0".into(),
            cpu_threads: 4,
        },
        deadline_monotonic_ns: "0".into(),
        output_scope_token: "test-scope".into(),
        cloud_consent_id: None,
    }
}

/// Budgets looser than each manifest's defaults — the runner clamps
/// anyway; qualification asserts real work finishes inside them.
fn budget(cpu: u64, mem: u64, wall_ns: u64) -> JobBudget {
    JobBudget {
        cpu_seconds: cpu,
        memory_bytes: mem,
        vram_bytes: 0,
        deadline_monotonic_ns: 0,
        wall_ns,
    }
}

fn ctx<'a>(rt: &'a WorkerRuntime, b: JobBudget) -> RunContext<'a> {
    RunContext { runtime: rt, budget: b, model_version: Some("1.0.0".into()) }
}

fn run_succeeded(
    f: &Fixture,
    runtime: &WorkerRuntime,
    s: JobSpec,
    b: JobBudget,
) -> (Vec<void_jobs::ArtifactRecord>, u64) {
    f.db.submit(&s).unwrap();
    let r = runner(f);
    let out = r.run(&s, &ctx(runtime, b)).unwrap();
    match out {
        RunOutcome::Succeeded { artifacts, wall_ns, stray_files, warnings } => {
            assert_eq!(stray_files, 0, "stray files in staging");
            assert!(warnings.iter().all(|w| !w.is_empty()));
            (artifacts, wall_ns)
        }
        other => panic!("expected success, got {other:?}"),
    }
}

fn asset_bytes(rec: &void_jobs::ArtifactRecord) -> Vec<u8> {
    fs::read(&rec.asset_abs).unwrap()
}

fn verify_manifest(path: &Path) -> ModelManifest {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
    ModelManifest::parse_and_verify(&bytes)
        .unwrap_or_else(|e| panic!("manifest {path:?} failed verification: {e}"))
}

fn file_sha256(path: &Path) -> String {
    use sha2::Digest;
    let mut h = sha2::Sha256::new();
    h.update(&fs::read(path).unwrap_or_else(|e| panic!("read {path:?}: {e}")));
    h.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

// ---------- manifests: signatures + runtime/artifact integrity ----------

/// Every shipped manifest verifies under the real
/// `ModelManifest::parse_and_verify`, its declared executable sha
/// matches the on-disk launcher, and every required artifact hash
/// matches the vendored weight file it names.
#[test]
fn w15_manifests_verify_and_resolve() {
    let cases: &[(&str, &str, &[(&str, &str)])] = &[
        (
            "transcribe",
            "void-audio-transcribe",
            &[("nmp.tflite", "transcribe/.venv/lib/python3.10/site-packages/basic_pitch/saved_models/icassp_2022/nmp.tflite")],
        ),
        (
            "separate",
            "void-audio-separate",
            &[
                ("5c90dfd2.safetensors", "separate/models/hf/hub/models--adefossez--HTDemucs-6s/snapshots/3c5ee475be622df764938de97e4281a7b07ffa58/5c90dfd2.safetensors"),
                ("htdemucs_6s.yaml", "separate/models/hf/hub/models--adefossez--HTDemucs-6s/snapshots/3c5ee475be622df764938de97e4281a7b07ffa58/htdemucs_6s.yaml"),
            ],
        ),
        (
            "generate",
            "void-audio-generate",
            &[
                ("state_dict.bin", "generate/models/hf/hub/models--facebook--musicgen-small/snapshots/4c8334b02c6ec4e8664a91979669a501ec497792/state_dict.bin"),
                ("compression_state_dict.bin", "generate/models/hf/hub/models--facebook--musicgen-small/snapshots/4c8334b02c6ec4e8664a91979669a501ec497792/compression_state_dict.bin"),
                ("model.safetensors", "generate/models/hf/hub/models--t5-base/snapshots/a9723ea7f1b39c1eae772870f3b547bf6ef7e6c1/model.safetensors"),
                ("spiece.model", "generate/models/hf/hub/models--t5-base/snapshots/a9723ea7f1b39c1eae772870f3b547bf6ef7e6c1/spiece.model"),
                ("tokenizer.json", "generate/models/hf/hub/models--t5-base/snapshots/a9723ea7f1b39c1eae772870f3b547bf6ef7e6c1/tokenizer.json"),
                ("config.json", "generate/models/hf/hub/models--t5-base/snapshots/a9723ea7f1b39c1eae772870f3b547bf6ef7e6c1/config.json"),
            ],
        ),
        ("generate-procedural", "void-audio-generate-procedural", &[]),
    ];
    for (dir, launcher, files) in cases {
        let m = verify_manifest(&worker_dir(dir).join("manifest.json"));
        assert_eq!(m.runtime.executable, *launcher, "{dir}");
        assert_eq!(m.runtime.worker_protocol, 1);
        let exe = worker_dir(dir).join(launcher);
        assert!(exe.is_file(), "launcher missing: {exe:?}");
        if let Some(declared) = &m.runtime.executable_sha256 {
            assert_eq!(declared, &file_sha256(&exe), "{dir} launcher sha drift");
        }
        // Required artifact shas match the vendored files they name.
        assert_eq!(m.artifacts.len(), files.len(), "{dir} artifact table");
        for (a, (label, rel)) in m.artifacts.iter().zip(files.iter()) {
            let p = repo_root().join("workers/audio").join(rel);
            assert_eq!(&a.sha256, &file_sha256(&p), "{dir}:{label} weight drift");
            assert_eq!(a.bytes.parse::<u64>().unwrap(), fs::metadata(&p).unwrap().len());
        }
    }
}

// ---------- T61: transcription via real basic-pitch worker ----------

#[test]
fn t61_transcribe_basic_pitch_onset_accuracy() {
    let f = fixture();
    let wav = synth_melody_wav();
    let input = f.store.import_bytes(&wav, "wav").unwrap();
    let rt = WorkerRuntime::new(worker_exe("transcribe", "void-audio-transcribe"));
    let s = spec(
        JobKind::Transcription,
        "void-audio-transcribe",
        "void.audio.transcribe-basic-pitch",
        vec![input.sha256.clone()],
        serde_json::json!({"input": input.sha256}),
    );
    let jid = s.job_id.clone();
    let (arts, wall_ns) = run_succeeded(&f, &rt, s, budget(240, 4 << 30, 300_000_000_000));
    let mid = arts.iter().find(|a| a.name == "transcription.mid").expect("transcription.mid");
    let data = asset_bytes(mid);
    let onsets = midi_note_ons(&data);

    // Accuracy contract: >= 4 notes detected on a clean 8-note melody,
    // >= half the detections within 80 ms of a ground-truth onset.
    assert!(onsets.len() >= 4, "too few notes detected: {onsets:?}");
    assert!(onsets.len() <= 64, "implausible note count: {}", onsets.len());
    let near = onsets
        .iter()
        .filter(|t| TRUTH.iter().any(|(_, gt)| (*t - gt).abs() <= 0.080))
        .count();
    assert!(
        near * 2 >= onsets.len(),
        "onset alignment poor: {near}/{} within 80 ms of truth {onsets:?}",
        onsets.len()
    );
    // metrics.json published with real inference numbers.
    let metrics = arts.iter().find(|a| a.name == "metrics.json").expect("metrics.json");
    let m: serde_json::Value = serde_json::from_slice(&asset_bytes(metrics)).unwrap();
    assert_eq!(m["worker"], "void-audio-transcribe");
    assert!(m["inferenceMs"].as_u64().unwrap() > 0);
    assert!(m["peakRssBytes"].as_u64().unwrap() > 10_000_000);
    assert_eq!(
        m["model"]["weightsSha256"],
        "3db297d54af8e01c6e5618245c956b1d71b6a2b978cb2dedb527173186552676"
    );
    // provenance published through the real runner path.
    let prov: serde_json::Value = serde_json::from_slice(
        &fs::read(f.root.path().join("jobs").join(&jid).join("provenance.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(prov["modelId"], "void.audio.transcribe-basic-pitch");
    println!(
        "t61 transcribe: {} notes, {} aligned, wall {} ms",
        onsets.len(),
        near,
        wall_ns / 1_000_000u64
    );
}

// ---------- T61: separation via real htdemucs_6s worker ----------

#[test]
fn t61_separate_htdemucs_6s_stems_align() {
    let f = fixture();
    let wav = synth_melody_wav();
    let input = f.store.import_bytes(&wav, "wav").unwrap();
    let rt = WorkerRuntime::new(worker_exe("separate", "void-audio-separate"));
    let s = spec(
        JobKind::Separation,
        "void-audio-separate",
        "void.audio.separate-htdemucs-6s",
        vec![input.sha256.clone()],
        serde_json::json!({"input": input.sha256, "model": "htdemucs_6s"}),
    );
    let (arts, wall_ns) = run_succeeded(&f, &rt, s, budget(600, 6 << 30, 600_000_000_000));
    let stems: Vec<&void_jobs::ArtifactRecord> =
        arts.iter().filter(|a| a.name.starts_with("stems/")).collect();
    // htdemucs_6s → exactly 6 stems (drums bass other vocals guitar piano).
    assert_eq!(stems.len(), 6, "stem set: {:?}", arts.iter().map(|a| &a.name).collect::<Vec<_>>());
    let names: Vec<&str> = stems.iter().map(|a| a.name.as_str()).collect();
    for want in ["stems/drums.wav", "stems/bass.wav", "stems/other.wav", "stems/vocals.wav", "stems/guitar.wav", "stems/piano.wav"] {
        assert!(names.contains(&want), "missing {want}");
    }
    // Channel/time alignment: every stem carries exactly the input's
    // PCM frame count at the model's 44.1 kHz rate (input resampled
    // by demucs) — sum-length ≈ input length.
    let in_secs = (wav_data_bytes(&wav) / 4) as f64 / SR as f64;
    for st in stems {
        let b = asset_bytes(st);
        assert_eq!(&b[..4], b"RIFF");
        let sr = wav_sample_rate(&b);
        let secs = (wav_data_bytes(&b) / 4) as f64 / sr as f64;
        assert!(
            (secs - in_secs).abs() < 0.05,
            "{} length {secs:.3}s vs input {in_secs:.3}s",
            st.name
        );
    }
    let metrics = arts.iter().find(|a| a.name == "metrics.json").expect("metrics.json");
    let m: serde_json::Value = serde_json::from_slice(&asset_bytes(metrics)).unwrap();
    assert_eq!(m["stemCount"], 6);
    println!("t61 separate: 6 stems aligned, wall {} ms", wall_ns / 1_000_000u64);
}

// ---------- T60: generation — procedural worker determinism ----------

#[test]
fn t60_generate_procedural_deterministic_bytes() {
    let f = fixture();
    let rt = WorkerRuntime::new(worker_exe("generate-procedural", "void-audio-generate-procedural"));
    let mk = || {
        spec(
            JobKind::AudioGeneration,
            "void-audio-generate-procedural",
            "void.audio.generate-procedural",
            vec![],
            serde_json::json!({"seed": 7, "secondsPerChord": 0.6, "sampleRate": 48000}),
        )
    };
    let (a1, w1) = run_succeeded(&f, &rt, mk(), budget(120, 1 << 30, 120_000_000_000));
    let (a2, w2) = run_succeeded(&f, &rt, mk(), budget(120, 1 << 30, 120_000_000_000));
    let w1_sha = a1.iter().find(|a| a.name == "generated.wav").unwrap().sha256.clone();
    let w2_sha = a2.iter().find(|a| a.name == "generated.wav").unwrap().sha256.clone();
    // Determinism contract: identical spec → byte-identical WAV.
    assert_eq!(w1_sha, w2_sha, "procedural generation not deterministic");
    let data = asset_bytes(a1.iter().find(|a| a.name == "generated.wav").unwrap());
    assert_eq!(&data[..4], b"RIFF");
    assert_eq!(wav_sample_rate(&data), 48_000);
    let secs = (wav_data_bytes(&data) / 4) as f64 / 48_000.0;
    assert!((secs - 4.0 * 0.6 - 0.35).abs() < 0.05, "duration {secs}");
    let notes: serde_json::Value = serde_json::from_slice(
        &asset_bytes(a1.iter().find(|a| a.name == "notes.json").unwrap()),
    )
    .unwrap();
    assert_eq!(notes["notes"].as_array().unwrap().len(), 16);
    println!("t60 procedural: deterministic sha {w1_sha}, walls {}+{} ms", w1 / 1_000_000u64, w2 / 1_000_000u64);
}

// ---------- T60: generation — real musicgen-small worker ----------

#[test]
fn t60_generate_musicgen_small_real_audio() {
    let f = fixture();
    let rt = WorkerRuntime::new(worker_exe("generate", "void-audio-generate"));
    let s = spec(
        JobKind::AudioGeneration,
        "void-audio-generate",
        "void.audio.generate-musicgen-small",
        vec![],
        serde_json::json!({"prompt": "80s synth arpeggio, warm analog bass", "durationS": 4, "seed": 42}),
    );
    let jid = s.job_id.clone();
    let (arts, wall_ns) = run_succeeded(&f, &rt, s, budget(900, 9 << 30, 600_000_000_000));
    let wav_rec = arts.iter().find(|a| a.name == "generated.wav").expect("generated.wav");
    let data = asset_bytes(wav_rec);
    assert_eq!(&data[..4], b"RIFF");
    assert_eq!(&data[8..12], b"WAVE");
    // MusicGen renders mono at its own 32 kHz model rate.
    let sr = wav_sample_rate(&data);
    assert_eq!(sr, 32_000, "musicgen native rate");
    let secs = (wav_data_bytes(&data) / 2) as f64 / sr as f64; // mono
    assert!((secs - 4.0).abs() < 0.4, "rendered {secs:.2}s for 4s request");
    // Real synthesis: not silence — RMS energy must be nonzero.
    let pcm_off = data.len() - wav_data_bytes(&data);
    let mut acc = 0f64;
    let mut n = 0usize;
    for ch in data[pcm_off..].chunks_exact(2) {
        let v = i16::from_le_bytes([ch[0], ch[1]]) as f64 / 32768.0;
        acc += v * v;
        n += 1;
    }
    let rms = (acc / n as f64).sqrt();
    assert!(rms > 0.005, "generated audio is silence (rms {rms})");
    let prov: serde_json::Value = serde_json::from_slice(
        &fs::read(f.root.path().join("jobs").join(&jid).join("provenance.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(prov["modelId"], "void.audio.generate-musicgen-small");
    let metrics: serde_json::Value = serde_json::from_slice(
        &asset_bytes(arts.iter().find(|a| a.name == "metrics.json").unwrap()),
    )
    .unwrap();
    assert_eq!(metrics["worker"], "void-audio-generate");
    println!(
        "t60 musicgen: {secs:.2}s audio rms {rms:.4}, wall {} ms, sha {}",
        wall_ns / 1_000_000u64,
        wav_rec.sha256
    );
}

// ---------- T62: cancel generation mid-run, then retry ----------

#[test]
fn t62_cancel_and_retry_generation() {
    let f = fixture();
    let r = runner(&f);
    let token = r.cancel_token();
    let s = spec(
        JobKind::AudioGeneration,
        "void-audio-generate",
        "void.audio.generate-musicgen-small",
        vec![],
        // 30 s render keeps the job alive long enough to cancel.
        serde_json::json!({"prompt": "slow cinematic drone", "durationS": 30, "seed": 1}),
    );
    f.db.submit(&s).unwrap();
    let jid = s.job_id.clone();
    let s_run = s.clone();
    let handle = thread::spawn(move || {
        r.run(&s_run, &ctx(t62_runtime(), budget(900, 9 << 30, 600_000_000_000)))
    });
    // Wait until the job is actually running before cancelling.
    for _ in 0..600 {
        if f.db.get(&jid).unwrap().status == JobStatus::Running {
            break;
        }
        thread::sleep(Duration::from_millis(100));
    }
    let db2 = JobDb::open(f.root.path().join("index.db")).unwrap();
    db2.cancel(&jid).unwrap(); // running → cancelling
    token.cancel(); // runner-side cooperative cancel
    let out = handle.join().unwrap().unwrap();
    assert!(matches!(out, RunOutcome::Cancelled { .. }), "{out:?}");
    assert_eq!(f.db.get(&jid).unwrap().status, JobStatus::Cancelled);
    // Cancelled generation commits nothing — no provenance, no
    // artifacts, no staging left behind (T62 cancel path).
    assert!(!f.root.path().join("jobs").join(&jid).join("provenance.json").exists());
    assert!(!f.root.path().join("staging").join(format!("job-{jid}")).exists());

    // Retry the same request as a fresh job — the cancelled attempt is
    // never mutated, the retry succeeds and imports real output.
    let retry = spec(
        JobKind::AudioGeneration,
        "void-audio-generate",
        "void.audio.generate-musicgen-small",
        vec![],
        serde_json::json!({"prompt": "slow cinematic drone", "durationS": 2, "seed": 1}),
    );
    let retry_id = retry.job_id.clone();
    let (arts, wall_ns) = run_succeeded(
        &f,
        t62_runtime(),
        retry,
        budget(900, 9 << 30, 600_000_000_000),
    );
    let wav_rec = arts.iter().find(|a| a.name == "generated.wav").expect("generated.wav");
    assert_eq!(wav_rec.sha256.len(), 64);
    assert_eq!(f.db.get(&retry_id).unwrap().status, JobStatus::Succeeded);
    // cancelled job still reads Cancelled — history preserved.
    assert_eq!(f.db.get(&jid).unwrap().status, JobStatus::Cancelled);
    println!("t62 cancel+retry: cancelled {jid}, retry {retry_id} ok in {} ms", wall_ns / 1_000_000u64);
}

fn t62_runtime() -> &'static WorkerRuntime {
    static RT: std::sync::OnceLock<WorkerRuntime> = std::sync::OnceLock::new();
    RT.get_or_init(|| WorkerRuntime::new(worker_exe("generate", "void-audio-generate")))
}
