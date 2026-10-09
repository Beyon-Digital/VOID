//! OSC address-space model + bounded-message validation (T94/T96 side).
//!
//! Models the OSC 1.0 address rules without claiming wire delivery:
//! an address is a slash-separated path of non-empty segments, with
//! the spec's forbidden characters checked and pattern characters
//! kept out of *bound* addresses (a bound control address must be
//! literal — pattern matching is only allowed on incoming dispatch).
//!
//! Bounded validation: every declared control has a parameter spec
//! (type tags + value range). A message whose args can't satisfy the
//! spec is rejected with a typed error — bounds are enforced, not
//! approximated.

use serde::{Deserialize, Serialize};

use crate::error::SyncError;

/// Characters forbidden inside an OSC address segment (space `#` `*`
/// `,` `/` `[` `]` `{` `}` `?` — plus `/` which is the separator and
/// is never inside a segment by construction).
const FORBIDDEN_IN_SEGMENT: &[char] = &[' ', '#', '*', ',', '[', ']', '{', '}', '?'];

/// Max message payload we accept modelling (bounded validation — a
/// sane bound, not unlimited).
pub const MAX_OSC_PAYLOAD_ARGS: usize = 64;

/// A validated OSC address: `/seg/seg/...` — literal segments only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OscAddress(String);

impl OscAddress {
    /// Parse a literal address. Rejects empty, non-absolute, bad
    /// chars, empty segments, and pattern wildcards.
    pub fn parse(s: &str) -> Result<Self, SyncError> {
        if s.is_empty() {
            return Err(SyncError::Osc("empty address".into()));
        }
        if !s.starts_with('/') {
            return Err(SyncError::Osc(format!(
                "address {s:?} must start with '/'"
            )));
        }
        if s.len() == 1 {
            // "/" is the root — legal as a namespace root, never as a
            // bound target.
            return Err(SyncError::Osc("root '/' is not a bound address".into()));
        }
        for (i, seg) in s[1..].split('/').enumerate() {
            if seg.is_empty() {
                return Err(SyncError::Osc(format!(
                    "address {s:?} has empty segment at index {i}"
                )));
            }
            for &c in FORBIDDEN_IN_SEGMENT {
                if seg.contains(c) {
                    return Err(SyncError::Osc(format!(
                        "address {s:?} segment {seg:?} contains forbidden {c:?}"
                    )));
                }
            }
        }
        Ok(OscAddress(s.to_string()))
    }

    /// Pattern syntax (`* ? [ ] { }`) is allowed only on dispatch
    /// patterns, never on bound addresses — parse() already rejects
    /// them, this documents that split.
    pub fn is_pattern_safe(&self) -> bool {
        true
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// `true` if `pattern` (with OSC wildcards) could match this
    /// address — used to express that a bound address is covered by a
    /// dispatch pattern. Literal-only implementation of the spec's
    /// match rules: `*` (any chars incl. empty, within a segment),
    /// `?` (one char), `[abc]`/`[!abc]`/`[a-z]` char sets,
    /// `{a,b}` alternatives, `/` segment boundaries.
    pub fn matches(&self, pattern: &str) -> bool {
        let addr_segs: Vec<&str> = self.0[1..].split('/').collect();
        let Some(pat) = pattern
            .strip_prefix('/')
            .map(|p| p.split('/').collect::<Vec<_>>())
        else {
            return false;
        };
        if pat.len() != addr_segs.len() {
            return false;
        }
        pat.iter()
            .zip(addr_segs.iter())
            .all(|(p, a)| segment_match(p, a))
    }
}

/// OSC segment-pattern match (spec §OSC Messages / dispatch).
fn segment_match(pat: &str, seg: &str) -> bool {
    // Recursive matcher over pattern chars.
    fn m(p: &[u8], s: &[u8]) -> bool {
        if p.is_empty() {
            return s.is_empty();
        }
        match p[0] {
            b'*' => {
                // * matches zero+ chars in-segment.
                (0..=s.len()).any(|k| m(&p[1..], &s[k..]))
            }
            b'?' => !s.is_empty() && m(&p[1..], &s[1..]),
            b'[' => {
                // [abc] / [!abc] / [a-z] — find closing ].
                if let Some(end) = p[1..].iter().position(|&c| c == b']') {
                    let inner = &p[1..1 + end];
                    if s.is_empty() {
                        return false;
                    }
                    let (neg, set) = if inner.first() == Some(&b'!') {
                        (true, &inner[1..])
                    } else {
                        (false, inner)
                    };
                    let hit = set_contains(set, s[0]);
                    (hit != neg) && m(&p[2 + end..], &s[1..])
                } else {
                    // Unclosed bracket: treat literally (spec doesn't
                    // define; being literal is honest).
                    p[0] == s.first().copied().unwrap_or(0) && m(&p[1..], &s[1..])
                }
            }
            b'{' => {
                // {a,b} alternation — split on top-level commas.
                if let Some(end) = p[1..].iter().position(|&c| c == b'}') {
                    let inner = &p[1..1 + end];
                    let alts: Vec<&[u8]> = split_alts(inner);
                    alts.iter().any(|alt| {
                        let mut merged = alt.to_vec();
                        merged.extend_from_slice(&p[2 + end..]);
                        m(&merged, s)
                    })
                } else {
                    p[0] == s.first().copied().unwrap_or(0) && m(&p[1..], &s[1..])
                }
            }
            c => !s.is_empty() && s[0] == c && m(&p[1..], &s[1..]),
        }
    }
    m(pat.as_bytes(), seg.as_bytes())
}

fn split_alts(inner: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut start = 0;
    for (i, &c) in inner.iter().enumerate() {
        if c == b',' {
            out.push(&inner[start..i]);
            start = i + 1;
        }
    }
    out.push(&inner[start..]);
    out
}

fn set_contains(set: &[u8], c: u8) -> bool {
    let mut i = 0;
    while i < set.len() {
        if i + 2 < set.len() && set[i + 1] == b'-' {
            if (set[i]..=set[i + 2]).contains(&c) {
                return true;
            }
            i += 3;
        } else {
            if set[i] == c {
                return true;
            }
            i += 1;
        }
    }
    false
}

/// OSC type tags VOID bounds (per spec 1.0 typetag chars).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OscArgType {
    Int32,   // 'i'
    Float32, // 'f'
    String,  // 's'
    Blob,    // 'b'
    True,    // 'T' (no payload — the tag itself is the value)
    False,   // 'F'
    Null,    // 'N'
}

impl OscArgType {
    pub fn tag(self) -> char {
        match self {
            OscArgType::Int32 => 'i',
            OscArgType::Float32 => 'f',
            OscArgType::String => 's',
            OscArgType::Blob => 'b',
            OscArgType::True => 'T',
            OscArgType::False => 'F',
            OscArgType::Null => 'N',
        }
    }
}

/// A declared bounded argument: type + optional numeric range.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OscArgSpec {
    pub arg_type: OscArgType,
    /// For i/f: inclusive min..max. `None` = full type range.
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// For s/b: max byte length. Bounded validation.
    pub max_len: Option<u32>,
}

impl OscArgSpec {
    pub fn int32() -> Self {
        OscArgSpec {
            arg_type: OscArgType::Int32,
            min: None,
            max: None,
            max_len: None,
        }
    }
    pub fn float_range(min: f64, max: f64) -> Self {
        OscArgSpec {
            arg_type: OscArgType::Float32,
            min: Some(min),
            max: Some(max),
            max_len: None,
        }
    }
    pub fn string(max_len: u32) -> Self {
        OscArgSpec {
            arg_type: OscArgType::String,
            min: None,
            max: None,
            max_len: Some(max_len),
        }
    }
}

/// A declared control surface address: literal address + the bounded
/// argument spec a message must satisfy to be dispatched.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OscControl {
    /// Literal bound address (parsed — patterns rejected).
    #[serde(deserialize_with = "de_address")]
    pub address: String,
    /// Required arg spec sequence — count and types must match.
    pub args: Vec<OscArgSpec>,
    /// Human label for control lists.
    pub label: String,
}

fn de_address<'de, D>(d: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(d)?;
    OscAddress::parse(&s).map_err(serde::de::Error::custom)?;
    Ok(s)
}

/// One incoming message argument (already un-packed from the wire).
#[derive(Debug, Clone, PartialEq)]
pub enum OscArg {
    Int32(i32),
    Float32(f32),
    Str(String),
    Blob(Vec<u8>),
    True,
    False,
    Null,
}

impl OscArg {
    fn arg_type(&self) -> OscArgType {
        match self {
            OscArg::Int32(_) => OscArgType::Int32,
            OscArg::Float32(_) => OscArgType::Float32,
            OscArg::Str(_) => OscArgType::String,
            OscArg::Blob(_) => OscArgType::Blob,
            OscArg::True => OscArgType::True,
            OscArg::False => OscArgType::False,
            OscArg::Null => OscArgType::Null,
        }
    }
}

/// A message presented for bounded validation.
#[derive(Debug, Clone)]
pub struct OscMessage {
    pub address: String,
    pub args: Vec<OscArg>,
}

fn check_range(i: usize, v: f64, spec: &OscArgSpec) -> Result<(), SyncError> {
    if !v.is_finite() {
        return Err(SyncError::Osc(format!("arg {i}: non-finite value")));
    }
    if let Some(mn) = spec.min {
        if v < mn {
            return Err(SyncError::Osc(format!("arg {i}: {v} < min {mn}")));
        }
    }
    if let Some(mx) = spec.max {
        if v > mx {
            return Err(SyncError::Osc(format!("arg {i}: {v} > max {mx}")));
        }
    }
    Ok(())
}

/// Validate `msg` against the declared control. Bounds are hard:
/// wrong address, arg-count/type mismatch, range overflow, or payload
/// over `MAX_OSC_PAYLOAD_ARGS` are all typed rejects.
pub fn validate_against(control: &OscControl, msg: &OscMessage) -> Result<(), SyncError> {
    if msg.args.len() > MAX_OSC_PAYLOAD_ARGS {
        return Err(SyncError::Osc(format!(
            "message carries {} args > bound {MAX_OSC_PAYLOAD_ARGS}",
            msg.args.len()
        )));
    }
    // Address must equal the bound literal address exactly (a bound
    // control is not a pattern).
    if msg.address != control.address {
        return Err(SyncError::Osc(format!(
            "message to {:?} does not match bound {:?}",
            msg.address, control.address
        )));
    }
    if msg.args.len() != control.args.len() {
        return Err(SyncError::Osc(format!(
            "message has {} args, control requires {}",
            msg.args.len(),
            control.args.len()
        )));
    }
    for (i, (spec, arg)) in control.args.iter().zip(msg.args.iter()).enumerate() {
        if arg.arg_type() != spec.arg_type {
            return Err(SyncError::Osc(format!(
                "arg {i}: got {:?} tag, spec requires '{}'",
                arg.arg_type(),
                spec.arg_type.tag()
            )));
        }
        match arg {
            OscArg::Int32(n) => check_range(i, *n as f64, spec)?,
            OscArg::Float32(n) => check_range(i, *n as f64, spec)?,
            OscArg::Str(s) => {
                if let Some(l) = spec.max_len {
                    if s.len() as u32 > l {
                        return Err(SyncError::Osc(format!(
                            "arg {i}: string len {} > max {l}",
                            s.len()
                        )));
                    }
                }
            }
            OscArg::Blob(b) => {
                if let Some(l) = spec.max_len {
                    if b.len() as u32 > l {
                        return Err(SyncError::Osc(format!(
                            "arg {i}: blob len {} > max {l}",
                            b.len()
                        )));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}
