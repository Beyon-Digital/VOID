//! void-fake-ffmpeg — deterministic ffmpeg/ffprobe stand-in for the
//! offline av qualification harness (W24/T88). Like void-fake-worker,
//! this is a REAL implementation of the same argv contract, not a
//! canned success: it parses argv, fails on missing inputs, writes a
//! real fixture container (VFMT), and answers `-of json` probe-back by
//! re-reading the bytes it wrote. Failure modes (hang, exit-1, bad
//! container, drift, oversized output) are honest argv/env-selected
//! behaviors so every pipeline boundary is exercised.
//!
//! Mode selection is via the declared env var VOID_FAKE_FFMPEG_MODE —
//! env_clear means nothing else leaks in, which is itself what T88
//! asserts (envdump mode records the visible env-var count).

use std::env;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::thread;
use std::time::Duration;

const MAGIC: &[u8] = b"VFMT1\n";
const PAYLOAD_PER_FRAME: u64 = 1024;

fn main() {
    let argv: Vec<String> = env::args().skip(1).collect();
    if argv.is_empty() {
        eprintln!("void-fake-ffmpeg: no args");
        exit(1);
    }
    // ffprobe-shaped call: `-of json` present.
    if argv.iter().any(|a| a == "json") && argv.iter().any(|a| a == "-of") {
        probe_mode(&argv);
        return;
    }
    if argv.iter().any(|a| a == "-version") {
        println!("void-fake-ffmpeg version 1.0.0 (offline fixture — not a real encoder)");
        return;
    }
    if argv.iter().any(|a| a == "-encoders") {
        encoders_mode();
        return;
    }
    encode_mode(&argv);
}

fn encoders_mode() {
    let default = "ffv1,flac,libvpx-vp9,libopus,libx264,aac,prores_ks,pcm_s16le";
    let list = env::var("VOID_FAKE_FFMPEG_ENCODERS").unwrap_or_else(|_| default.into());
    println!("Encoders:");
    for n in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        let flag = if is_audio_encoder(n) {
            " A..... "
        } else {
            " V..... "
        };
        println!("{flag}{n:<16} fake {n}");
    }
}

fn is_audio_encoder(n: &str) -> bool {
    matches!(
        n,
        "flac" | "libopus" | "aac" | "pcm_s16le" | "opus" | "pcm_s24le"
    )
}

#[derive(Default)]
struct Enc {
    inputs: Vec<(bool, String)>, // (is_lavfi, arg)
    fps_num: u64,
    fps_den: u64,
    frames_v: u64,
    size: String,
    t_seconds: String,
    c_v: String,
    c_a: String,
    ar: u64,
    ac: u64,
    fs_cap: u64,
    out: String,
}

fn encode_mode(argv: &[String]) {
    let mode = env::var("VOID_FAKE_FFMPEG_MODE").unwrap_or_else(|_| "ok".into());
    let mut e = Enc {
        fs_cap: u64::MAX,
        ..Enc::default()
    };
    let mut i = 0;
    let mut is_lavfi_next = false;
    while i < argv.len() {
        match argv[i].as_str() {
            "-f" => {
                i += 1;
                is_lavfi_next = argv.get(i).map(|s| s == "lavfi").unwrap_or(false);
            }
            "-i" => {
                i += 1;
                let a = argv.get(i).cloned().unwrap_or_default();
                e.inputs.push((is_lavfi_next, a));
                is_lavfi_next = false;
            }
            "-r" => {
                i += 1;
                let r = argv.get(i).cloned().unwrap_or_default();
                let mut p = r.split('/');
                e.fps_num = p.next().and_then(|x| x.parse().ok()).unwrap_or(0);
                e.fps_den = p.next().and_then(|x| x.parse().ok()).unwrap_or(0);
            }
            "-frames:v" => {
                i += 1;
                e.frames_v = argv.get(i).and_then(|x| x.parse().ok()).unwrap_or(0);
            }
            "-s" => {
                i += 1;
                e.size = argv.get(i).cloned().unwrap_or_default();
            }
            "-t" => {
                i += 1;
                e.t_seconds = argv.get(i).cloned().unwrap_or_default();
            }
            "-c:v" => {
                i += 1;
                e.c_v = argv.get(i).cloned().unwrap_or_default();
            }
            "-c:a" => {
                i += 1;
                e.c_a = argv.get(i).cloned().unwrap_or_default();
            }
            "-ar" => {
                i += 1;
                e.ar = argv.get(i).and_then(|x| x.parse().ok()).unwrap_or(0);
            }
            "-ac" => {
                i += 1;
                e.ac = argv.get(i).and_then(|x| x.parse().ok()).unwrap_or(0);
            }
            "-fs" => {
                i += 1;
                e.fs_cap = argv.get(i).and_then(|x| x.parse().ok()).unwrap_or(u64::MAX);
            }
            "-pix_fmt" | "-map" | "-b:v" | "-crf" | "-profile:v" | "-movflags" | "-v" => {
                i += 1; // value arg, skip
            }
            "-hide_banner" | "-nostdin" | "-y" | "-count_frames" | "-show_entries" | "-of" => {}
            s if !s.starts_with('-') => {
                e.out = s.to_string();
            }
            _ => {}
        }
        i += 1;
    }

    // Real input validation: a non-lavfi -i must exist on disk — same
    // failure ffmpeg would give.
    for (lavfi, p) in &e.inputs {
        if !lavfi && !Path::new(p).is_file() {
            eprintln!("{p}: No such file or directory");
            exit(1);
        }
        if !lavfi && p.contains("..") {
            eprintln!("{p}: refused path");
            exit(1);
        }
    }
    if e.out.is_empty() {
        eprintln!("no output file");
        exit(1);
    }

    match mode.as_str() {
        "hang" => {
            eprintln!("fake: hanging (timeout probe)");
            loop {
                thread::sleep(Duration::from_secs(60));
            }
        }
        "exit1" => {
            eprintln!("fake: forced encoder failure");
            exit(1);
        }
        "badheader" => {
            fs::write(&e.out, b"NOTAVFMT garbage bytes").unwrap();
            return;
        }
        _ => {}
    }

    let frames = if mode == "underframes" && e.frames_v > 0 {
        e.frames_v - 1
    } else {
        e.frames_v
    };
    // audio samples from -t seconds (µs precision) × -ar
    let us = parse_seconds_us(&e.t_seconds);
    let mut audio_samples = ((us * e.ar as u128) / 1_000_000u128) as u64;
    match mode.as_str() {
        "jitter" => audio_samples += 7,      // within one-frame tolerance
        "bigdrift" => audio_samples += e.ar, // beyond tolerance
        _ => {}
    }

    let header = format!(
        "{{\"frames\":{frames},\"fps_num\":{},\"fps_den\":{},\"audio_samples\":{audio_samples},\"ar\":{},\"ac\":{},\"vcodec\":\"{}\",\"acodec\":\"{}\",\"env_count\":{}}}\n",
        e.fps_num, e.fps_den, e.ar, e.ac, e.c_v, e.c_a,
        env::vars().count()
    );
    let total = MAGIC.len() as u64 + header.len() as u64 + frames * PAYLOAD_PER_FRAME;
    if total > e.fs_cap {
        eprintln!("fake: output {total} exceeds -fs {}", e.fs_cap);
        exit(1);
    }
    let mut f = fs::File::create(&e.out).unwrap();
    f.write_all(MAGIC).unwrap();
    f.write_all(header.as_bytes()).unwrap();
    // deterministic payload — byte pattern derived from index, not zeros
    let mut buf = vec![0u8; PAYLOAD_PER_FRAME as usize];
    for n in 0..frames {
        for (k, b) in buf.iter_mut().enumerate() {
            *b = ((n as usize + k) % 251) as u8;
        }
        f.write_all(&buf).unwrap();
    }
    f.sync_all().unwrap();
}

fn parse_seconds_us(s: &str) -> u128 {
    let mut it = s.split('.');
    let sec: u128 = it.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let frac = it.next().unwrap_or("0");
    let mut f: u128 = frac.parse().unwrap_or(0);
    for _ in frac.len()..6 {
        f *= 10;
    }
    sec * 1_000_000 + f
}

fn probe_mode(argv: &[String]) {
    let file = argv.last().cloned().unwrap_or_default();
    let path = PathBuf::from(&file);
    let data = match fs::read(&path) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("{file}: {e}");
            exit(1);
        }
    };
    if !data.starts_with(MAGIC) {
        eprintln!("{file}: Invalid data found when processing input");
        exit(1);
    }
    let rest = &data[MAGIC.len()..];
    let nl = rest.iter().position(|b| *b == b'\n').unwrap_or(0);
    let header = String::from_utf8_lossy(&rest[..nl]).to_string();
    let get = |k: &str| -> String {
        let key = format!("\"{k}\":");
        let start = header.find(&key).map(|i| i + key.len()).unwrap_or(0);
        let tail = &header[start..];
        let end = tail.find([',', '}']).unwrap_or(tail.len());
        tail[..end].trim_matches('"').to_string()
    };
    let frames: u64 = get("frames").parse().unwrap_or(0);
    let fps_num: u64 = get("fps_num").parse().unwrap_or(0);
    let fps_den: u64 = get("fps_den").parse().unwrap_or(0);
    let audio_samples: u64 = get("audio_samples").parse().unwrap_or(0);
    let ar: u64 = get("ar").parse().unwrap_or(0);
    let ac: u64 = get("ac").parse().unwrap_or(0);
    let vcodec = get("vcodec");
    let acodec = get("acodec");
    let size = data.len();
    println!(
        "{{\"streams\":[{{\"index\":0,\"codec_type\":\"video\",\"codec_name\":\"{vcodec}\",\"nb_read_frames\":\"{frames}\",\"r_frame_rate\":\"{fps_num}/{fps_den}\"}},{{\"index\":1,\"codec_type\":\"audio\",\"codec_name\":\"{acodec}\",\"sample_rate\":\"{ar}\",\"channels\":{ac},\"duration_ts\":{audio_samples},\"time_base\":\"1/{ar}\"}}],\"format\":{{\"size\":\"{size}\"}}}}"
    );
}
