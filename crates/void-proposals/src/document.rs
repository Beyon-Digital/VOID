//! The worker's `proposals.json` document — typed, bounded, validated
//! (T56: model output is data; only scoped typed note lists survive).

use crate::error::{ProposalError, Result};
use serde_json::Value;

pub const DOC_TAG: &str = "void-proposals/1";
pub const MAX_PROPOSALS: usize = 8;
pub const MAX_CANDIDATE_NOTES: usize = 1024;
pub const MAX_RATIONALE_CHARS: usize = 512;
pub const MAX_ID_CHARS: usize = 64;

#[derive(Debug, Clone, PartialEq)]
pub struct DocumentNote {
    pub pitch: i32,
    pub velocity: i32,
    pub onset_ticks: i64,
    pub length_ticks: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CandidateDoc {
    pub id: String,
    pub score: f64,
    pub rationale: String,
    pub notes: Vec<DocumentNote>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeneratorInfo {
    pub id: String,
    pub version: String,
    pub model: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProposalDocument {
    pub generator: GeneratorInfo,
    pub seed: u64,
    pub candidates: Vec<CandidateDoc>,
    /// analysis block echoed verbatim (measured facts — key estimate,
    /// grid, correlation); informational only.
    pub analysis: Value,
    pub warnings: Vec<String>,
}

fn bad(m: impl Into<String>) -> ProposalError {
    ProposalError::MalformedDocument(m.into())
}

fn i64_field(v: &Value, name: &str) -> Result<i64> {
    match v {
        Value::String(s) => s
            .trim()
            .parse::<i64>()
            .map_err(|_| bad(format!("{name} not i64"))),
        Value::Number(n) => n.as_i64().ok_or_else(|| bad(format!("{name} not i64"))),
        _ => Err(bad(format!("{name} missing/wrong type"))),
    }
}

fn str_field(v: &Value, name: &str) -> Result<String> {
    let s = v
        .as_str()
        .ok_or_else(|| bad(format!("{name} missing/wrong type")))?;
    // Truncate, don't fail — display text is bounded data.
    Ok(s.chars().take(MAX_RATIONALE_CHARS).collect())
}

/// Parse + validate a worker document. Strict shape: wrong types,
/// out-of-range values, oversized collections and missing required
/// fields are all rejected. Unknown keys are ignored — they are data,
/// never instructions, and are dropped from the record (T56).
pub fn parse_document(bytes: &[u8]) -> Result<ProposalDocument> {
    let v: Value = serde_json::from_slice(bytes).map_err(|e| bad(e.to_string()))?;
    if v.get("v").and_then(|x| x.as_u64()) != Some(1) {
        return Err(bad("missing/unsupported document version"));
    }
    if v.get("doc").and_then(|d| d.as_str()) != Some(DOC_TAG) {
        return Err(bad("wrong document tag"));
    }
    let gen = v.get("generator").ok_or_else(|| bad("generator missing"))?;
    let generator = GeneratorInfo {
        id: str_field(
            gen.get("id").ok_or_else(|| bad("generator.id missing"))?,
            "id",
        )?,
        version: str_field(
            gen.get("version")
                .ok_or_else(|| bad("generator.version missing"))?,
            "version",
        )?,
        model: str_field(
            gen.get("model")
                .ok_or_else(|| bad("generator.model missing"))?,
            "model",
        )?,
    };
    let seed = match v.get("seed") {
        Some(Value::String(s)) => s.trim().parse::<u64>().map_err(|_| bad("seed not u64"))?,
        Some(Value::Number(n)) => n.as_u64().ok_or_else(|| bad("seed not u64"))?,
        _ => return Err(bad("seed missing")),
    };
    let props = v
        .get("proposals")
        .and_then(|p| p.as_array())
        .ok_or_else(|| bad("proposals missing"))?;
    if props.is_empty() || props.len() > MAX_PROPOSALS {
        return Err(bad("proposals empty or over bound"));
    }
    let mut candidates = Vec::with_capacity(props.len());
    let mut seen_ids = std::collections::BTreeSet::new();
    for p in props {
        let id = str_field(p.get("id").ok_or_else(|| bad("id missing"))?, "id")?
            .chars()
            .take(MAX_ID_CHARS)
            .collect::<String>();
        if id.is_empty() || !seen_ids.insert(id.clone()) {
            return Err(bad("duplicate/empty candidate id"));
        }
        let score = p
            .get("score")
            .and_then(|s| s.as_f64())
            .ok_or_else(|| bad("score missing"))?;
        if !score.is_finite() || !(0.0..=1.0).contains(&score) {
            return Err(bad("score not a finite 0..1 value"));
        }
        let notes_v = p
            .get("notes")
            .and_then(|n| n.as_array())
            .ok_or_else(|| bad("notes missing"))?;
        if notes_v.is_empty() || notes_v.len() > MAX_CANDIDATE_NOTES {
            return Err(bad("notes empty or over bound"));
        }
        let mut notes = Vec::with_capacity(notes_v.len());
        let mut prev_onset = i64::MIN;
        for n in notes_v {
            let pitch = n
                .get("pitch")
                .and_then(|x| x.as_i64())
                .ok_or_else(|| bad("pitch missing"))? as i32;
            let velocity = n
                .get("velocity")
                .and_then(|x| x.as_i64())
                .ok_or_else(|| bad("velocity missing"))? as i32;
            let onset = i64_field(
                n.get("onsetTicks")
                    .ok_or_else(|| bad("onsetTicks missing"))?,
                "onsetTicks",
            )?;
            let length = i64_field(
                n.get("lengthTicks")
                    .ok_or_else(|| bad("lengthTicks missing"))?,
                "lengthTicks",
            )?;
            if !(0..=127).contains(&pitch)
                || !(1..=127).contains(&velocity)
                || length <= 0
                || onset < 0
            {
                return Err(bad("note out of range"));
            }
            // Monotone non-decreasing onsets keep candidates sane.
            if onset < prev_onset {
                return Err(bad("notes not ordered by onset"));
            }
            prev_onset = onset;
            notes.push(DocumentNote {
                pitch,
                velocity,
                onset_ticks: onset,
                length_ticks: length,
            });
        }
        candidates.push(CandidateDoc {
            id,
            score,
            rationale: str_field(
                p.get("rationale").ok_or_else(|| bad("rationale missing"))?,
                "rationale",
            )?,
            notes,
        });
    }
    Ok(ProposalDocument {
        generator,
        seed,
        candidates,
        analysis: v.get("analysis").cloned().unwrap_or(Value::Null),
        warnings: Vec::new(),
    })
}
