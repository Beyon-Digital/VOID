//! void-symbolic-worker — deterministic symbolic-completion argv worker
//! (workers/PROTOCOL.md v1, CONTRACTS.md §6, W13/T53–T56).
//!
//! stdin ← one `job/1.0.0` spec JSON; `parameters` carries the scoped
//! context digest (seed notes, tempo/meter, region bounds, locked
//! ranges, label strings — all inert data). stdout → progress lines +
//! one result line. The proposal document lands in the staging dir as
//! `proposals.json` — the only artifact declared.
//!
//! The generator is a real interval-Markov + key-estimation model
//! (gen.rs): seeded, deterministic, zero network, zero reads outside
//! what the spec delivers. Labels/lyrics are parsed-but-unused — model
//! output never mints object ids, paths, or ops (T56).

use serde::Deserialize;
use std::env;
use std::fs;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::exit;

mod gen;

use gen::{Model, NoteEvent, Rng, KEY_NAMES};

pub const GENERATOR_ID: &str = "void-symbolic-worker";
pub const GENERATOR_VERSION: &str = "1.0.0";
pub const MODEL_ID: &str = "interval-markov-1";

const MAX_INPUT_NOTES: usize = 8192;
const MAX_PROPOSALS: usize = 8;
const MAX_CANDIDATE_NOTES: usize = 1024;
const MAX_CONTINUATION_TICKS: i64 = 256 * 4 * gen::TICKS_PER_QUARTER; // 256 bars of 4/4
const MAX_LABELS: usize = 64;

// ---------------------------------------------------------------------------
// Spec parsing — typed, bounded, inert. Unknown fields are ignored by
// serde; hostile strings can only ever land in `labels`, which the
// generator never reads.
// ---------------------------------------------------------------------------

mod de_i64 {
    use serde::{Deserialize, Deserializer};
    pub fn de<'de, D: Deserializer<'de>>(d: D) -> Result<i64, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        match v {
            serde_json::Value::String(s) => {
                s.trim().parse::<i64>().map_err(serde::de::Error::custom)
            }
            serde_json::Value::Number(n) => n
                .as_i64()
                .ok_or_else(|| serde::de::Error::custom("non-integer")),
            _ => Err(serde::de::Error::custom("expected decimal string")),
        }
    }
}

mod de_u64 {
    use serde::{Deserialize, Deserializer};
    pub fn de<'de, D: Deserializer<'de>>(d: D) -> Result<u64, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        match v {
            serde_json::Value::String(s) => {
                s.trim().parse::<u64>().map_err(serde::de::Error::custom)
            }
            serde_json::Value::Number(n) => n
                .as_u64()
                .ok_or_else(|| serde::de::Error::custom("non-integer")),
            _ => Err(serde::de::Error::custom("expected decimal string")),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpecNote {
    pitch: i32,
    #[serde(default = "dvel")]
    velocity: i32,
    #[serde(deserialize_with = "de_i64::de")]
    onset_ticks: i64,
    #[serde(deserialize_with = "de_i64::de")]
    length_ticks: i64,
}
fn dvel() -> i32 {
    96
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SpecRange {
    #[serde(deserialize_with = "de_i64::de")]
    start_ticks: i64,
    #[serde(deserialize_with = "de_i64::de")]
    length_ticks: i64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkerParams {
    /// Deterministic seed (decimal string). Coordinator supplies it
    /// (context-derived when the caller omitted one); echoed back into
    /// the document and provenance.
    #[serde(deserialize_with = "de_u64::de", default = "dseed")]
    seed: u64,
    #[serde(default = "dmax")]
    max_proposals: usize,
    #[serde(deserialize_with = "de_i64::de")]
    continuation_start_ticks: i64,
    #[serde(deserialize_with = "de_i64::de")]
    continuation_ticks: i64,
    #[serde(default)]
    track_id: String,
    #[serde(default)]
    clip_id: String,
    #[serde(default)]
    tempo_bpm: Option<f64>,
    #[serde(default)]
    ts_num: Option<u32>,
    #[serde(default)]
    ts_den: Option<u32>,
    #[serde(default)]
    key_hint: Option<String>,
    #[serde(default)]
    notes: Vec<SpecNote>,
    #[serde(default)]
    locked_ranges: Vec<SpecRange>,
    #[serde(default)]
    labels: Vec<String>,
    /// Test hook: "fail" reports a failed result line.
    #[serde(default)]
    mode: Option<String>,
}
fn dseed() -> u64 {
    0
}
fn dmax() -> usize {
    4
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct JobSpec {
    #[serde(default)]
    job_id: String,
    parameters: WorkerParams,
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

fn emit(line: String) {
    println!("{line}");
    let _ = std::io::stdout().flush();
}

fn progress(percent: u32, message: &str) {
    emit(format!(
        "{{\"v\":1,\"kind\":\"progress\",\"percent\":{percent},\"message\":\"{}\"}}",
        esc(message)
    ));
}

fn result_ok(artifacts: &[String], warnings: &[String]) {
    let arts = artifacts
        .iter()
        .map(|a| format!("{{\"path\":\"{}\"}}", esc(a)))
        .collect::<Vec<_>>()
        .join(",");
    let warns = warnings
        .iter()
        .map(|w| format!("\"{}\"", esc(w)))
        .collect::<Vec<_>>()
        .join(",");
    emit(format!(
        "{{\"v\":1,\"kind\":\"result\",\"status\":\"succeeded\",\"artifacts\":[{arts}],\"warnings\":[{warns}],\"error\":null}}"
    ));
}

fn result_fail(msg: &str) {
    emit(format!(
        "{{\"v\":1,\"kind\":\"result\",\"status\":\"failed\",\"artifacts\":[],\"warnings\":[],\"error\":\"{}\"}}",
        esc(msg)
    ));
}

// ---------------------------------------------------------------------------
// Generation + document serialization
// ---------------------------------------------------------------------------

struct Candidate {
    signature: Vec<(i32, i64, i64)>,
    notes: Vec<NoteEvent>,
    score: f64,
    snapped: u32,
    fallback: bool,
}

fn signature(notes: &[NoteEvent]) -> Vec<(i32, i64, i64)> {
    notes.iter().map(|n| (n.pitch, n.onset, n.length)).collect()
}

fn key_name(root: i32, minor: bool) -> String {
    format!(
        "{} {}",
        KEY_NAMES[root as usize % 12],
        if minor { "minor" } else { "major" }
    )
}

/// Generate up to `want` distinct candidates, ranked by model score.
fn generate_candidates(
    model: &Model,
    seed: u64,
    start: i64,
    span: i64,
    want: usize,
) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    let mut seen: Vec<Vec<(i32, i64, i64)>> = Vec::new();
    // Bounded retry budget: distinct sub-seeds per attempt keep the
    // process deterministic while making collision loops impossible.
    for attempt in 0..(want * 8 + 8) {
        if out.len() >= want {
            break;
        }
        let mut rng =
            Rng::new(seed.wrapping_add(0x9E37_79B9_7F4A_7C15u64.wrapping_mul(attempt as u64 + 1)));
        let (notes, score, facts) = if model.fallback {
            model.generate_fallback(&mut rng, start, span, MAX_CANDIDATE_NOTES)
        } else {
            model.generate(&mut rng, start, span, MAX_CANDIDATE_NOTES)
        };
        if notes.is_empty() {
            continue;
        }
        let sig = signature(&notes);
        if seen.iter().any(|s| *s == sig) {
            continue; // duplicate melody — resample with next sub-seed
        }
        seen.push(sig.clone());
        out.push(Candidate {
            signature: sig,
            notes,
            score,
            snapped: facts.snapped,
            fallback: facts.fallback,
        });
    }
    // Rank by the model's own likelihood, descending; stable order for ties.
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.signature.cmp(&b.signature))
    });
    out
}

fn rationale(c: &Candidate, rank: usize, model: &Model) -> String {
    if c.fallback {
        return format!(
            "scale-walk continuation in {} — fewer than 2 seed notes; grid {}t",
            key_name(model.key_root, model.key_minor),
            model.grid
        );
    }
    let mut s = format!(
        "interval-chain continuation #{rank}; key≈{} (r={:.2}), dominant interval {:+}st, grid {}t",
        key_name(model.key_root, model.key_minor),
        model.key_corr,
        model.dominant_interval,
        model.grid
    );
    if c.snapped > 0 {
        s.push_str(&format!("; {} scale-snapped", c.snapped));
    }
    s
}

fn note_json(n: &NoteEvent) -> String {
    format!(
        "{{\"pitch\":{},\"velocity\":{},\"onsetTicks\":\"{}\",\"lengthTicks\":\"{}\"}}",
        n.pitch, n.velocity, n.onset, n.length
    )
}

fn document_json(params: &WorkerParams, model: &Model, candidates: &[Candidate]) -> String {
    let mut o = String::with_capacity(4096);
    o.push_str("{");
    o.push_str("\"v\":1,");
    o.push_str("\"doc\":\"void-proposals/1\",");
    o.push_str(&format!(
        "\"generator\":{{\"id\":\"{}\",\"version\":\"{}\",\"model\":\"{}\"}},",
        GENERATOR_ID, GENERATOR_VERSION, MODEL_ID
    ));
    o.push_str(&format!("\"seed\":\"{}\",", params.seed));
    o.push_str(&format!(
        "\"analysis\":{{\"noteCount\":{},\"keyEstimate\":\"{}\",\"keyCorrelation\":{:.4},\"gridTicks\":\"{}\",\"dominantInterval\":{},\"fallback\":{}}},",
        model.note_count,
        esc(&key_name(model.key_root, model.key_minor)),
        model.key_corr,
        model.grid,
        model.dominant_interval,
        model.fallback
    ));
    o.push_str(&format!(
        "\"continuation\":{{\"startTicks\":\"{}\",\"lengthTicks\":\"{}\",\"trackId\":\"{}\",\"clipId\":\"{}\"}},",
        params.continuation_start_ticks,
        params.continuation_ticks,
        esc(&params.track_id),
        esc(&params.clip_id)
    ));
    o.push_str("\"proposals\":[");
    for (i, c) in candidates.iter().enumerate() {
        if i > 0 {
            o.push(',');
        }
        let notes = c.notes.iter().map(note_json).collect::<Vec<_>>().join(",");
        o.push_str(&format!(
            "{{\"id\":\"c{}\",\"rank\":{},\"score\":{:.6},\"rationale\":\"{}\",\"notes\":[{}]}}",
            i + 1,
            i + 1,
            c.score,
            esc(&rationale(c, i + 1, model)),
            notes
        ));
    }
    o.push_str("]}");
    o
}

fn validate_params(p: &WorkerParams) -> Result<(), String> {
    if p.notes.len() > MAX_INPUT_NOTES {
        return Err(format!(
            "too many seed notes ({} > {MAX_INPUT_NOTES})",
            p.notes.len()
        ));
    }
    if p.continuation_ticks <= 0 || p.continuation_ticks > MAX_CONTINUATION_TICKS {
        return Err("continuationTicks out of bounds".into());
    }
    if p.max_proposals == 0 || p.max_proposals > MAX_PROPOSALS {
        return Err(format!("maxProposals out of bounds (1..={MAX_PROPOSALS})"));
    }
    if p.labels.len() > MAX_LABELS {
        return Err("labels bound exceeded".into());
    }
    for n in &p.notes {
        if !(0..=127).contains(&n.pitch) || !(1..=127).contains(&n.velocity) || n.length_ticks <= 0
        {
            return Err("seed note out of bounds (pitch 0..127, velocity 1..127, length>0)".into());
        }
    }
    Ok(())
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
            eprintln!("void-symbolic-worker: --staging <dir> required");
            exit(64);
        }
    };

    let mut spec = String::new();
    if std::io::stdin().read_to_string(&mut spec).is_err() {
        result_fail("could not read job spec on stdin");
        exit(0);
    }
    progress(5, "spec read");

    let spec: JobSpec = match serde_json::from_str(&spec) {
        Ok(s) => s,
        Err(e) => {
            // Safe message: serde errors carry no secrets/paths.
            result_fail(&format!("job spec parse failed: {e}"));
            exit(0);
        }
    };
    let params = spec.parameters;
    if let Err(e) = validate_params(&params) {
        result_fail(&e);
        exit(0);
    }
    if params.mode.as_deref() == Some("fail") {
        result_fail("requested failure (mode=fail)");
        exit(0);
    }

    progress(15, "training interval model");
    let notes: Vec<NoteEvent> = params
        .notes
        .iter()
        .map(|n| NoteEvent {
            pitch: n.pitch,
            velocity: n.velocity,
            onset: n.onset_ticks,
            length: n.length_ticks,
        })
        .collect();
    let model = Model::train(&notes, params.key_hint.as_deref());

    progress(40, "generating candidates");
    let mut warnings: Vec<String> = Vec::new();
    if model.fallback {
        warnings.push("fewer than 2 seed notes — scale-walk fallback used".into());
    }
    let candidates = generate_candidates(
        &model,
        params.seed,
        params.continuation_start_ticks,
        params.continuation_ticks,
        params.max_proposals,
    );
    if candidates.is_empty() {
        result_fail("no proposals could be generated for this context");
        exit(0);
    }
    if candidates.len() < params.max_proposals {
        warnings.push(format!(
            "only {} distinct continuations (requested {})",
            candidates.len(),
            params.max_proposals
        ));
    }

    progress(80, "writing proposal document");
    let doc = document_json(&params, &model, &candidates);
    let out = staging.join("proposals.json");
    if fs::write(&out, doc.as_bytes()).is_err() {
        result_fail("could not write proposals.json");
        exit(0);
    }
    progress(95, "done");
    result_ok(&["proposals.json".to_string()], &warnings);
    exit(0);
}
