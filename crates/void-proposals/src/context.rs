//! Scoped context digest (W13 item 2; CONTRACTS.md §6).
//!
//! The context is everything the worker sees — a bounded digest of the
//! selected region, never the whole song. `context_sha256` hashes the
//! canonical serialization: it is the identity the accept path
//! revalidates against (a changed region = a different context = stale).

use crate::error::{ProposalError, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const MAX_SEED_NOTES: usize = 8192;
pub const MAX_LOCKED_RANGES: usize = 256;
pub const MAX_LABELS: usize = 64;
pub const MAX_LABEL_CHARS: usize = 256;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NoteEvent {
    pub pitch: i32,
    pub velocity: i32,
    pub onset_ticks: String,
    pub length_ticks: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TickRange {
    pub start_ticks: String,
    pub length_ticks: String,
}

impl TickRange {
    pub fn start(&self) -> i64 {
        self.start_ticks.parse().unwrap_or(0)
    }
    pub fn len(&self) -> i64 {
        self.length_ticks.parse().unwrap_or(0)
    }
    pub fn is_empty(&self) -> bool {
        self.len() <= 0
    }
    pub fn end(&self) -> i64 {
        self.start() + self.len()
    }
    pub fn overlaps(&self, start: i64, end: i64) -> bool {
        self.start() < end && start < self.end()
    }
}

/// Everything sent to the worker + recorded as the proposal's identity.
/// `labels` carry lyric/clip/asset text — inert data, never instructions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegionContext {
    pub track_id: String,
    pub clip_id: String,
    pub region: TickRange,
    pub continuation_start_ticks: String,
    pub continuation_ticks: String,
    pub tempo_bpm: f64,
    pub ts_num: u32,
    pub ts_den: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key_hint: Option<String>,
    #[serde(default)]
    pub notes: Vec<NoteEvent>,
    #[serde(default)]
    pub locked_ranges: Vec<TickRange>,
    #[serde(default)]
    pub labels: Vec<String>,
}

impl RegionContext {
    pub fn validate(&self) -> Result<()> {
        let bad = |m: &str| ProposalError::InvalidRequest(m.into());
        if !void_protocol::ids::is_valid_id(&self.track_id)
            || !void_protocol::ids::is_valid_id(&self.clip_id)
        {
            return Err(bad("track/clip id not a UUID"));
        }
        if self.notes.len() > MAX_SEED_NOTES
            || self.locked_ranges.len() > MAX_LOCKED_RANGES
            || self.labels.len() > MAX_LABELS
        {
            return Err(bad("context bounds exceeded"));
        }
        for l in &self.labels {
            if l.chars().count() > MAX_LABEL_CHARS {
                return Err(bad("label too long"));
            }
        }
        for n in &self.notes {
            let onset: i64 = n
                .onset_ticks
                .parse()
                .map_err(|_| bad("note onset not i64"))?;
            let len: i64 = n
                .length_ticks
                .parse()
                .map_err(|_| bad("note length not i64"))?;
            if !(0..=127).contains(&n.pitch)
                || !(1..=127).contains(&n.velocity)
                || len <= 0
                || onset < self.region.start()
            {
                return Err(bad("seed note out of bounds or outside region"));
            }
        }
        if self.continuation_ticks.parse::<i64>().unwrap_or(0) <= 0
            || self.continuation_start_ticks.parse::<i64>().is_err()
            || self.tempo_bpm <= 0.0
            || !self.tempo_bpm.is_finite()
            || self.ts_num == 0
            || self.ts_den == 0
        {
            return Err(bad("bad continuation window / tempo / meter"));
        }
        Ok(())
    }

    /// Canonical hash — the "input region hash" recorded in provenance.
    /// serde_json serializes struct fields in declaration order and map
    /// keys sorted, so equal contexts hash identically.
    pub fn sha256(&self) -> String {
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        let d: [u8; 32] = Sha256::digest(&bytes).into();
        d.iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Default deterministic seed: first 8 bytes of the context hash.
    /// Same region ⇒ same seed ⇒ same proposals (contract determinism).
    pub fn default_seed(&self) -> u64 {
        let h = self.sha256();
        u64::from_str_radix(&h[..16], 16).unwrap_or(0)
    }

    /// The worker `parameters` object — the whole context, verbatim.
    pub fn to_worker_params(&self, seed: u64, max_proposals: u32) -> serde_json::Value {
        serde_json::json!({
            "seed": seed.to_string(),
            "maxProposals": max_proposals,
            "continuationStartTicks": self.continuation_start_ticks,
            "continuationTicks": self.continuation_ticks,
            "trackId": self.track_id,
            "clipId": self.clip_id,
            "tempoBpm": self.tempo_bpm,
            "tsNum": self.ts_num,
            "tsDen": self.ts_den,
            "keyHint": self.key_hint,
            "notes": self.notes,
            "lockedRanges": self.locked_ranges,
            "labels": self.labels,
        })
    }
}
