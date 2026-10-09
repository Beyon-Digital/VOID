//! visfx-fake-worker — argv worker (protocol v1) for the visfx suite.
//! REAL deterministic generation: reads the scene-gen spec from stdin
//! and renders `scene.json` (+ a procedural `frame0.bin` texture when
//! the spec asks for media). Output bytes genuinely derive from the
//! spec's seed/params — change them and the artifacts change.
//!
//! Test-only modes via spec `parameters.testMode`:
//!   "bad_doc"    — emit a doc carrying a forbidden action
//!   "bad_shader" — emit a doc with a malformed shaderBody
//!   "fail"       — result status=failed
//!   "stray"      — also drop an undeclared file (runner counts it)

use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::exit;

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
            c => o.push(c),
        }
    }
    o
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

/// Deterministic 4KiB procedural texture — bytes = f(seed, i).
fn render_frame0(path: &PathBuf, seed: u64) {
    let mut data = Vec::with_capacity(4096 + 8);
    data.extend_from_slice(b"VXT1");
    data.extend_from_slice(&seed.to_le_bytes());
    let mut x = seed | 1;
    for _ in 0..4096 {
        // xorshift64 — real generated content, never canned.
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        data.push((x & 0xff) as u8);
    }
    fs::write(path, data).expect("write frame0");
}

fn main() {
    let mut staging = PathBuf::from(".");
    let args: Vec<String> = env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--staging" && i + 1 < args.len() {
            staging = PathBuf::from(&args[i + 1]);
            i += 1;
        }
        i += 1;
    }

    let mut spec = String::new();
    std::io::stdin().read_to_string(&mut spec).expect("stdin spec");

    let seed = json_get_str(&spec, "seed")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    let kind = json_get_str(&spec, "kind").unwrap_or("scene_patch").to_string();
    let with_media = spec.contains("\"withMedia\":true");
    let mode = json_get_str(&spec, "testMode").unwrap_or("").to_string();

    emit("{\"v\":1,\"kind\":\"progress\",\"percent\":10,\"message\":\"parse\"}".into());

    if mode == "fail" {
        result_fail("synthetic worker failure");
        exit(0);
    }

    emit("{\"v\":1,\"kind\":\"progress\",\"percent\":55,\"message\":\"render\"}".into());

    let mut artifacts: Vec<String> = vec!["scene.json".into()];
    let mut layers = String::new();

    match mode.as_str() {
        "bad_doc" => {
            layers.push_str(&format!(
                "{{\"name\":\"gen-0\",\"channel\":\"preview\",\"kind\":\"generator\",\"generator\":{{\"preset\":\"plasma\",\"seed\":\"{seed}\",\"params\":{{}}}}}}"
            ));
            let doc = format!(
                "{{\"tag\":\"void-scene-gen\",\"generatorId\":\"visfx-fake-worker\",\"generatorVersion\":\"1.0.0\",\"seed\":\"{seed}\",\"layers\":[{layers}],\"actions\":[{{\"action\":\"exec_shell\",\"layer\":\"gen-0\",\"value\":{{}}}}],\"analysis\":{{\"kind\":\"{kind}\"}}}}"
            );
            fs::write(staging.join("scene.json"), doc).expect("scene");
        }
        "bad_shader" => {
            layers.push_str(&format!(
                "{{\"name\":\"gen-0\",\"channel\":\"preview\",\"kind\":\"generator\",\"generator\":{{\"shaderBody\":\"{body}\",\"seed\":\"{seed}\",\"params\":{{}}}}}}",
                body = "fn gen(uv: vec2<f32>, u: GenU) -> vec4<f32> {{ broken",
                seed = seed
            ));
            let doc = format!(
                "{{\"tag\":\"void-scene-gen\",\"generatorId\":\"visfx-fake-worker\",\"generatorVersion\":\"1.0.0\",\"seed\":\"{seed}\",\"layers\":[{layers}],\"actions\":[],\"analysis\":{{\"kind\":\"{kind}\"}}}}"
            );
            fs::write(staging.join("scene.json"), doc).expect("scene");
        }
        _ => {
            layers.push_str(&format!(
                "{{\"name\":\"gen-0\",\"channel\":\"preview\",\"kind\":\"generator\",\"generator\":{{\"preset\":\"plasma\",\"seed\":\"{seed}\",\"params\":{{}}}}}}"
            ));
            if with_media {
                layers.push_str(&format!(
                    ",{{\"name\":\"tex-0\",\"channel\":\"preview\",\"kind\":\"media\",\"media\":{{\"artifact\":\"frame0.bin\",\"mediaKind\":\"texture\",\"durationTicks\":\"0\"}}}}"
                ));
                render_frame0(&staging.join("frame0.bin"), seed);
                artifacts.push("frame0.bin".into());
            }
            let doc = format!(
                "{{\"tag\":\"void-scene-gen\",\"generatorId\":\"visfx-fake-worker\",\"generatorVersion\":\"1.0.0\",\"seed\":\"{seed}\",\"layers\":[{layers}],\"actions\":[{{\"action\":\"add_layer\",\"layer\":\"gen-0\",\"value\":{{}}}}],\"analysis\":{{\"kind\":\"{kind}\",\"seedEcho\":\"{seed}\"}}}}"
            );
            fs::write(staging.join("scene.json"), doc).expect("scene");
        }
    }
    if mode == "stray" {
        fs::write(staging.join("undeclared.tmp"), b"x").expect("stray");
    }

    emit("{\"v\":1,\"kind\":\"progress\",\"percent\":90,\"message\":\"pack\"}".into());
    result_ok(&artifacts);
    exit(0);
}
