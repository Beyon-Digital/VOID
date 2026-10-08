//! void-fake-worker — reference void-job-worker (protocol v1).
//!
//! A REAL but trivial deterministic synthesizer used by the void-jobs
//! runner boundary tests. It genuinely renders output bytes per run —
//! sine WAV or a tiny MIDI pattern — driven entirely by the job spec's
//! `parameters` object. Nothing is canned: change the parameters and
//! the artifact bytes change.
//!
//! Protocol: spec JSON on stdin → progress JSON lines on stdout → one
//! final result line. `--staging <dir>` is the only write scope.
//!
//! Test modes (`parameters.mode`) exist to exercise runner boundaries:
//!   succeed (default) · fail · crash · sleep · oom · spin · escape
//!
//! Intentionally std-only: a worker bundle must not need a runtime the
//! user has to install.

use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::exit;
use std::time::Duration;

// ---------------------------------------------------------------------------
// Minimal tolerant JSON field extraction (std-only).
// The runner emits serde_json output for the job spec; the fields this
// fixture needs are unique keys, so a shallow scan suffices. Documented
// scope: this is not a general JSON parser.
// ---------------------------------------------------------------------------

fn json_get_str<'a>(doc: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("\"{key}\"");
    let i = doc.find(&pat)? + pat.len();
    let rest = &doc[i..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    if !after.starts_with('"') {
        return None;
    }
    let end = after[1..].find('"')?;
    Some(&after[1..1 + end])
}

fn json_get_num(doc: &str, key: &str) -> Option<i64> {
    let pat = format!("\"{key}\"");
    let i = doc.find(&pat)? + pat.len();
    let rest = &doc[i..];
    let colon = rest.find(':')?;
    let after = rest[colon + 1..].trim_start();
    let end = after
        .find(|c: char| !(c.is_ascii_digit() || c == '-' || c == '.'))
        .unwrap_or(after.len());
    after[..end].trim_end().parse::<f64>().ok().map(|f| f as i64)
}

fn emit(line: String) {
    println!("{line}");
    let _ = std::io::stdout().flush();
}

fn esc(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c if (c as u32) < 0x20 => o.push_str(&format!("\\u{:04x}", c as u32)),
            c => o.push(c),
        }
    }
    o
}

fn progress(percent: u32, message: &str) {
    emit(format!(
        "{{\"v\":1,\"kind\":\"progress\",\"percent\":{percent},\"message\":\"{}\"}}",
        esc(message)
    ));
}

fn result_ok(artifacts: &[String]) {
    let arts = artifacts
        .iter()
        .map(|a| format!("{{\"path\":\"{}\"}}", esc(a)))
        .collect::<Vec<_>>()
        .join(",");
    emit(format!(
        "{{\"v\":1,\"kind\":\"result\",\"status\":\"succeeded\",\"artifacts\":[{arts}],\"warnings\":[],\"error\":null}}"
    ));
}

fn result_fail(msg: &str) {
    emit(format!(
        "{{\"v\":1,\"kind\":\"result\",\"status\":\"failed\",\"artifacts\":[],\"warnings\":[],\"error\":\"{}\"}}",
        esc(msg)
    ));
}

// ---------------------------------------------------------------------------
// Real synthesis — deterministic outputs
// ---------------------------------------------------------------------------

/// 16-bit PCM mono WAV: sine at `freq_hz`, `duration_ms`, `sample_rate`.
/// `seed` shifts the phase so parameter changes change the bytes.
fn render_sine_wav(path: &PathBuf, freq_hz: f64, duration_ms: u64, sample_rate: u32, seed: u64) {
    let frames = (sample_rate as u64 * duration_ms / 1000) as usize;
    let mut data = Vec::with_capacity(44 + frames * 2);
    let byte_rate = sample_rate * 2;
    let data_len = (frames * 2) as u32;
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&(36 + data_len).to_le_bytes());
    data.extend_from_slice(b"WAVEfmt ");
    data.extend_from_slice(&16u32.to_le_bytes());
    data.extend_from_slice(&1u16.to_le_bytes()); // PCM
    data.extend_from_slice(&1u16.to_le_bytes()); // mono
    data.extend_from_slice(&sample_rate.to_le_bytes());
    data.extend_from_slice(&byte_rate.to_le_bytes());
    data.extend_from_slice(&2u16.to_le_bytes()); // block align
    data.extend_from_slice(&16u16.to_le_bytes());
    data.extend_from_slice(b"data");
    data.extend_from_slice(&data_len.to_le_bytes());
    let phase = (seed % 628) as f64 / 100.0;
    for i in 0..frames {
        let t = i as f64 / sample_rate as f64;
        let s = (2.0 * std::f64::consts::PI * freq_hz * t + phase).sin() * 0.5;
        let v = (s * 32767.0) as i16;
        data.extend_from_slice(&v.to_le_bytes());
    }
    fs::write(path, &data).expect("write wav");
}

/// SMF format-0: tempo + 4-note pattern derived from `seed`.
fn render_tiny_midi(path: &PathBuf, seed: u64) {
    let mut data: Vec<u8> = Vec::new();
    data.extend_from_slice(b"MThd");
    data.extend_from_slice(&6u32.to_be_bytes());
    data.extend_from_slice(&0u16.to_be_bytes()); // format 0
    data.extend_from_slice(&1u16.to_be_bytes()); // one track
    data.extend_from_slice(&480u16.to_be_bytes()); // 480 tpq
    let mut track: Vec<u8> = Vec::new();
    // tempo meta event: 500000 us/qn = 120 bpm
    track.extend_from_slice(&[0x00, 0xFF, 0x51, 0x03, 0x07, 0xA1, 0x20]);
    let base = 60u8 + (seed % 12) as u8;
    for (i, semis) in [0u8, 4, 7, 12].iter().enumerate() {
        let note = base + *semis;
        if i == 0 {
            track.extend_from_slice(&[0x00, 0x90, note, 100]);
        } else {
            // delta 480 ticks (VLQ 0x83 0x60) → note-on of this note.
            track.extend_from_slice(&[0x83, 0x60, 0x90, note, 100]);
        }
        // note-off 480 ticks later.
        track.extend_from_slice(&[0x83, 0x60, 0x80, note, 0]);
    }
    track.extend_from_slice(&[0x00, 0xFF, 0x2F, 0x00]);
    data.extend_from_slice(b"MTrk");
    data.extend_from_slice(&(track.len() as u32).to_be_bytes());
    data.extend_from_slice(&track);
    fs::write(path, &data).expect("write midi");
}

fn main() {
    let mut staging: Option<PathBuf> = None;
    let mut args = env::args().skip(1);
    while let Some(a) = args.next() {
        if a == "--staging" {
            staging = args.next().map(PathBuf::from);
        }
    }
    let staging = match staging {
        Some(s) => s,
        None => {
            eprintln!("void-fake-worker: --staging <dir> required");
            exit(64);
        }
    };

    let mut spec = String::new();
    std::io::stdin()
        .read_to_string(&mut spec)
        .expect("read spec");

    let mode = json_get_str(&spec, "mode").unwrap_or("succeed").to_string();
    let duration_ms = json_get_num(&spec, "durationMs").unwrap_or(250).max(1) as u64;
    let freq_hz = json_get_num(&spec, "freqHz").unwrap_or(440).max(1) as f64;
    let sample_rate = json_get_num(&spec, "sampleRate").unwrap_or(48_000).max(8_000) as u32;
    let seed = json_get_num(&spec, "seed").unwrap_or(0) as u64;
    let format = json_get_str(&spec, "format").unwrap_or("wav").to_string();

    progress(5, "spec parsed");

    match mode.as_str() {
        "fail" => {
            progress(20, "failing as requested");
            result_fail("requested failure (mode=fail)");
            exit(0);
        }
        "crash" => {
            progress(20, "crashing as requested");
            eprintln!("void-fake-worker: simulated crash");
            exit(3);
        }
        "sleep" => {
            progress(10, "sleeping");
            std::thread::sleep(Duration::from_millis(duration_ms.max(30_000)));
            // Falls through to a normal success after the nap.
        }
        "oom" => {
            progress(10, "allocating until the budget bites");
            let mut chunks: Vec<Vec<u8>> = Vec::new();
            loop {
                // Grow steadily — RLIMIT_AS or the allocator kills us.
                let mut v = vec![0u8; 64 * 1024 * 1024];
                v[0] = 1; // commit pages
                chunks.push(v);
            }
        }
        "spin" => {
            progress(10, "burning cpu");
            let mut x: u64 = seed;
            loop {
                x = x.wrapping_mul(6364136223846793005).wrapping_add(1);
                if x == 42 {
                    break;
                }
            }
        }
        "escape" => {
            // Worker misbehaviour fixture: writes a valid file but
            // declares an escaping path — the runner must reject it.
            let p = staging.join("out.wav");
            render_sine_wav(&p, freq_hz, duration_ms, sample_rate, seed);
            emit("{\"v\":1,\"kind\":\"result\",\"status\":\"succeeded\",\"artifacts\":[{\"path\":\"../escape.wav\"}],\"warnings\":[],\"error\":null}".into());
            exit(0);
        }
        "stray" => {
            // Writes an undeclared file alongside the declared one —
            // the runner must count it and never auto-commit it.
            let p = staging.join("stray.bin");
            fs::write(&p, b"undeclared").expect("write stray");
        }
        _ => {}
    }

    progress(30, "synthesizing");
    let (name, ext): (&str, &str) = if format == "midi" {
        ("out", "mid")
    } else {
        ("out", "wav")
    };
    let out = staging.join(format!("{name}.{ext}"));
    if ext == "mid" {
        render_tiny_midi(&out, seed);
    } else {
        render_sine_wav(&out, freq_hz, duration_ms, sample_rate, seed);
    }
    progress(90, "artifact written");
    let name = out.file_name().unwrap().to_string_lossy().to_string();
    result_ok(&[name]);
    exit(0);
}
